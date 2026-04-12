use super::*;

pub async fn watch_task(pool: &Pool, task_id: i64, user_id: i64) -> Result<()> {
    sqlx::query("INSERT OR IGNORE INTO task_watchers (task_id, user_id, created_at) VALUES (?,?,?)")
        .bind(task_id).bind(user_id).bind(&now_str()).execute(pool).await?;
    Ok(())
}

pub async fn unwatch_task(pool: &Pool, task_id: i64, user_id: i64) -> Result<()> {
    sqlx::query("DELETE FROM task_watchers WHERE task_id = ? AND user_id = ?")
        .bind(task_id).bind(user_id).execute(pool).await?;
    Ok(())
}

pub async fn get_task_watchers(pool: &Pool, task_id: i64) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT u.username FROM task_watchers tw JOIN users u ON tw.user_id = u.id WHERE tw.task_id = ? ORDER BY tw.created_at")
        .bind(task_id).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

pub async fn get_watched_tasks(pool: &Pool, user_id: i64) -> Result<Vec<i64>> {
    let rows: Vec<(i64,)> = sqlx::query_as("SELECT task_id FROM task_watchers WHERE user_id = ?")
        .bind(user_id).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}
