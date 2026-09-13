use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct HealthCheck {
    pub id: i64,
    pub name: String,
    pub url: String,
    pub method: String,
    pub expected_status: i64,
    pub interval_secs: i64,
    pub enabled: bool,
    pub last_checked_at: Option<String>,
    pub last_status: Option<String>,
    pub last_response_ms: Option<i64>,
    pub last_error: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateHealthCheck {
    pub name: String,
    pub url: String,
    pub method: Option<String>,
    pub expected_status: Option<i64>,
    pub interval_secs: Option<i64>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateHealthCheck {
    pub name: Option<String>,
    pub url: Option<String>,
    pub method: Option<String>,
    pub expected_status: Option<i64>,
    pub interval_secs: Option<i64>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct FeishuConfig {
    pub id: i64,
    pub webhook_url: String,
    pub enabled: bool,
    pub alert_cooldown_secs: i64,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFeishuConfig {
    pub webhook_url: Option<String>,
    pub enabled: Option<bool>,
    pub alert_cooldown_secs: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct TestFeishuRequest {
    pub webhook_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct CheckRun {
    pub id: i64,
    pub check_id: i64,
    pub status: String,
    pub response_ms: Option<i64>,
    pub error: Option<String>,
    pub checked_at: String,
}

#[derive(Debug, Serialize)]
pub struct CheckHistoryResponse {
    pub items: Vec<CheckRun>,
    pub total: i64,
    pub limit: u32,
    pub offset: u32,
}
