use super::*;

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize)]
pub struct TaskRecurrence {
    pub task_id: i64,
    pub pattern: String, // "daily", "weekly", "biweekly", "monthly"
    pub next_due: String,
    pub last_created: Option<String>,
}

pub async fn set_recurrence(
    pool: &Pool,
    task_id: i64,
    pattern: &str,
    next_due: &str,
) -> Result<TaskRecurrence> {
    sqlx::query("INSERT INTO task_recurrence (task_id, pattern, next_due) VALUES (?,?,?) ON CONFLICT(task_id) DO UPDATE SET pattern=excluded.pattern, next_due=excluded.next_due")
        .bind(task_id).bind(pattern).bind(next_due).execute(pool).await?;
    Ok(
        sqlx::query_as::<_, TaskRecurrence>("SELECT * FROM task_recurrence WHERE task_id = ?")
            .bind(task_id)
            .fetch_one(pool)
            .await?,
    )
}

pub async fn remove_recurrence(pool: &Pool, task_id: i64) -> Result<()> {
    let r = sqlx::query("DELETE FROM task_recurrence WHERE task_id = ?")
        .bind(task_id)
        .execute(pool)
        .await?;
    if r.rows_affected() == 0 {
        return Err(anyhow::anyhow!("No recurrence set"));
    }
    Ok(())
}

pub async fn get_recurrence(pool: &Pool, task_id: i64) -> Result<Option<TaskRecurrence>> {
    Ok(
        sqlx::query_as::<_, TaskRecurrence>("SELECT * FROM task_recurrence WHERE task_id = ?")
            .bind(task_id)
            .fetch_optional(pool)
            .await?,
    )
}

pub async fn get_due_recurrences(pool: &Pool, before: &str) -> Result<Vec<TaskRecurrence>> {
    Ok(
        sqlx::query_as::<_, TaskRecurrence>("SELECT * FROM task_recurrence WHERE next_due <= ?")
            .bind(before)
            .fetch_all(pool)
            .await?,
    )
}

pub async fn advance_recurrence(pool: &Pool, task_id: i64, next_due: &str) -> Result<()> {
    let now = now_str();
    sqlx::query("UPDATE task_recurrence SET next_due = ?, last_created = ? WHERE task_id = ?")
        .bind(next_due)
        .bind(&now)
        .bind(task_id)
        .execute(pool)
        .await?;
    Ok(())
}
