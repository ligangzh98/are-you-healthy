use chrono::Utc;
use sqlx::SqlitePool;
use tracing::warn;

use crate::feishu;
use crate::pushplus;

#[derive(Clone, Debug)]
pub struct AlertLogContext {
    pub kind: &'static str,
    pub check_id: Option<i64>,
    pub check_name: Option<String>,
}

pub async fn deliver_feishu(
    pool: &SqlitePool,
    client: &reqwest::Client,
    ctx: &AlertLogContext,
    webhook_url: &str,
    message: &str,
) -> bool {
    let sent_at = Utc::now().to_rfc3339();
    let result = feishu::send_text_alert(client, webhook_url, message).await;
    let (status, error) = match result {
        Ok(()) => ("ok", None),
        Err(e) => ("failed", Some(e.to_string())),
    };
    if let Err(e) = insert_row(
        pool,
        ctx,
        "feishu",
        status,
        None,
        message,
        error.as_deref(),
        &sent_at,
    )
    .await
    {
        warn!("alert history insert failed: {}", e);
    }
    status == "ok"
}

pub async fn deliver_pushplus(
    pool: &SqlitePool,
    client: &reqwest::Client,
    ctx: &AlertLogContext,
    token: &str,
    title: &str,
    content: &str,
) -> bool {
    let sent_at = Utc::now().to_rfc3339();
    let result = pushplus::send_message(client, token, title, content).await;
    let (status, error) = match result {
        Ok(()) => ("ok", None),
        Err(e) => ("failed", Some(e.to_string())),
    };
    if let Err(e) = insert_row(
        pool,
        ctx,
        "pushplus",
        status,
        Some(title),
        content,
        error.as_deref(),
        &sent_at,
    )
    .await
    {
        warn!("alert history insert failed: {}", e);
    }
    status == "ok"
}

async fn insert_row(
    pool: &SqlitePool,
    ctx: &AlertLogContext,
    channel: &str,
    status: &str,
    title: Option<&str>,
    message: &str,
    error: Option<&str>,
    sent_at: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO alert_deliveries (kind, channel, status, title, message, error, \
         check_id, check_name, sent_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(ctx.kind)
    .bind(channel)
    .bind(status)
    .bind(title)
    .bind(message)
    .bind(error)
    .bind(ctx.check_id)
    .bind(ctx.check_name.as_deref())
    .bind(sent_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn cleanup_by_age(pool: &SqlitePool, cutoff_rfc3339: &str) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM alert_deliveries WHERE sent_at < ?")
        .bind(cutoff_rfc3339)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}
