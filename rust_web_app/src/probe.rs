use crate::checkpoints::{evaluate_all, CheckpointRule};
use reqwest::header::HeaderMap;
use reqwest::{Client, Method, Response};
use std::str::FromStr;

pub struct CheckProbeResult {
    pub status: String,
    pub response_ms: Option<i64>,
    pub error: Option<String>,
    pub request_message: String,
    pub response_message: Option<String>,
}

pub fn capture_max_bytes() -> usize {
    crate::config::get().probe.capture_max_bytes
}

pub fn build_request_message(method: &str, url: &str) -> String {
    let method = method.to_uppercase();
    let mut lines = vec![format!("{} {}", method, url)];
    if method == "POST" {
        lines.push(String::new());
        lines.push("(empty body)".to_string());
    }
    lines.join("\n")
}

pub async fn run_probe(
    client: &Client,
    method: &str,
    url: &str,
    expected_status: i64,
    checkpoints: &[CheckpointRule],
) -> CheckProbeResult {
    let request_message = build_request_message(method, url);
    let method_upper = method.to_uppercase();
    let max_body = capture_max_bytes();

    let http_method = match Method::from_str(&method_upper) {
        Ok(m) => m,
        Err(_) => {
            return CheckProbeResult {
                status: "error".into(),
                response_ms: None,
                error: Some(format!("unsupported method: {}", method_upper)),
                request_message,
                response_message: None,
            };
        }
    };

    let start = std::time::Instant::now();
    let request = client.request(http_method, url);

    match request.send().await {
        Ok(resp) => {
            let ms = start.elapsed().as_millis() as i64;
            let code = resp.status().as_u16() as i64;
            let head_only = method_upper == "HEAD";
            let (response_message, body_for_check) =
                read_response(resp, head_only, max_body).await;

            let (status, error) = if code != expected_status {
                (
                    "down".to_string(),
                    Some(format!("status {} (expected {})", code, expected_status)),
                )
            } else if !checkpoints.is_empty() && head_only {
                (
                    "down".to_string(),
                    Some("HEAD 请求无响应体，无法执行检查点".into()),
                )
            } else if let Some(cp_err) = evaluate_all(&body_for_check, checkpoints) {
                ("down".to_string(), Some(cp_err))
            } else {
                ("up".to_string(), None)
            };

            CheckProbeResult {
                status,
                response_ms: Some(ms),
                error,
                request_message,
                response_message: Some(response_message),
            }
        }
        Err(e) => CheckProbeResult {
            status: "down".into(),
            response_ms: None,
            error: Some(e.to_string()),
            request_message,
            response_message: None,
        },
    }
}

async fn read_response(resp: Response, head_only: bool, max_body: usize) -> (String, String) {
    let status = resp.status();
    let headers = resp.headers().clone();
    let mut lines = vec![format!("HTTP/1.1 {}", status)];
    append_headers(&mut lines, &headers);

    if head_only {
        return (lines.join("\n"), String::new());
    }

    lines.push(String::new());
    let body = match resp.text().await {
        Ok(b) => b,
        Err(e) => {
            let msg = format!("[failed to read body: {}]", e);
            lines.push(msg.clone());
            return (lines.join("\n"), String::new());
        }
    };

    let body_for_check = truncate_body(&body, max_body);
    lines.push(body_for_check.clone());
    (lines.join("\n"), body_for_check)
}

fn append_headers(lines: &mut Vec<String>, headers: &HeaderMap) {
    for (name, value) in headers.iter() {
        let v = value.to_str().unwrap_or("[binary]");
        lines.push(format!("{}: {}", name, v));
    }
}

fn truncate_body(body: &str, max_bytes: usize) -> String {
    if body.len() <= max_bytes {
        return body.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !body.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n\n[truncated, {} bytes total]",
        &body[..end],
        body.len()
    )
}
