use crate::feishu;
use crate::history;
use crate::models::{FeishuConfig, HealthCheck, PushplusConfig};
use crate::pushplus;
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
    let pushplus_cfg = load_pushplus_config(&pool).await;

    let now = Utc::now();

    for check in checks {
        if !is_due(&check.last_checked_at, check.interval_secs, &now) {
            continue;
        }

        execute_health_check(
            &pool,
            &client,
            &check,
            feishu.as_ref(),
            pushplus_cfg.as_ref(),
        )
        .await;
    }
}

pub async fn execute_health_check(
    pool: &SqlitePool,
    client: &reqwest::Client,
    check: &HealthCheck,
    feishu: Option<&FeishuConfig>,
    pushplus_cfg: Option<&PushplusConfig>,
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
        send_down_alerts(
            pool,
            client,
            check,
            feishu,
            pushplus_cfg,
            became_down,
            &checked_at,
            &error,
        )
        .await;
    } else if prev_status == Some("down") {
        send_recovery_alerts(pool, client, check, feishu, pushplus_cfg, &checked_at).await;
    }
}

async fn send_down_alerts(
    pool: &SqlitePool,
    client: &reqwest::Client,
    check: &HealthCheck,
    feishu: Option<&FeishuConfig>,
    pushplus: Option<&PushplusConfig>,
    became_down: bool,
    checked_at: &str,
    error: &Option<String>,
) {
    if !has_alert_channel(feishu, pushplus) {
        return;
    }

    let cooldown = alert_cooldown_secs(feishu, pushplus);
    let should_alert = became_down || cooldown_elapsed(pool, check.id, cooldown).await;
    if !should_alert {
        return;
    }

    let feishu_body = format_alert(&check.name, &check.url, error);
    let title = format!("健康检查告警: {}", check.name);
    let push_body = format_down_content(&check.name, &check.url, error);

    let mut sent = false;

    if let Some(cfg) = feishu {
        if cfg.enabled && !cfg.webhook_url.is_empty() {
            match feishu::send_text_alert(client, &cfg.webhook_url, &feishu_body).await {
                Ok(()) => sent = true,
                Err(e) => tracing::warn!("feishu alert failed for {}: {}", check.name, e),
            }
        }
    }

    if let Some(cfg) = pushplus {
        if cfg.enabled && !cfg.token.is_empty() {
            match pushplus::send_message(client, &cfg.token, &title, &push_body).await {
                Ok(()) => sent = true,
                Err(e) => tracing::warn!("pushplus alert failed for {}: {}", check.name, e),
            }
        }
    }

    if sent {
        let _ = sqlx::query(
            "INSERT INTO alert_state (check_id, last_alert_at) VALUES (?, ?) \
             ON CONFLICT(check_id) DO UPDATE SET last_alert_at = excluded.last_alert_at",
        )
        .bind(check.id)
        .bind(checked_at)
        .execute(pool)
        .await;
    }
}

async fn send_recovery_alerts(
    _pool: &SqlitePool,
    client: &reqwest::Client,
    check: &HealthCheck,
    feishu: Option<&FeishuConfig>,
    pushplus_cfg: Option<&PushplusConfig>,
    _checked_at: &str,
) {
    let time = feishu::format_time_east8();
    let feishu_msg = format!(
        "【健康检查恢复】\n名称: {}\nURL: {}\n时间: {}",
        check.name,
        check.url,
        time
    );
    let title = format!("健康检查恢复: {}", check.name);
    let push_body = format!(
        "名称: {}\nURL: {}\n时间: {}",
        check.name,
        check.url,
        time
    );

    if let Some(cfg) = feishu {
        if cfg.enabled && !cfg.webhook_url.is_empty() {
            let _ = feishu::send_text_alert(client, &cfg.webhook_url, &feishu_msg).await;
        }
    }

    if let Some(cfg) = pushplus_cfg {
        if cfg.enabled && !cfg.token.is_empty() {
            let _ = pushplus::send_message(client, &cfg.token, &title, &push_body).await;
        }
    }
}

fn has_alert_channel(feishu: Option<&FeishuConfig>, pushplus: Option<&PushplusConfig>) -> bool {
    feishu_enabled(feishu) || pushplus_enabled(pushplus)
}

fn feishu_enabled(cfg: Option<&FeishuConfig>) -> bool {
    cfg.is_some_and(|c| c.enabled && !c.webhook_url.is_empty())
}

fn pushplus_enabled(cfg: Option<&PushplusConfig>) -> bool {
    cfg.is_some_and(|c| c.enabled && !c.token.is_empty())
}

fn alert_cooldown_secs(feishu: Option<&FeishuConfig>, pushplus: Option<&PushplusConfig>) -> i64 {
    let mut secs = Vec::new();
    if let Some(c) = feishu {
        if c.enabled && !c.webhook_url.is_empty() {
            secs.push(c.alert_cooldown_secs);
        }
    }
    if let Some(c) = pushplus {
        if c.enabled && !c.token.is_empty() {
            secs.push(c.alert_cooldown_secs);
        }
    }
    secs.into_iter().min().unwrap_or(300)
}

fn format_down_content(name: &str, url: &str, error: &Option<String>) -> String {
    let detail = error.as_deref().unwrap_or("unknown error");
    format!(
        "名称: {}\nURL: {}\n原因: {}\n时间: {}",
        name,
        url,
        detail,
        feishu::format_time_east8()
    )
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

pub async fn load_pushplus_config(pool: &SqlitePool) -> Option<PushplusConfig> {
    sqlx::query_as::<_, PushplusConfig>(
        "SELECT id, token, enabled, alert_cooldown_secs FROM pushplus_config WHERE id = 1",
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
        feishu::format_time_east8()
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
