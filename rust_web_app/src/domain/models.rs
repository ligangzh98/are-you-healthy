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
    pub checkpoints: Option<Vec<CheckpointInput>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateHealthCheck {
    pub name: Option<String>,
    pub url: Option<String>,
    pub method: Option<String>,
    pub expected_status: Option<i64>,
    pub interval_secs: Option<i64>,
    pub enabled: Option<bool>,
    pub checkpoints: Option<Vec<CheckpointInput>>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct CheckCheckpoint {
    pub id: i64,
    pub check_id: i64,
    pub kind: String,
    pub value: String,
    pub enabled: bool,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CheckpointInput {
    pub kind: String,
    pub value: String,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct CheckpointsResponse {
    pub checkpoints: Vec<CheckCheckpoint>,
}

#[derive(Debug, Deserialize)]
pub struct ReplaceCheckpointsBody {
    pub checkpoints: Vec<CheckpointInput>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct CheckRun {
    pub id: i64,
    pub check_id: i64,
    pub status: String,
    pub response_ms: Option<i64>,
    pub error: Option<String>,
    pub checked_at: String,
    pub request_message: String,
    pub response_message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CheckHistoryResponse {
    pub items: Vec<CheckRun>,
    pub total: i64,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct AlertDelivery {
    pub id: i64,
    pub kind: String,
    pub channel: String,
    pub status: String,
    pub title: Option<String>,
    pub message: String,
    pub error: Option<String>,
    pub check_id: Option<i64>,
    pub check_name: Option<String>,
    pub sent_at: String,
}

#[derive(Debug, Serialize)]
pub struct AlertHistoryResponse {
    pub items: Vec<AlertDelivery>,
    pub total: i64,
    pub limit: u32,
    pub offset: u32,
}
