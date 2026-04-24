use super::*;

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Attachment {
    pub id: i64,
    pub task_id: i64,
    pub user_id: i64,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub storage_key: String,
    pub created_at: String,
}

pub async fn list_attachments(pool: &Pool, task_id: i64) -> Result<Vec<Attachment>> {
    Ok(
        sqlx::query_as("SELECT * FROM task_attachments WHERE task_id = ? ORDER BY created_at DESC")
            .bind(task_id)
            .fetch_all(pool)
            .await?,
    )
}

pub async fn create_attachment(
    pool: &Pool,
    task_id: i64,
    user_id: i64,
    filename: &str,
    mime_type: &str,
    size_bytes: i64,
    storage_key: &str,
) -> Result<Attachment> {
    let now = now_str();
    let id = sqlx::query("INSERT INTO task_attachments (task_id, user_id, filename, mime_type, size_bytes, storage_key, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(task_id).bind(user_id).bind(filename).bind(mime_type).bind(size_bytes).bind(storage_key).bind(&now)
        .execute(pool).await?.last_insert_rowid();
    Ok(
        sqlx::query_as("SELECT * FROM task_attachments WHERE id = ?")
            .bind(id)
            .fetch_one(pool)
            .await?,
    )
}

pub async fn get_attachment(pool: &Pool, id: i64) -> Result<Attachment> {
    Ok(
        sqlx::query_as("SELECT * FROM task_attachments WHERE id = ?")
            .bind(id)
            .fetch_one(pool)
            .await?,
    )
}

pub async fn delete_attachment(pool: &Pool, id: i64) -> Result<String> {
    let att: Attachment = sqlx::query_as("SELECT * FROM task_attachments WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    sqlx::query("DELETE FROM task_attachments WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(att.storage_key)
}

/// Get the attachments storage directory
pub fn attachments_dir() -> std::path::PathBuf {
    let dir = super::data_dir().join("attachments");
    std::fs::create_dir_all(&dir).ok();
    dir
}

/// O3: Clean up orphaned attachment files (files on disk without DB records)
pub async fn cleanup_orphaned_attachments(pool: &Pool) -> Result<u64> {
    let dir = attachments_dir();
    let db_keys: Vec<(String,)> = sqlx::query_as("SELECT storage_key FROM task_attachments")
        .fetch_all(pool)
        .await?;
    let db_set: std::collections::HashSet<String> = db_keys.into_iter().map(|(k,)| k).collect();
    let mut removed = 0u64;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !db_set.contains(&name) && std::fs::remove_file(entry.path()).is_ok() {
                removed += 1;
            }
        }
    }
    Ok(removed)
}
