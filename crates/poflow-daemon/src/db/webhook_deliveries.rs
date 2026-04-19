use super::*;

#[derive(Debug, Clone, FromRow, serde::Serialize, utoipa::ToSchema)]
pub struct WebhookDelivery {
    pub id: i64,
    pub webhook_id: i64,
    pub event: String,
    pub status_code: Option<i64>,
    pub success: bool,
    pub attempts: i64,
    pub error: Option<String>,
    pub created_at: String,
}

pub async fn log_delivery(pool: &Pool, webhook_id: i64, event: &str, status_code: Option<u16>, success: bool, attempts: u32, error: Option<&str>) -> Result<()> {
    sqlx::query("INSERT INTO webhook_deliveries (webhook_id, event, status_code, success, attempts, error, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(webhook_id).bind(event).bind(status_code.map(|c| c as i64)).bind(success).bind(attempts as i64).bind(error).bind(now_str())
        .execute(pool).await?;
    Ok(())
}

pub async fn list_deliveries(pool: &Pool, webhook_id: i64, limit: i64) -> Result<Vec<WebhookDelivery>> {
    Ok(sqlx::query_as::<_, WebhookDelivery>("SELECT * FROM webhook_deliveries WHERE webhook_id = ? ORDER BY created_at DESC LIMIT ?")
        .bind(webhook_id).bind(limit).fetch_all(pool).await?)
}
