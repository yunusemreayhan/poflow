use super::*;

#[derive(Debug, Clone, FromRow, serde::Serialize, utoipa::ToSchema)]
pub struct Notification {
    pub id: i64,
    pub user_id: i64,
    pub kind: String,
    pub message: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<i64>,
    pub read: bool,
    pub created_at: String,
}

pub async fn create_notification(pool: &Pool, user_id: i64, kind: &str, message: &str, entity_type: Option<&str>, entity_id: Option<i64>) -> Result<()> {
    // B1: Check notification_prefs before creating — skip if user disabled this event type
    let disabled: Option<(bool,)> = sqlx::query_as("SELECT enabled FROM notification_prefs WHERE user_id = ? AND event_type = ?")
        .bind(user_id).bind(kind).fetch_optional(pool).await?;
    if let Some((false,)) = disabled { return Ok(()); }
    // V31-6: Cap at 500 unread notifications per user — trim oldest if exceeded
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notifications WHERE user_id = ? AND read = 0")
        .bind(user_id).fetch_one(pool).await?;
    if count >= 500 {
        sqlx::query("DELETE FROM notifications WHERE user_id = ? AND id IN (SELECT id FROM notifications WHERE user_id = ? AND read = 0 ORDER BY created_at ASC LIMIT 50)")
            .bind(user_id).bind(user_id).execute(pool).await?;
    }
    sqlx::query("INSERT INTO notifications (user_id, kind, message, entity_type, entity_id, created_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(user_id).bind(kind).bind(message).bind(entity_type).bind(entity_id).bind(&now_str())
        .execute(pool).await?;
    Ok(())
}

pub async fn list_notifications(pool: &Pool, user_id: i64, limit: i64) -> Result<Vec<Notification>> {
    Ok(sqlx::query_as::<_, Notification>("SELECT * FROM notifications WHERE user_id = ? ORDER BY created_at DESC LIMIT ?")
        .bind(user_id).bind(limit).fetch_all(pool).await?)
}

pub async fn mark_read(pool: &Pool, user_id: i64, id: Option<i64>) -> Result<()> {
    if let Some(id) = id {
        sqlx::query("UPDATE notifications SET read = 1 WHERE id = ? AND user_id = ?").bind(id).bind(user_id).execute(pool).await?;
    } else {
        sqlx::query("UPDATE notifications SET read = 1 WHERE user_id = ?").bind(user_id).execute(pool).await?;
    }
    Ok(())
}

pub async fn unread_count(pool: &Pool, user_id: i64) -> Result<i64> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notifications WHERE user_id = ? AND read = 0")
        .bind(user_id).fetch_one(pool).await?;
    Ok(count)
}

// B8: Cleanup old notifications (keep last 200 per user, delete read older than 30 days)
pub async fn cleanup_notifications(pool: &Pool) -> Result<u64> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(30)).format("%Y-%m-%dT%H:%M:%S%.3f").to_string();
    let result = sqlx::query("DELETE FROM notifications WHERE read = 1 AND created_at < ?")
        .bind(&cutoff).execute(pool).await?;
    Ok(result.rows_affected())
}
