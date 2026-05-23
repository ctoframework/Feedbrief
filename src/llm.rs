use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

use crate::fetcher::Article;
use crate::progress::ProgressEvent;

const OLLAMA_URL: &str = "http://localhost:11434/api/generate";

#[derive(Serialize)]
struct OllamaRequest<'a> {
    model: &'a str,
    prompt: String,
    stream: bool,
    format: Option<&'a str>,
    options: OllamaOptions,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_predict: i32,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

async fn ollama_call(
    client: &reqwest::Client,
    model: &str,
    prompt: String,
    json_mode: bool,
    max_tokens: i32,
) -> Result<String> {
    let req = OllamaRequest {
        model,
        prompt,
        stream: false,
        format: if json_mode { Some("json") } else { None },
        options: OllamaOptions {
            temperature: 0.2,
            num_predict: max_tokens,
        },
    };
    let resp = client
        .post(OLLAMA_URL)
        .timeout(Duration::from_secs(240))
        .json(&req)
        .send()
        .await
        .context("Ollama not reachable — is `ollama serve` running?")?;

    let status = resp.status();
    let text = resp
        .text()
        .await
        .context("Failed to read Ollama response body")?;

    if !status.is_success() {
        anyhow::bail!("Ollama returned error {}: {}", status, text);
    }

    let body: OllamaResponse = serde_json::from_str(&text)
        .with_context(|| format!("Failed to parse Ollama JSON response: {}", text))?;
    Ok(body.response)
}

#[derive(Deserialize)]
struct ScoringResult {
    relevance: f32,
    topic: String,
}

pub async fn score_articles(
    client: &reqwest::Client,
    model: &str,
    persona_name: &str,
    persona_description: &str,
    articles: &mut [Article],
    tx: &UnboundedSender<ProgressEvent>,
) -> Result<()> {
    let total = articles.len();
    let n_batches = (total + 4) / 5;
    let mut batch_idx = 0usize;

    for chunk in articles.chunks_mut(5) {
        batch_idx += 1;
        let percent = 28 + (batch_idx * 22 / n_batches.max(1)) as u8;
        let _ = tx.send(ProgressEvent::Stage {
            stage: "SCORE".into(),
            message: format!(
                "Scoring batch {}/{} ({} articles)…",
                batch_idx,
                n_batches,
                chunk.len()
            ),
            percent,
        });

        let items: Vec<String> = chunk
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let snippet: String = a.summary.chars().take(200).collect();
                format!("{}. [{}] {}\n   {}", i + 1, a.source, a.title, snippet)
            })
            .collect();

        let prompt = format!(
            r#"You are an intelligence analyst for {}. Score each item below by how much it matters for someone tracking: {}

For each item, return a relevance score 0.0–10.0 (10 = must-read, 0 = noise) and a short topic tag (2–4 words, lowercase). Invent new tags if a story doesn't fit existing ones — emerging themes matter.

Items:
{}

Return ONLY a JSON object of this exact shape:
{{"scores": [{{"relevance": 7.5, "topic": "llm training"}}, ...]}}
The array must have exactly {} entries in the same order as the items above."#,
            persona_name,
            persona_description,
            items.join("\n\n"),
            chunk.len()
        );

        let response = ollama_call(client, model, prompt, true, 400).await?;

        #[derive(Deserialize)]
        struct Wrapper {
            scores: Vec<ScoringResult>,
        }

        match serde_json::from_str::<Wrapper>(&response) {
            Ok(w) if w.scores.len() == chunk.len() => {
                for (article, score) in chunk.iter_mut().zip(w.scores.iter()) {
                    article.relevance = Some(score.relevance);
                    article.topic_tag = Some(score.topic.clone());
                }
            }
            _ => {
                for a in chunk.iter_mut() {
                    a.relevance = Some(5.0);
                    a.topic_tag = Some("uncategorized".to_string());
                }
            }
        }
    }
    Ok(())
}

pub async fn summarize_article(
    client: &reqwest::Client,
    model: &str,
    persona_name: &str,
    article: &Article,
) -> Result<String> {
    let body: String = article.summary.chars().take(1500).collect();
    let prompt = format!(
        r#"Summarize the following news item in EXACTLY 2 sentences for {}. Be concrete: name the actors, the number, the technique, the impact. No fluff, no "in this article", no editorializing.

Title: {}
Source: {}
Content: {}

Two-sentence summary:"#,
        persona_name, article.title, article.source, body
    );
    let response = ollama_call(client, model, prompt, false, 200).await?;
    Ok(response.trim().to_string())
}

pub async fn daily_brief(
    client: &reqwest::Client,
    model: &str,
    persona_name: &str,
    top_articles: &[Article],
) -> Result<String> {
    let bullets: Vec<String> = top_articles
        .iter()
        .take(15)
        .map(|a| format!("- [{}] {}", a.topic_tag.as_deref().unwrap_or("?"), a.title))
        .collect();

    let prompt = format!(
        r#"You are briefing {}. Below are today's top headlines, already filtered for relevance. Write a single-paragraph (4–6 sentences) executive briefing that synthesizes the THEMES of the day — what's the through-line? what's accelerating? what should they pay attention to this week? Be specific, name companies and technologies. No greetings, no sign-off, just the briefing.

Today's top items:
{}

Briefing:"#,
        persona_name,
        bullets.join("\n")
    );
    let response = ollama_call(client, model, prompt, false, 350).await?;
    Ok(response.trim().to_string())
}

pub fn ollama_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(240))
        .build()
        .expect("ollama client")
}

pub async fn check_ollama(model: &str) -> bool {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let resp = match client.get("http://localhost:11434/api/tags").send().await {
        Ok(r) => r,
        Err(_) => return false,
    };
    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => return false,
    };
    body.get("models")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter().any(|m| {
                m.get("name")
                    .and_then(|n| n.as_str())
                    .map(|s| s.starts_with(model))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}
