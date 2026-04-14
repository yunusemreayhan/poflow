use super::*;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct ChecklistItem {
    pub id: i64,
    pub task_id: i64,
    pub title: String,
    pub checked: bool,
    pub sort_order: i64,
    pub created_at: String,
}

pub async fn list_checklist(pool: &Pool, task_id: i64) -> Result<Vec<ChecklistItem>> {
    Ok(sqlx::query_as::<_, ChecklistItem>("SELECT * FROM checklist_items WHERE task_id = ? ORDER BY sort_order, id")
        .bind(task_id).fetch_all(pool).await?)
}

pub async fn add_checklist_item(pool: &Pool, task_id: i64, title: &str, sort_order: i64) -> Result<ChecklistItem> {
    let now = now_str();
    let id = sqlx::query("INSERT INTO checklist_items (task_id, title, sort_order, created_at) VALUES (?, ?, ?, ?)")
        .bind(task_id).bind(title).bind(sort_order).bind(&now)
        .execute(pool).await?.last_insert_rowid();
    Ok(ChecklistItem { id, task_id, title: title.to_string(), checked: false, sort_order, created_at: now })
}

pub async fn update_checklist_item(pool: &Pool, id: i64, title: Option<&str>, checked: Option<bool>, sort_order: Option<i64>) -> Result<ChecklistItem> {
    if let Some(t) = title { sqlx::query("UPDATE checklist_items SET title = ? WHERE id = ?").bind(t).bind(id).execute(pool).await?; }
    if let Some(c) = checked { sqlx::query("UPDATE checklist_items SET checked = ? WHERE id = ?").bind(c).bind(id).execute(pool).await?; }
    if let Some(s) = sort_order { sqlx::query("UPDATE checklist_items SET sort_order = ? WHERE id = ?").bind(s).bind(id).execute(pool).await?; }
    Ok(sqlx::query_as::<_, ChecklistItem>("SELECT * FROM checklist_items WHERE id = ?").bind(id).fetch_one(pool).await?)
}

pub async fn delete_checklist_item(pool: &Pool, id: i64) -> Result<()> {
    let r = sqlx::query("DELETE FROM checklist_items WHERE id = ?").bind(id).execute(pool).await?;
    if r.rows_affected() == 0 { return Err(anyhow::anyhow!("not found")); }
    Ok(())
}
