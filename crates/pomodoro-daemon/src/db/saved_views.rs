use super::*;

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct SavedView {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub filters: String,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn list_saved_views(pool: &Pool, user_id: i64) -> Result<Vec<SavedView>> {
    Ok(sqlx::query_as::<_, SavedView>("SELECT * FROM saved_views WHERE user_id = ? ORDER BY updated_at DESC")
        .bind(user_id).fetch_all(pool).await?)
}

pub async fn create_saved_view(pool: &Pool, user_id: i64, name: &str, filters: &str) -> Result<SavedView> {
    let now = now_str();
    let id = sqlx::query("INSERT INTO saved_views (user_id, name, filters, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
        .bind(user_id).bind(name).bind(filters).bind(&now).bind(&now)
        .execute(pool).await?.last_insert_rowid();
    Ok(sqlx::query_as::<_, SavedView>("SELECT * FROM saved_views WHERE id = ?").bind(id).fetch_one(pool).await?)
}

pub async fn update_saved_view(pool: &Pool, id: i64, user_id: i64, name: &str, filters: &str) -> Result<SavedView> {
    let r = sqlx::query("UPDATE saved_views SET name = ?, filters = ?, updated_at = ? WHERE id = ? AND user_id = ?")
        .bind(name).bind(filters).bind(now_str()).bind(id).bind(user_id)
        .execute(pool).await?;
    if r.rows_affected() == 0 { return Err(anyhow::anyhow!("Not found")); }
    Ok(sqlx::query_as::<_, SavedView>("SELECT * FROM saved_views WHERE id = ?").bind(id).fetch_one(pool).await?)
}

pub async fn delete_saved_view(pool: &Pool, id: i64, user_id: i64) -> Result<()> {
    let r = sqlx::query("DELETE FROM saved_views WHERE id = ? AND user_id = ?")
        .bind(id).bind(user_id).execute(pool).await?;
    if r.rows_affected() == 0 { return Err(anyhow::anyhow!("Not found")); }
    Ok(())
}
