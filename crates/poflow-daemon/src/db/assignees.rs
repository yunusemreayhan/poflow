use super::*;


// --- Assignees ---

pub async fn add_assignee(pool: &Pool, task_id: i64, user_id: i64) -> Result<()> {
    sqlx::query("INSERT OR IGNORE INTO task_assignees (task_id, user_id) VALUES (?, ?)")
        .bind(task_id).bind(user_id).execute(pool).await?;
    Ok(())
}

pub async fn remove_assignee(pool: &Pool, task_id: i64, user_id: i64) -> Result<()> {
    let r = sqlx::query("DELETE FROM task_assignees WHERE task_id = ? AND user_id = ?")
        .bind(task_id).bind(user_id).execute(pool).await?;
    if r.rows_affected() == 0 { return Err(anyhow::anyhow!("User not assigned")); }
    Ok(())
}

pub async fn list_assignees(pool: &Pool, task_id: i64) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT u.username FROM task_assignees ta JOIN users u ON ta.user_id = u.id WHERE ta.task_id = ? ORDER BY u.username")
        .bind(task_id).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(u,)| u).collect())
}

pub async fn get_user_id_by_username(pool: &Pool, username: &str) -> Result<Option<i64>> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT id FROM users WHERE username = ?").bind(username)
        .fetch_optional(pool).await?;
    Ok(row.map(|(id,)| id))
}

// --- Room CRUD ---
