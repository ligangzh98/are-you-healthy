use crate::checkpoint_db;
use crate::checkpoints;
use crate::checker;
use crate::feishu;
use crate::models::{
    CheckHistoryResponse, CheckRun, CheckpointsResponse, CreateHealthCheck,
    CheckpointInput, FeishuConfig, HealthCheck, PushplusConfig, ReplaceCheckpointsBody,
    TestFeishuRequest, TestPushplusRequest, UpdateFeishuConfig, UpdateHealthCheck,
    UpdatePushplusConfig,
};
use crate::pushplus;
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
        .route("/api/feishu", get(get_feishu).put(update_feishu))
        .route("/api/feishu/test", post(test_feishu))
        .route("/api/pushplus", get(get_pushplus).put(update_pushplus))
        .route("/api/pushplus/test", post(test_pushplus))
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

    let feishu = checker::load_feishu_config(&state.pool).await;
    let pushplus = checker::load_pushplus_config(&state.pool).await;
    checker::execute_health_check(
        &state.pool,
        &state.http,
        &check,
        feishu.as_ref(),
        pushplus.as_ref(),
    )
    .await;

    get_check(State(state), Path(id)).await
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

async fn get_feishu(State(state): State<AppState>) -> Result<Json<FeishuConfig>, AppError> {
    let row = sqlx::query_as::<_, FeishuConfig>(
        "SELECT id, webhook_url, enabled, alert_cooldown_secs FROM feishu_config WHERE id = 1",
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

async fn update_feishu(
    State(state): State<AppState>,
    Json(body): Json<UpdateFeishuConfig>,
) -> Result<Json<FeishuConfig>, AppError> {
    let existing = get_feishu(State(state.clone())).await?.0;

    let webhook_url = body.webhook_url.unwrap_or(existing.webhook_url);
    let enabled = body.enabled.unwrap_or(existing.enabled);
    let alert_cooldown_secs = body
        .alert_cooldown_secs
        .unwrap_or(existing.alert_cooldown_secs)
        .max(60);

    sqlx::query(
        "UPDATE feishu_config SET webhook_url = ?, enabled = ?, alert_cooldown_secs = ? WHERE id = 1",
    )
    .bind(webhook_url)
    .bind(enabled)
    .bind(alert_cooldown_secs)
    .execute(&state.pool)
    .await?;

    get_feishu(State(state)).await
}

async fn test_feishu(
    State(state): State<AppState>,
    Json(body): Json<TestFeishuRequest>,
) -> Result<StatusCode, AppError> {
    let webhook_url = match body
        .webhook_url
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        Some(url) => url,
        None => get_feishu(State(state.clone())).await?.0.webhook_url,
    };

    if webhook_url.is_empty() {
        return Err(AppError::BadRequest("请先填写 Webhook URL"));
    }

    let text = format!(
        "【测试消息】Are You Healthy 飞书告警通道正常。\n时间: {}",
        feishu::format_time_east8()
    );

    feishu::send_text_alert(&state.http, &webhook_url, &text)
        .await
        .map_err(|e| AppError::Upstream(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

async fn get_pushplus(State(state): State<AppState>) -> Result<Json<PushplusConfig>, AppError> {
    let row = sqlx::query_as::<_, PushplusConfig>(
        "SELECT id, token, enabled, alert_cooldown_secs FROM pushplus_config WHERE id = 1",
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

async fn update_pushplus(
    State(state): State<AppState>,
    Json(body): Json<UpdatePushplusConfig>,
) -> Result<Json<PushplusConfig>, AppError> {
    let existing = get_pushplus(State(state.clone())).await?.0;

    let token = body.token.unwrap_or(existing.token);
    let enabled = body.enabled.unwrap_or(existing.enabled);
    let alert_cooldown_secs = body
        .alert_cooldown_secs
        .unwrap_or(existing.alert_cooldown_secs)
        .max(60);

    sqlx::query(
        "UPDATE pushplus_config SET token = ?, enabled = ?, alert_cooldown_secs = ? WHERE id = 1",
    )
    .bind(token)
    .bind(enabled)
    .bind(alert_cooldown_secs)
    .execute(&state.pool)
    .await?;

    get_pushplus(State(state)).await
}

async fn test_pushplus(
    State(state): State<AppState>,
    Json(body): Json<TestPushplusRequest>,
) -> Result<StatusCode, AppError> {
    let token = match body
        .token
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        Some(t) => t,
        None => get_pushplus(State(state.clone())).await?.0.token,
    };

    if token.is_empty() {
        return Err(AppError::BadRequest("请先填写 PushPlus Token"));
    }

    let title = "Are You Healthy 测试消息";
    let content = format!(
        "PushPlus 告警通道正常。\n时间: {}",
        feishu::format_time_east8()
    );

    pushplus::send_message(&state.http, &token, title, &content)
        .await
        .map_err(|e| AppError::Upstream(e.to_string()))?;

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
