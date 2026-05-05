use tokio::sync::mpsc::UnboundedSender;

use crate::feeds::default_feeds;
use crate::fetcher::fetch_all;
use crate::llm::{daily_brief, ollama_client, score_articles, summarize_article};
use crate::progress::{BriefStats, ProgressEvent};

pub struct PipelineConfig {
    pub model: String,
    pub hours: i64,
    pub top_n: usize,
}

pub async fn run_pipeline(cfg: PipelineConfig, tx: UnboundedSender<ProgressEvent>) {
    let sources = default_feeds();
    let n_feeds = sources.len();

    // === FETCH ===
    let articles = fetch_all(&sources, cfg.hours, &tx).await;
    let total = articles.len();
    let _ = tx.send(ProgressEvent::Stage {
        stage: "FETCH".into(),
        message: format!("Got {} articles after dedup. Preparing LLM…", total),
        percent: 26,
    });

    if articles.is_empty() {
        let _ = tx.send(ProgressEvent::Done {
            brief: "No articles found in the time window. Try expanding the hours filter.".to_string(),
            articles: vec![],
            stats: BriefStats { feeds_fetched: n_feeds, total_articles: 0, articles_kept: 0 },
        });
        return;
    }

    let client = ollama_client();
    let mut to_score: Vec<_> = articles.into_iter().take(80).collect();

    // === SCORE ===
    if let Err(e) = score_articles(&client, &cfg.model, &mut to_score, &tx).await {
        let _ = tx.send(ProgressEvent::Error(format!(
            "LLM scoring failed: {}. Is Ollama running with model '{}'?",
            e, cfg.model
        )));
        return;
    }

    to_score.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap_or(std::cmp::Ordering::Equal));
    let mut top: Vec<_> = to_score.into_iter().take(cfg.top_n).collect();
    let n = top.len();

    // === SUMMARIZE ===
    for (i, article) in top.iter_mut().enumerate() {
        let pct = 52 + ((i * 38) / n.max(1)) as u8;
        let title_short: String = article.title.chars().take(70).collect();
        let _ = tx.send(ProgressEvent::Stage {
            stage: "SUMMARIZE".into(),
            message: format!("[{}/{}] {}", i + 1, n, title_short),
            percent: pct,
        });
        match summarize_article(&client, &cfg.model, article).await {
            Ok(s) => article.ai_summary = Some(s),
            Err(_) => article.ai_summary = Some(article.summary.clone()),
        }
    }

    // === BRIEF ===
    let _ = tx.send(ProgressEvent::Stage {
        stage: "BRIEF".into(),
        message: "Synthesizing the day's themes…".into(),
        percent: 94,
    });
    let brief = daily_brief(&client, &cfg.model, &top).await
        .unwrap_or_else(|e| format!("(Brief generation failed: {}.)", e));

    let _ = tx.send(ProgressEvent::Stage {
        stage: "DONE".into(),
        message: "Complete.".into(),
        percent: 100,
    });

    let _ = tx.send(ProgressEvent::Done {
        brief,
        articles: top,
        stats: BriefStats { feeds_fetched: n_feeds, total_articles: total, articles_kept: n },
    });
}
