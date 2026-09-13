use anyhow::Context;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::debug;

const SEND_URL: &str = "https://www.pushplus.plus/send";

#[derive(Debug, Deserialize)]
struct PushplusResponse {
    code: Value,
    msg: Option<String>,
    data: Option<Value>,
}

fn code_is_success(code: &Value) -> bool {
    match code {
        Value::Number(n) => n.as_i64() == Some(200),
        Value::String(s) => s == "200",
        _ => false,
    }
}

fn format_api_error(code: &Value, msg: Option<&str>, data: Option<&Value>) -> String {
    let mut parts = vec![format!("code {}", code)];
    if let Some(m) = msg {
        if !m.is_empty() {
            parts.push(m.to_string());
        }
    }
    if let Some(Value::String(s)) = data {
        if !s.is_empty() {
            parts.push(s.clone());
        }
    }
    parts.join(": ")
}

/// 调用 [pushplus 发送接口](https://www.pushplus.plus/push1.html)（POST JSON，template=txt）。
pub async fn send_message(
    client: &reqwest::Client,
    token: &str,
    title: &str,
    content: &str,
) -> anyhow::Result<()> {
    if token.is_empty() {
        anyhow::bail!("pushplus token is empty");
    }

    let body = json!({
        "token": token,
        "title": title,
        "content": content,
        "template": "txt",
        "channel": "wechat",
    });

    let resp = client
        .post(SEND_URL)
        .json(&body)
        .send()
        .await
        .context("pushplus request")?;

    let status = resp.status();
    let text = resp.text().await.context("read pushplus response")?;
    debug!("pushplus response http={} body={}", status, text);

    let parsed = serde_json::from_str::<PushplusResponse>(&text)
        .context("pushplus response is not json")?;

    if code_is_success(&parsed.code) {
        return Ok(());
    }

    anyhow::bail!(
        "pushplus {}",
        format_api_error(&parsed.code, parsed.msg.as_deref(), parsed.data.as_ref())
    );
}
