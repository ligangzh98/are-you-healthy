use crate::feishu;
use crate::history;
use crate::models::{FeishuConfig, HealthCheck};
use chrono::Utc;
use sqlx::SqlitePool;
use std::time::Instant;

pub async fn run_check(
    client: &reqwest::Client,
    method: &str,
    url: &str,
    expected_status: i64,
) -> (String, Option<i64>, Option<String>) {
    let start = Instant::now();
    let method = method.to_uppercase();

    let request = match method.as_str() {
        "GET" => client.get(url),
        "HEAD" => client.head(url),
        "POST" => client.post(url),
        other => {
            return (
                "error".into(),
                None,
                Some(format!("unsupported method: {}", other)),
            );
        }
    };

    match request.send().await {
        Ok(resp) => {
            let ms = start.elapsed().as_millis() as i64;
            let code = resp.status().as_u16() as i64;
            if code == expected_status {
                ("up".into(), Some(ms), None)
            } else {
                (
                    "down".into(),
                    Some(ms),
                    Some(format!("status {} (expected {})", code, expected_status)),
                )
            }
        }
        Err(e) => ("down".into(), None, Some(e.to_string())),
    }
}

pub async fn scheduler_tick(pool: SqlitePool, client: reqwest::Client) {
    let checks = sqlx::query_as::<_, crate::models::HealthCheck>(
        "SELECT id, name, url, method, expected_status, interval_secs, enabled, \
         last_checked_at, last_status, last_response_ms, last_error, created_at \
         FROM health_checks WHERE enabled = 1",
    )
    .fetch_all(&pool)
    .await;

    let checks = match checks {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("load checks: {}", e);
            return;
        }
    };

    let feishu = load_feishu_config(&pool).await;

    let now = Utc::now();

    for check in checks {
        if !is_due(&check.last_checked_at, check.interval_secs, &now) {
            continue;
        }

        if let Err(e) = execute_health_check(&pool, &client, &check, feishu.as_ref()).await {
            tracing::error!("check {}: {}", check.id, e);
        }
    }
}

pub async fn execute_health_check(
    pool: &SqlitePool,
    client: &reqwest::Client,
    check: &HealthCheck,
    feishu: Option<&FeishuConfig>,
) -> Result<(), sqlx::Error> {
    let (status, response_ms, error) = run_check(
        client,
        &check.method,
        &check.url,
        check.expected_status,
    )
    .await;

    let checked_at = Utc::now().to_rfc3339();
    let prev_status = check.last_status.as_deref();

    sqlx::query(
        "UPDATE health_checks SET last_checked_at = ?, last_status = ?, \
         last_response_ms = ?, last_error = ? WHERE id = ?",
    )
    .bind(&checked_at)
    .bind(&status)
    .bind(response_ms)
    .bind(&error)
    .bind(check.id)
    .execute(pool)
    .await?;

    history::insert_run(pool, check.id, &status, response_ms, &error, &checked_at).await?;

    let became_down = status == "down" && prev_status != Some("down");
    let still_down = status == "down";

    if still_down {
        if let Some(cfg) = feishu {
            if cfg.enabled && !cfg.webhook_url.is_empty() {
                let should_alert =
                    became_down || cooldown_elapsed(pool, check.id, cfg.alert_cooldown_secs).await;

                if should_alert {
                    let msg = format_alert(&check.name, &check.url, &error);
                    if let Err(e) = feishu::send_text_alert(client, &cfg.webhook_url, &msg).await {
                        tracing::warn!("feishu alert failed for {}: {}", check.name, e);
                    } else {
                        let _ = sqlx::query(
                            "INSERT INTO alert_state (check_id, last_alert_at) VALUES (?, ?) \
                             ON CONFLICT(check_id) DO UPDATE SET last_alert_at = excluded.last_alert_at",
                        )
                        .bind(check.id)
                        .bind(&checked_at)
                        .execute(pool)
                        .await;
                    }
                }
            }
        }
    } else if prev_status == Some("down") {
        if let Some(cfg) = feishu {
            if cfg.enabled && !cfg.webhook_url.is_empty() {
                let msg = format!(
                    "【健康检查恢复】\n名称: {}\nURL: {}\n时间: {}",
                    check.name,
                    check.url,
                    checked_at
                );
                let _ = feishu::send_text_alert(client, &cfg.webhook_url, &msg).await;
            }
        }
    }

    Ok(())
}

pub async fn load_feishu_config(pool: &SqlitePool) -> Option<FeishuConfig> {
    sqlx::query_as::<_, FeishuConfig>(
        "SELECT id, webhook_url, enabled, alert_cooldown_secs FROM feishu_config WHERE id = 1",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

fn is_due(last_checked_at: &Option<String>, interval_secs: i64, now: &chrono::DateTime<Utc>) -> bool {
    match last_checked_at {
        None => true,
        Some(s) => {
            let parsed = chrono::DateTime::parse_from_rfc3339(s);
            match parsed {
                Ok(t) => {
                    let elapsed = now.signed_duration_since(t.with_timezone(&Utc));
                    elapsed.num_seconds() >= interval_secs
                }
                Err(_) => true,
            }
        }
    }
}

async fn cooldown_elapsed(pool: &SqlitePool, check_id: i64, cooldown_secs: i64) -> bool {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT last_alert_at FROM alert_state WHERE check_id = ?")
            .bind(check_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();

    match row {
        None => true,
        Some((last,)) => {
            let parsed = chrono::DateTime::parse_from_rfc3339(&last);
            match parsed {
                Ok(t) => {
                    let elapsed = Utc::now().signed_duration_since(t.with_timezone(&Utc));
                    elapsed.num_seconds() >= cooldown_secs
                }
                Err(_) => true,
            }
        }
    }
}

fn format_alert(name: &str, url: &str, error: &Option<String>) -> String {
    let detail = error
        .as_deref()
        .unwrap_or("unknown error");
    format!(
        "【健康检查告警】\n名称: {}\nURL: {}\n原因: {}\n时间: {}",
        name,
        url,
        detail,
        Utc::now().to_rfc3339()
    )
}

pub fn spawn_scheduler(pool: SqlitePool, client: reqwest::Client) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            scheduler_tick(pool.clone(), client.clone()).await;
        }
    });
}
