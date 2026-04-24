use super::*;

#[derive(Debug, Clone, FromRow)]
pub struct TimerStateRow {
    pub user_id: i64,
    pub phase: String,
    pub status: String,
    pub elapsed_s: i64,
    pub duration_s: i64,
    pub session_count: i64,
    pub current_task_id: Option<i64>,
    pub current_session_id: Option<i64>,
    pub daily_completed: i64,
    pub daily_goal: i64,
    pub updated_at: String,
}

pub struct SaveTimerState<'a> {
    pub user_id: i64,
    pub phase: &'a str,
    pub status: &'a str,
    pub elapsed_s: u32,
    pub duration_s: u32,
    pub session_count: u32,
    pub task_id: Option<i64>,
    pub session_id: Option<i64>,
    pub daily_completed: i64,
    pub daily_goal: u32,
}

pub async fn save_timer_state(pool: &Pool, s: SaveTimerState<'_>) -> Result<()> {
    sqlx::query(
        "INSERT INTO timer_states (user_id, phase, status, elapsed_s, duration_s, session_count, current_task_id, current_session_id, daily_completed, daily_goal, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(user_id) DO UPDATE SET phase=excluded.phase, status=excluded.status, elapsed_s=excluded.elapsed_s, duration_s=excluded.duration_s, session_count=excluded.session_count, current_task_id=excluded.current_task_id, current_session_id=excluded.current_session_id, daily_completed=excluded.daily_completed, daily_goal=excluded.daily_goal, updated_at=excluded.updated_at"
    )
    .bind(s.user_id).bind(s.phase).bind(s.status)
    .bind(s.elapsed_s as i64).bind(s.duration_s as i64).bind(s.session_count as i64)
    .bind(s.task_id).bind(s.session_id)
    .bind(s.daily_completed).bind(s.daily_goal as i64)
    .bind(now_str())
    .execute(pool).await?;
    Ok(())
}

pub async fn load_timer_states(pool: &Pool) -> Result<Vec<TimerStateRow>> {
    Ok(sqlx::query_as("SELECT * FROM timer_states")
        .fetch_all(pool)
        .await?)
}

pub async fn delete_timer_state(pool: &Pool, user_id: i64) -> Result<()> {
    sqlx::query("DELETE FROM timer_states WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
