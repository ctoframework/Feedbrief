use anyhow::{Context, Result};
use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;

use crate::fetcher::Article;
use crate::progress::BriefStats;

pub struct Storage {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct StoredBrief {
    pub date: NaiveDate,
    pub brief: String,
    pub articles: Vec<Article>,
    pub stats: BriefStats,
    pub model: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub fn data_dir() -> PathBuf {
    directories::ProjectDirs::from("com", "techbrief", "TechBrief")
        .map(|p| p.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

impl Storage {
    pub fn open() -> Result<Self> {
        let dir = data_dir();
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("briefs.db");
        let conn = Connection::open(path)?;
        conn.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS briefs (
                date         TEXT PRIMARY KEY,
                brief_text   TEXT NOT NULL,
                articles_json TEXT NOT NULL,
                feeds_fetched INTEGER NOT NULL,
                total_articles INTEGER NOT NULL,
                articles_kept INTEGER NOT NULL,
                model        TEXT NOT NULL,
                created_at   TEXT NOT NULL
            );
        "#)?;
        Ok(Self { conn })
    }

    /// Save (or overwrite) today's brief.
    pub fn save(&self, date: NaiveDate, brief: &str, articles: &[Article], stats: &BriefStats, model: &str) -> Result<()> {
        let articles_json = serde_json::to_string(articles)?;
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT OR REPLACE INTO briefs (date, brief_text, articles_json, feeds_fetched, total_articles, articles_kept, model, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                date.format("%Y-%m-%d").to_string(),
                brief,
                articles_json,
                stats.feeds_fetched as i64,
                stats.total_articles as i64,
                stats.articles_kept as i64,
                model,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn load(&self, date: NaiveDate) -> Result<Option<StoredBrief>> {
        let mut stmt = self.conn.prepare(
            "SELECT date, brief_text, articles_json, feeds_fetched, total_articles, articles_kept, model, created_at
             FROM briefs WHERE date = ?",
        )?;
        let mut rows = stmt.query(params![date.format("%Y-%m-%d").to_string()])?;
        if let Some(row) = rows.next()? {
            let date_str: String = row.get(0)?;
            let brief: String = row.get(1)?;
            let articles_json: String = row.get(2)?;
            let stats = BriefStats {
                feeds_fetched: row.get::<_, i64>(3)? as usize,
                total_articles: row.get::<_, i64>(4)? as usize,
                articles_kept: row.get::<_, i64>(5)? as usize,
            };
            let model: String = row.get(6)?;
            let created_at_str: String = row.get(7)?;
            let articles: Vec<Article> = serde_json::from_str(&articles_json)?;
            Ok(Some(StoredBrief {
                date: NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")?,
                brief,
                articles,
                stats,
                model,
                created_at: chrono::DateTime::parse_from_rfc3339(&created_at_str)?.with_timezone(&chrono::Utc),
            }))
        } else {
            Ok(None)
        }
    }

    /// All dates that have a brief, sorted ascending.
    pub fn all_dates(&self) -> Result<Vec<NaiveDate>> {
        let mut stmt = self.conn.prepare("SELECT date FROM briefs ORDER BY date ASC")?;
        let dates: Vec<NaiveDate> = stmt.query_map([], |row| {
            let s: String = row.get(0)?;
            Ok(NaiveDate::parse_from_str(&s, "%Y-%m-%d").unwrap_or_else(|_| NaiveDate::from_ymd_opt(2000, 1, 1).unwrap()))
        })?
        .filter_map(|r| r.ok())
        .collect();
        Ok(dates)
    }

    pub fn previous_date(&self, current: NaiveDate) -> Result<Option<NaiveDate>> {
        let result = self.conn.query_row(
            "SELECT date FROM briefs WHERE date < ? ORDER BY date DESC LIMIT 1",
            params![current.format("%Y-%m-%d").to_string()],
            |row| {
                let s: String = row.get(0)?;
                Ok(NaiveDate::parse_from_str(&s, "%Y-%m-%d").unwrap())
            },
        ).optional().context("query previous_date")?;
        Ok(result)
    }

    pub fn next_date(&self, current: NaiveDate) -> Result<Option<NaiveDate>> {
        let result = self.conn.query_row(
            "SELECT date FROM briefs WHERE date > ? ORDER BY date ASC LIMIT 1",
            params![current.format("%Y-%m-%d").to_string()],
            |row| {
                let s: String = row.get(0)?;
                Ok(NaiveDate::parse_from_str(&s, "%Y-%m-%d").unwrap())
            },
        ).optional().context("query next_date")?;
        Ok(result)
    }
}
