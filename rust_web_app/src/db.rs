use anyhow::Context;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;

pub async fn init_pool(db_path: &Path) -> anyhow::Result<SqlitePool> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let url = format!("sqlite:{}?mode=rwc", db_path.display());
    let options = SqliteConnectOptions::from_str(&url)?.create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .context("connect sqlite")?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS health_checks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            url TEXT NOT NULL,
            method TEXT NOT NULL DEFAULT 'GET',
            expected_status INTEGER NOT NULL DEFAULT 200,
            interval_secs INTEGER NOT NULL DEFAULT 60,
            enabled INTEGER NOT NULL DEFAULT 1,
            last_checked_at TEXT,
            last_status TEXT,
            last_response_ms INTEGER,
            last_error TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS feishu_config (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            webhook_url TEXT NOT NULL DEFAULT '',
            enabled INTEGER NOT NULL DEFAULT 0,
            alert_cooldown_secs INTEGER NOT NULL DEFAULT 300
        );
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO feishu_config (id, webhook_url, enabled, alert_cooldown_secs)
        VALUES (1, '', 0, 300);
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS alert_state (
            check_id INTEGER PRIMARY KEY,
            last_alert_at TEXT,
            FOREIGN KEY (check_id) REFERENCES health_checks(id) ON DELETE CASCADE
        );
        "#,
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}
