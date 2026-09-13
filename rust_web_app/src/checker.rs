use crate::feishu;
use crate::history;
use crate::models::{FeishuConfig, HealthCheck};
use crate::probe;
use chrono::Utc;
use sqlx::SqlitePool;

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

        execute_health_check(&pool, &client, &check, feishu.as_ref()).await;
    }
}

pub async fn execute_health_check(
    pool: &SqlitePool,
    client: &reqwest::Client,
    check: &HealthCheck,
    feishu: Option<&FeishuConfig>,
) {
    let probe = probe::run_probe(client, &check.method, &check.url, check.expected_status).await;
    let status = probe.status;
    let response_ms = probe.response_ms;
    let error = probe.error;

    let checked_at = Utc::now().to_rfc3339();
    let prev_status = check.last_status.as_deref();

    if let Err(e) = sqlx::query(
        "UPDATE health_checks SET last_checked_at = ?, last_status = ?, \
         last_response_ms = ?, last_error = ? WHERE id = ?",
    )
    .bind(&checked_at)
    .bind(&status)
    .bind(response_ms)
    .bind(&error)
    .bind(check.id)
    .execute(pool)
    .await
    {
        tracing::warn!(
            "failed to update latest status for check {} (alerts will still run): {}",
            check.id,
            e
        );
    }

    if let Err(e) = history::insert_run(
        pool,
        check.id,
        &status,
        response_ms,
        &error,
        &checked_at,
        &probe.request_message,
        &probe.response_message,
    )
    .await
    {
        tracing::warn!(
            "history insert failed for check {} (alerts will still run): {}",
            check.id,
            e
        );
    }

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
