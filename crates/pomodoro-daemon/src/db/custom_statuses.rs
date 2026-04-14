use super::*;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct CustomStatus {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub category: String, // "todo", "in_progress", "done"
    pub sort_order: i64,
    pub created_by: i64,
    pub created_at: String,
}

pub async fn list_custom_statuses(pool: &Pool) -> Result<Vec<CustomStatus>> {
    Ok(sqlx::query_as::<_, CustomStatus>("SELECT * FROM custom_statuses ORDER BY sort_order, id")
        .fetch_all(pool).await?)
}

pub async fn create_custom_status(pool: &Pool, name: &str, color: &str, category: &str, sort_order: i64, created_by: i64) -> Result<CustomStatus> {
    let now = now_str();
    let id = sqlx::query("INSERT INTO custom_statuses (name, color, category, sort_order, created_by, created_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(name).bind(color).bind(category).bind(sort_order).bind(created_by).bind(&now)
        .execute(pool).await?.last_insert_rowid();
    Ok(CustomStatus { id, name: name.to_string(), color: color.to_string(), category: category.to_string(), sort_order, created_by, created_at: now })
}

pub async fn update_custom_status(pool: &Pool, id: i64, name: &str, color: &str, category: &str, sort_order: i64) -> Result<CustomStatus> {
    sqlx::query("UPDATE custom_statuses SET name = ?, color = ?, category = ?, sort_order = ? WHERE id = ?")
        .bind(name).bind(color).bind(category).bind(sort_order).bind(id)
        .execute(pool).await?;
    Ok(sqlx::query_as::<_, CustomStatus>("SELECT * FROM custom_statuses WHERE id = ?")
        .bind(id).fetch_one(pool).await?)
}

pub async fn delete_custom_status(pool: &Pool, id: i64) -> Result<()> {
    let result = sqlx::query("DELETE FROM custom_statuses WHERE id = ?").bind(id).execute(pool).await?;
    if result.rows_affected() == 0 { return Err(anyhow::anyhow!("not found")); }
    Ok(())
}

pub async fn get_custom_status_by_name(pool: &Pool, name: &str) -> Result<Option<CustomStatus>> {
    Ok(sqlx::query_as::<_, CustomStatus>("SELECT * FROM custom_statuses WHERE name = ?")
        .bind(name).fetch_optional(pool).await?)
}
