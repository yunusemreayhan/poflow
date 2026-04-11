use super::*;


// --- Assignees ---

pub async fn add_assignee(pool: &Pool, task_id: i64, user_id: i64) -> Result<()> {
    sqlx::query("INSERT OR IGNORE INTO task_assignees (task_id, user_id) VALUES (?, ?)")
        .bind(task_id).bind(user_id).execute(pool).await?;
    Ok(())
}

pub async fn remove_assignee(pool: &Pool, task_id: i64, user_id: i64) -> Result<()> {
    sqlx::query("DELETE FROM task_assignees WHERE task_id = ? AND user_id = ?")
        .bind(task_id).bind(user_id).execute(pool).await?;
    Ok(())
}

pub async fn list_assignees(pool: &Pool, task_id: i64) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT u.username FROM task_assignees ta JOIN users u ON ta.user_id = u.id WHERE ta.task_id = ? ORDER BY u.username")
        .bind(task_id).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(u,)| u).collect())
}

pub async fn get_user_id_by_username(pool: &Pool, username: &str) -> Result<i64> {
    let (id,): (i64,) = sqlx::query_as("SELECT id FROM users WHERE username = ?").bind(username).fetch_one(pool).await?;
    Ok(id)
}

// --- Room CRUD ---
