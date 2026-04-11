use super::*;

#[derive(Debug, Clone, FromRow, serde::Serialize)]
pub struct Label {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub created_at: String,
}

pub async fn list_labels(pool: &Pool) -> Result<Vec<Label>> {
    Ok(sqlx::query_as::<_, Label>("SELECT * FROM labels ORDER BY name").fetch_all(pool).await?)
}

pub async fn create_label(pool: &Pool, name: &str, color: &str) -> Result<Label> {
    let now = now_str();
    let id = sqlx::query("INSERT INTO labels (name, color, created_at) VALUES (?, ?, ?)")
        .bind(name).bind(color).bind(&now).execute(pool).await?.last_insert_rowid();
    Ok(sqlx::query_as::<_, Label>("SELECT * FROM labels WHERE id = ?").bind(id).fetch_one(pool).await?)
}

pub async fn delete_label(pool: &Pool, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM labels WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

pub async fn add_task_label(pool: &Pool, task_id: i64, label_id: i64) -> Result<()> {
    sqlx::query("INSERT OR IGNORE INTO task_labels (task_id, label_id) VALUES (?, ?)")
        .bind(task_id).bind(label_id).execute(pool).await?;
    Ok(())
}

pub async fn remove_task_label(pool: &Pool, task_id: i64, label_id: i64) -> Result<()> {
    sqlx::query("DELETE FROM task_labels WHERE task_id = ? AND label_id = ?")
        .bind(task_id).bind(label_id).execute(pool).await?;
    Ok(())
}

pub async fn get_task_labels(pool: &Pool, task_id: i64) -> Result<Vec<Label>> {
    Ok(sqlx::query_as::<_, Label>(
        "SELECT l.* FROM labels l JOIN task_labels tl ON l.id = tl.label_id WHERE tl.task_id = ? ORDER BY l.name"
    ).bind(task_id).fetch_all(pool).await?)
}
