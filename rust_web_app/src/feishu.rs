use anyhow::Context;
use serde_json::json;

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
