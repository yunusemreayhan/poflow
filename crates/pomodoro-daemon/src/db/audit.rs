use super::*;

#[derive(Debug, Clone, FromRow, serde::Serialize)]
pub struct AuditEntry {
    pub id: i64,
    pub user_id: i64,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<i64>,
    pub detail: Option<String>,
    pub created_at: String,
}

pub async fn audit(pool: &Pool, user_id: i64, action: &str, entity_type: &str, entity_id: Option<i64>, detail: Option<&str>) -> Result<()> {
    sqlx::query("INSERT INTO audit_log (user_id, action, entity_type, entity_id, detail, created_at) VALUES (?,?,?,?,?,?)")
        .bind(user_id).bind(action).bind(entity_type).bind(entity_id).bind(detail).bind(&now_str())
        .execute(pool).await?;
    Ok(())
}

pub async fn list_audit(pool: &Pool, entity_type: Option<&str>, entity_id: Option<i64>, limit: i64, offset: i64) -> Result<Vec<AuditEntry>> {
    let mut q = "SELECT * FROM audit_log WHERE 1=1".to_string();
    if entity_type.is_some() { q.push_str(" AND entity_type = ?"); }
    if entity_id.is_some() { q.push_str(" AND entity_id = ?"); }
    q.push_str(" ORDER BY id DESC LIMIT ? OFFSET ?");
    let mut query = sqlx::query_as::<_, AuditEntry>(&q);
    if let Some(t) = entity_type { query = query.bind(t); }
    if let Some(id) = entity_id { query = query.bind(id); }
    query = query.bind(limit).bind(offset);
    Ok(query.fetch_all(pool).await?)
}
