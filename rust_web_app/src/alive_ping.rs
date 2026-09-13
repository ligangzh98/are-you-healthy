use crate::config;
use crate::feishu;
use crate::pushplus;
use chrono::{FixedOffset, NaiveTime, Utc};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{info, warn};

const EAST8_SECS: i32 = 8 * 3600;

pub fn spawn_job(client: reqwest::Client) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if let Err(e) = tick(&client).await {
                warn!("alive ping tick: {}", e);
            }
        }
    });
}

async fn tick(client: &reqwest::Client) -> anyhow::Result<()> {
    let cfg = config::get();
    let ping = &cfg.alive_ping;
    if !ping.enabled {
        return Ok(());
    }

    let now = east8_now();
    let today = now.date_naive().to_string();
    if !due_today(now.time(), ping.send_time) {
        return Ok(());
    }

    let state_path = state_file_path(cfg);
    if last_sent_date(&state_path)? == Some(today.clone()) {
        return Ok(());
    }

    let sent = send_ping(client, cfg, &ping.title, &ping.message).await?;
    if !sent {
        return Ok(());
    }
    write_last_sent_date(&state_path, &today)?;
    info!("alive ping sent for {}", today);
    Ok(())
}

async fn send_ping(
    client: &reqwest::Client,
    cfg: &config::AppConfig,
    title: &str,
    message: &str,
) -> anyhow::Result<bool> {
    let time = feishu::format_time_east8();
    let feishu_body = format!("【{title}】\n{message}\n时间: {time}");
    let push_body = format!("{message}\n时间: {time}");

    let feishu_on = cfg.feishu.enabled && !cfg.feishu.webhook_url.trim().is_empty();
    let push_on = cfg.pushplus.enabled && !cfg.pushplus.token.trim().is_empty();
    if !feishu_on && !push_on {
        warn!("alive ping: enable feishu or pushplus in config.toml");
        return Ok(false);
    }

    let mut any_ok = false;

    if feishu_on {
        match feishu::send_text_alert(client, cfg.feishu.webhook_url.trim(), &feishu_body).await {
            Ok(()) => any_ok = true,
            Err(e) => warn!("alive ping feishu failed: {}", e),
        }
    }

    if push_on {
        match pushplus::send_message(client, cfg.pushplus.token.trim(), title, &push_body).await {
            Ok(()) => any_ok = true,
            Err(e) => warn!("alive ping pushplus failed: {}", e),
        }
    }

    Ok(any_ok)
}

fn due_today(now: NaiveTime, target: NaiveTime) -> bool {
    now >= target
}

fn east8_now() -> chrono::DateTime<FixedOffset> {
    let east8 = FixedOffset::east_opt(EAST8_SECS).expect("UTC+8");
    Utc::now().with_timezone(&east8)
}

fn state_file_path(cfg: &config::AppConfig) -> PathBuf {
    let db = cfg.database_path();
    let dir = db.parent().unwrap_or_else(|| std::path::Path::new("data"));
    dir.join("last_alive_ping.date")
}

fn last_sent_date(path: &PathBuf) -> anyhow::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(s) => {
            let line = s.lines().next().unwrap_or("").trim();
            if line.is_empty() {
                Ok(None)
            } else {
                Ok(Some(line.to_string()))
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn write_last_sent_date(path: &PathBuf, date: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, format!("{date}\n"))?;
    Ok(())
}

