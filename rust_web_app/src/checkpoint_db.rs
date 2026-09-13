use crate::checkpoints::CheckpointRule;
use crate::models::CheckCheckpoint;
use sqlx::SqlitePool;

pub async fn list_for_check(pool: &SqlitePool, check_id: i64) -> Result<Vec<CheckCheckpoint>, sqlx::Error> {
    sqlx::query_as::<_, CheckCheckpoint>(
        "SELECT id, check_id, kind, value, enabled, sort_order \
         FROM check_checkpoints WHERE check_id = ? ORDER BY sort_order, id",
    )
    .bind(check_id)
    .fetch_all(pool)
    .await
}

pub async fn load_enabled_rules(pool: &SqlitePool, check_id: i64) -> Vec<CheckpointRule> {
    let rows = list_for_check(pool, check_id).await.unwrap_or_default();
    rows.into_iter()
        .filter(|r| r.enabled)
        .map(|r| CheckpointRule {
            kind: r.kind,
            value: r.value,
        })
        .collect()
}

pub async fn replace_for_check(
    pool: &SqlitePool,
    check_id: i64,
    items: &[crate::models::CheckpointInput],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM check_checkpoints WHERE check_id = ?")
        .bind(check_id)
        .execute(pool)
        .await?;

    for (i, item) in items.iter().enumerate() {
        let enabled = item.enabled.unwrap_or(true);
        sqlx::query(
            "INSERT INTO check_checkpoints (check_id, kind, value, enabled, sort_order) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(check_id)
        .bind(&item.kind)
        .bind(item.value.trim())
        .bind(enabled)
        .bind(i as i64)
        .execute(pool)
        .await?;
    }

    Ok(())
}
