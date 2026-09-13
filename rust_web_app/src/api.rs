use crate::checkpoint_db;
use crate::checkpoints;
use crate::checker;
use crate::config;
use crate::feishu;
use crate::alert_history::{AlertLogContext, deliver_feishu, deliver_pushplus};
use crate::models::{
    AlertDelivery, AlertHistoryResponse, CheckHistoryResponse, CheckRun, CheckpointsResponse,
    CreateHealthCheck, CheckpointInput, HealthCheck, ReplaceCheckpointsBody, UpdateHealthCheck,
};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub http: reqwest::Client,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/checks", get(list_checks).post(create_check))
        .route(
            "/api/checks/:id",
            put(update_check).delete(delete_check).get(get_check),
        )
        .route("/api/checks/:id/run", post(run_check_now))
        .route("/api/checks/:id/history", get(list_check_history))
        .route(
            "/api/checks/:id/checkpoints",
            get(list_checkpoints).put(replace_checkpoints),
        )
        .route("/api/feishu/test", post(test_feishu))
        .route("/api/pushplus/test", post(test_pushplus))
        .route("/api/alerts/history", get(list_alert_history))
}

async fn list_checks(State(state): State<AppState>) -> Result<Json<Vec<HealthCheck>>, AppError> {
    let rows = sqlx::query_as::<_, HealthCheck>(
        "SELECT id, name, url, method, expected_status, interval_secs, enabled, \
         last_checked_at, last_status, last_response_ms, last_error, created_at \
         FROM health_checks ORDER BY id",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

async fn get_check(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<HealthCheck>, AppError> {
    let row = sqlx::query_as::<_, HealthCheck>(
        "SELECT id, name, url, method, expected_status, interval_secs, enabled, \
         last_checked_at, last_status, last_response_ms, last_error, created_at \
         FROM health_checks WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

async fn create_check(
    State(state): State<AppState>,
    Json(body): Json<CreateHealthCheck>,
) -> Result<(StatusCode, Json<HealthCheck>), AppError> {
    if body.name.trim().is_empty() || body.url.trim().is_empty() {
        return Err(AppError::BadRequest("name and url are required"));
    }

    let method = body.method.unwrap_or_else(|| "GET".into());
    let expected_status = body.expected_status.unwrap_or(200);
    let interval_secs = body.interval_secs.unwrap_or(60).max(10);
    let enabled = body.enabled.unwrap_or(true);

    let result = sqlx::query(
        "INSERT INTO health_checks (name, url, method, expected_status, interval_secs, enabled) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(body.name.trim())
    .bind(body.url.trim())
    .bind(method)
    .bind(expected_status)
    .bind(interval_secs)
    .bind(enabled)
    .execute(&state.pool)
    .await?;

    let id = result.last_insert_rowid();

    if let Some(cps) = body.checkpoints {
        let normalized = normalize_checkpoint_inputs(cps)?;
        checkpoint_db::replace_for_check(&state.pool, id, &normalized).await?;
    }

    get_check(State(state), Path(id)).await.map(|j| (StatusCode::CREATED, j))
}

async fn update_check(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateHealthCheck>,
) -> Result<Json<HealthCheck>, AppError> {
    let existing = get_check(State(state.clone()), Path(id)).await?.0;

    let name = body.name.unwrap_or(existing.name);
    let url = body.url.unwrap_or(existing.url);
    let method = body.method.unwrap_or(existing.method);
    let expected_status = body.expected_status.unwrap_or(existing.expected_status);
    let interval_secs = body.interval_secs.unwrap_or(existing.interval_secs).max(10);
    let enabled = body.enabled.unwrap_or(existing.enabled);

    sqlx::query(
        "UPDATE health_checks SET name = ?, url = ?, method = ?, expected_status = ?, \
         interval_secs = ?, enabled = ? WHERE id = ?",
    )
    .bind(name)
    .bind(url)
    .bind(method)
    .bind(expected_status)
    .bind(interval_secs)
    .bind(enabled)
    .bind(id)
    .execute(&state.pool)
    .await?;

    if let Some(cps) = body.checkpoints {
        let normalized = normalize_checkpoint_inputs(cps)?;
        checkpoint_db::replace_for_check(&state.pool, id, &normalized).await?;
    }

    get_check(State(state), Path(id)).await
}

fn normalize_checkpoint_inputs(items: Vec<CheckpointInput>) -> Result<Vec<CheckpointInput>, AppError> {
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        if item.value.trim().is_empty() {
            return Err(AppError::BadRequest("检查点预期值不能为空"));
        }
        let kind = checkpoints::normalize_kind(&item.kind).ok_or(AppError::BadRequest(
            "不支持的检查点类型，可选: contains, equals, not_contains, regex, not_regex",
        ))?;
        out.push(CheckpointInput {
            kind,
            value: item.value,
            enabled: item.enabled,
        });
    }
    Ok(out)
}

async fn list_checkpoints(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<CheckpointsResponse>, AppError> {
    let _ = get_check(State(state.clone()), Path(id)).await?;
    let rows = checkpoint_db::list_for_check(&state.pool, id).await?;
    Ok(Json(CheckpointsResponse { checkpoints: rows }))
}

async fn replace_checkpoints(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<ReplaceCheckpointsBody>,
) -> Result<Json<CheckpointsResponse>, AppError> {
    let _ = get_check(State(state.clone()), Path(id)).await?;
    let normalized = normalize_checkpoint_inputs(body.checkpoints)?;
    checkpoint_db::replace_for_check(&state.pool, id, &normalized).await?;
    list_checkpoints(State(state), Path(id)).await
}

#[derive(Debug, Deserialize)]
struct HistoryQuery {
    limit: Option<u32>,
    offset: Option<u32>,
}

async fn list_check_history(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<CheckHistoryResponse>, AppError> {
    let _ = get_check(State(state.clone()), Path(id)).await?;

    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0);

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM check_runs WHERE check_id = ?")
        .bind(id)
        .fetch_one(&state.pool)
        .await?;

    let items = sqlx::query_as::<_, CheckRun>(
        "SELECT id, check_id, status, response_ms, error, checked_at, request_message, \
         response_message FROM check_runs WHERE check_id = ? ORDER BY checked_at DESC \
         LIMIT ? OFFSET ?",
    )
    .bind(id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(CheckHistoryResponse {
        items,
        total: total.0,
        limit,
        offset,
    }))
}

async fn run_check_now(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<HealthCheck>, AppError> {
    let check = get_check(State(state.clone()), Path(id)).await?.0;

    let cfg = config::get();
    checker::execute_health_check(
        &state.pool,
        &state.http,
        &check,
        Some(&cfg.feishu),
        Some(&cfg.pushplus),
    )
    .await;

    get_check(State(state), Path(id)).await
}

async fn list_alert_history(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<AlertHistoryResponse>, AppError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0);

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM alert_deliveries")
        .fetch_one(&state.pool)
        .await?;

    let items = sqlx::query_as::<_, AlertDelivery>(
        "SELECT id, kind, channel, status, title, message, error, check_id, check_name, sent_at \
         FROM alert_deliveries ORDER BY sent_at DESC LIMIT ? OFFSET ?",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(AlertHistoryResponse {
        items,
        total: total.0,
        limit,
        offset,
    }))
}

async fn delete_check(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let result = sqlx::query("DELETE FROM health_checks WHERE id = ?")
        .bind(id)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn test_feishu(State(state): State<AppState>) -> Result<StatusCode, AppError> {
    let cfg = config::get();
    if !cfg.feishu.enabled {
        return Err(AppError::BadRequest("config.toml 中飞书告警未启用"));
    }
    let webhook_url = cfg.feishu.webhook_url.trim();
    if webhook_url.is_empty() {
        return Err(AppError::BadRequest("config.toml 中未配置飞书 webhook_url"));
    }

    let text = format!(
        "【测试消息】Are You Healthy 飞书告警通道正常。\n时间: {}",
        feishu::format_time_east8()
    );

    let ctx = AlertLogContext {
        kind: "test",
        check_id: None,
        check_name: None,
    };
    if !deliver_feishu(&state.pool, &state.http, &ctx, webhook_url, &text).await {
        return Err(AppError::Upstream("飞书发送失败，详见告警历史".into()));
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn test_pushplus(State(state): State<AppState>) -> Result<StatusCode, AppError> {
    let cfg = config::get();
    if !cfg.pushplus.enabled {
        return Err(AppError::BadRequest("config.toml 中 PushPlus 告警未启用"));
    }
    let token = cfg.pushplus.token.trim();
    if token.is_empty() {
        return Err(AppError::BadRequest("config.toml 中未配置 pushplus token"));
    }

    let title = "Are You Healthy 测试消息";
    let content = format!(
        "PushPlus 告警通道正常。\n时间: {}",
        feishu::format_time_east8()
    );

    let ctx = AlertLogContext {
        kind: "test",
        check_id: None,
        check_name: None,
    };
    if !deliver_pushplus(&state.pool, &state.http, &ctx, token, title, &content).await {
        return Err(AppError::Upstream("PushPlus 发送失败，详见告警历史".into()));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub enum AppError {
    NotFound,
    BadRequest(&'static str),
    Upstream(String),
    Db(sqlx::Error),
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::Db(e)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "not found").into_response(),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg).into_response(),
            AppError::Upstream(msg) => (StatusCode::BAD_GATEWAY, msg).into_response(),
            AppError::Db(e) => {
                tracing::error!("db error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
            }
        }
    }
}
