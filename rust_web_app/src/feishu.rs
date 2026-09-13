use anyhow::Context;
use chrono::{FixedOffset, Utc};
use serde_json::json;

/// 飞书消息中展示用的东八区时间（UTC+8）。
pub fn format_time_east8() -> String {
    let east8 = FixedOffset::east_opt(8 * 3600).expect("UTC+8");
    Utc::now()
        .with_timezone(&east8)
        .format("%Y-%m-%d %H:%M:%S 东八区")
        .to_string()
}

pub async fn send_text_alert(
    client: &reqwest::Client,
    webhook_url: &str,
    text: &str,
) -> anyhow::Result<()> {
    if webhook_url.is_empty() {
        anyhow::bail!("webhook url is empty");
    }

    let body = json!({
        "msg_type": "text",
        "content": { "text": text }
    });

    let resp = client
        .post(webhook_url)
        .json(&body)
        .send()
        .await
        .context("feishu request")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("feishu webhook failed: {} {}", status, body);
    }

    Ok(())
}
