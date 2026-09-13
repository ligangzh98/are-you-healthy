use chrono::{Duration, Utc};
use sqlx::SqlitePool;
use tracing::info;

#[derive(Clone, Debug)]
pub struct HistoryRetention {
    pub retention_days: u64,
    pub max_per_check: u64,
    pub cleanup_interval_secs: u64,
}

impl HistoryRetention {
    pub fn from_env() -> Self {
        Self {
            retention_days: env_u64("HISTORY_RETENTION_DAYS", 30),
            max_per_check: env_u64("HISTORY_MAX_PER_CHECK", 1000),
            cleanup_interval_secs: env_u64("HISTORY_CLEANUP_INTERVAL_SECS", 3600),
        }
    }
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

pub async fn insert_run(
    pool: &SqlitePool,
    check_id: i64,
    status: &str,
    response_ms: Option<i64>,
    error: &Option<String>,
    checked_at: &str,
    request_message: &str,
    response_message: &Option<String>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO check_runs (check_id, status, response_ms, error, checked_at, \
         request_message, response_message) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(check_id)
    .bind(status)
    .bind(response_ms)
    .bind(error)
    .bind(checked_at)
    .bind(request_message)
    .bind(response_message)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn cleanup(pool: &SqlitePool, retention: &HistoryRetention) -> Result<u64, sqlx::Error> {
    let mut deleted: u64 = 0;

    if retention.retention_days > 0 {
        let cutoff = (Utc::now() - Duration::days(retention.retention_days as i64)).to_rfc3339();
        let result = sqlx::query("DELETE FROM check_runs WHERE checked_at < ?")
            .bind(&cutoff)
            .execute(pool)
            .await?;
        deleted += result.rows_affected();
    }

    if retention.max_per_check > 0 {
        let result = sqlx::query(
            "DELETE FROM check_runs WHERE id IN (
                SELECT id FROM (
                    SELECT id,
                           ROW_NUMBER() OVER (PARTITION BY check_id ORDER BY checked_at DESC) AS rn
                    FROM check_runs
                ) AS ranked WHERE rn > ?
            )",
        )
        .bind(retention.max_per_check as i64)
        .execute(pool)
        .await?;
        deleted += result.rows_affected();
    }

    Ok(deleted)
}

pub fn spawn_cleanup_job(pool: SqlitePool, retention: HistoryRetention) {
    tokio::spawn(async move {
        let interval_secs = retention.cleanup_interval_secs.max(60);
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
        interval.tick().await;

        loop {
            match cleanup(&pool, &retention).await {
                Ok(n) if n > 0 => info!("history cleanup removed {} row(s)", n),
                Ok(_) => {}
                Err(e) => tracing::error!("history cleanup failed: {}", e),
            }
            interval.tick().await;
        }
    });
}
