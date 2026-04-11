use super::*;

#[derive(Debug, Clone, FromRow, serde::Serialize)]
pub struct Webhook {
    pub id: i64,
    pub user_id: i64,
    pub url: String,
    pub events: String,
    pub secret: Option<String>,
    pub active: i64,
    pub created_at: String,
}

pub async fn list_webhooks(pool: &Pool, user_id: i64) -> Result<Vec<Webhook>> {
    Ok(sqlx::query_as::<_, Webhook>("SELECT * FROM webhooks WHERE user_id = ? ORDER BY id").bind(user_id).fetch_all(pool).await?)
}

pub async fn create_webhook(pool: &Pool, user_id: i64, url: &str, events: &str, secret: Option<&str>) -> Result<Webhook> {
    let now = now_str();
    let id = sqlx::query("INSERT INTO webhooks (user_id, url, events, secret, created_at) VALUES (?,?,?,?,?)")
        .bind(user_id).bind(url).bind(events).bind(secret).bind(&now)
        .execute(pool).await?.last_insert_rowid();
    Ok(sqlx::query_as::<_, Webhook>("SELECT * FROM webhooks WHERE id = ?").bind(id).fetch_one(pool).await?)
}

pub async fn delete_webhook(pool: &Pool, id: i64, user_id: i64) -> Result<()> {
    sqlx::query("DELETE FROM webhooks WHERE id = ? AND user_id = ?").bind(id).bind(user_id).execute(pool).await?;
    Ok(())
}

pub async fn get_active_webhooks(pool: &Pool, event: &str) -> Result<Vec<Webhook>> {
    Ok(sqlx::query_as::<_, Webhook>("SELECT * FROM webhooks WHERE active = 1 AND (events = '*' OR events LIKE ?)")
        .bind(format!("%{}%", event)).fetch_all(pool).await?)
}
