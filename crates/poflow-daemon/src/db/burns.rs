use super::*;

const BURN_SELECT: &str = "SELECT b.id, b.sprint_id, b.task_id, b.session_id, b.user_id, u.username, b.points, b.hours, b.source, b.note, b.cancelled, b.cancelled_by_id, cu.username as cancelled_by, b.created_at FROM burn_log b JOIN users u ON b.user_id = u.id LEFT JOIN users cu ON b.cancelled_by_id = cu.id";

pub struct LogBurnOpts<'a> {
    pub sprint_id: Option<i64>,
    pub task_id: i64,
    pub session_id: Option<i64>,
    pub user_id: i64,
    pub points: f64,
    pub hours: f64,
    pub source: &'a str,
    pub note: Option<&'a str>,
}

pub async fn log_burn(pool: &Pool, opts: LogBurnOpts<'_>) -> Result<BurnEntry> {
    let now = now_str();
    let id = sqlx::query("INSERT INTO burn_log (sprint_id, task_id, session_id, user_id, points, hours, source, note, created_at) VALUES (?,?,?,?,?,?,?,?,?)")
        .bind(opts.sprint_id).bind(opts.task_id).bind(opts.session_id).bind(opts.user_id).bind(opts.points).bind(opts.hours).bind(opts.source).bind(opts.note).bind(&now)
        .execute(pool).await?.last_insert_rowid();
    // Auto-assign user to task
    sqlx::query("INSERT OR IGNORE INTO task_assignees (task_id, user_id) VALUES (?, ?)")
        .bind(opts.task_id)
        .bind(opts.user_id)
        .execute(pool)
        .await?;
    Ok(
        sqlx::query_as::<_, BurnEntry>(&format!("{} WHERE b.id = ?", BURN_SELECT))
            .bind(id)
            .fetch_one(pool)
            .await?,
    )
}

pub async fn cancel_burn(pool: &Pool, burn_id: i64, cancelled_by_id: i64) -> Result<BurnEntry> {
    sqlx::query("UPDATE burn_log SET cancelled = 1, cancelled_by_id = ? WHERE id = ?")
        .bind(cancelled_by_id)
        .bind(burn_id)
        .execute(pool)
        .await?;
    Ok(
        sqlx::query_as::<_, BurnEntry>(&format!("{} WHERE b.id = ?", BURN_SELECT))
            .bind(burn_id)
            .fetch_one(pool)
            .await?,
    )
}

pub async fn get_burn(pool: &Pool, id: i64) -> Result<BurnEntry> {
    Ok(
        sqlx::query_as::<_, BurnEntry>(&format!("{} WHERE b.id = ?", BURN_SELECT))
            .bind(id)
            .fetch_one(pool)
            .await?,
    )
}

pub async fn list_burns(pool: &Pool, sprint_id: i64) -> Result<Vec<BurnEntry>> {
    // V32-17: Cap at 1000 to prevent unbounded responses
    Ok(sqlx::query_as::<_, BurnEntry>(&format!(
        "{} WHERE b.sprint_id = ? ORDER BY b.created_at DESC LIMIT 1000",
        BURN_SELECT
    ))
    .bind(sprint_id)
    .fetch_all(pool)
    .await?)
}

pub async fn list_task_burns(pool: &Pool, task_id: i64) -> Result<Vec<BurnEntry>> {
    Ok(sqlx::query_as::<_, BurnEntry>(&format!(
        "{} WHERE b.task_id = ? ORDER BY b.created_at DESC LIMIT 1000",
        BURN_SELECT
    ))
    .bind(task_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_task_burn_total(pool: &Pool, task_id: i64) -> Result<BurnTotal> {
    Ok(sqlx::query_as::<_, BurnTotal>(
        "SELECT COALESCE(SUM(points), 0) as total_points, COALESCE(SUM(hours), 0) as total_hours, COUNT(*) as count FROM burn_log WHERE task_id = ? AND cancelled = 0"
    ).bind(task_id).fetch_one(pool).await?)
}

pub async fn get_all_burn_totals(pool: &Pool) -> Result<Vec<(i64, BurnTotal)>> {
    let rows: Vec<(i64, f64, f64, i64)> = sqlx::query_as(
        "SELECT task_id, COALESCE(SUM(points), 0), COALESCE(SUM(hours), 0), COUNT(*) FROM burn_log WHERE cancelled = 0 GROUP BY task_id"
    ).fetch_all(pool).await?;
    Ok(rows
        .into_iter()
        .map(|(tid, p, h, c)| {
            (
                tid,
                BurnTotal {
                    total_points: p,
                    total_hours: h,
                    count: c,
                },
            )
        })
        .collect())
}

pub async fn get_all_assignees(pool: &Pool) -> Result<Vec<TaskAssignee>> {
    Ok(sqlx::query_as::<_, TaskAssignee>(
        "SELECT ta.task_id, ta.user_id, u.username FROM task_assignees ta JOIN users u ON ta.user_id = u.id ORDER BY ta.task_id, u.username"
    ).fetch_all(pool).await?)
}

pub async fn get_burn_summary(pool: &Pool, sprint_id: i64) -> Result<Vec<BurnSummaryEntry>> {
    Ok(sqlx::query_as::<_, BurnSummaryEntry>(
        "SELECT DATE(b.created_at) as date, u.username, SUM(b.points) as points, SUM(b.hours) as hours, COUNT(*) as count FROM burn_log b JOIN users u ON b.user_id = u.id WHERE b.sprint_id = ? AND b.cancelled = 0 GROUP BY DATE(b.created_at), u.username ORDER BY date, u.username"
    ).bind(sprint_id).fetch_all(pool).await?)
}

pub async fn get_task_burn_users(pool: &Pool, task_id: i64) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT u.username FROM burn_log b JOIN users u ON b.user_id = u.id WHERE b.task_id = ? AND b.cancelled = 0")
        .bind(task_id).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(u,)| u).collect())
}

pub async fn find_task_active_sprint(pool: &Pool, task_id: i64) -> Result<Option<i64>> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT sp.id FROM sprint_tasks st JOIN sprints sp ON st.sprint_id = sp.id WHERE st.task_id = ? AND sp.status = 'active' LIMIT 1")
        .bind(task_id).fetch_optional(pool).await?;
    Ok(row.map(|(id,)| id))
}

pub async fn get_user_config(pool: &Pool, user_id: i64) -> Result<Option<UserConfig>> {
    Ok(
        sqlx::query_as::<_, UserConfig>("SELECT * FROM user_configs WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(pool)
            .await?,
    )
}

pub async fn set_user_config(pool: &Pool, user_id: i64, cfg: &UserConfig) -> Result<UserConfig> {
    sqlx::query("INSERT INTO user_configs (user_id, work_duration_min, short_break_min, long_break_min, long_break_interval, auto_start_breaks, auto_start_work, daily_goal, theme, notify_desktop, notify_sound) VALUES (?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(user_id) DO UPDATE SET work_duration_min=excluded.work_duration_min, short_break_min=excluded.short_break_min, long_break_min=excluded.long_break_min, long_break_interval=excluded.long_break_interval, auto_start_breaks=excluded.auto_start_breaks, auto_start_work=excluded.auto_start_work, daily_goal=excluded.daily_goal, theme=excluded.theme, notify_desktop=excluded.notify_desktop, notify_sound=excluded.notify_sound")
        .bind(user_id).bind(cfg.work_duration_min).bind(cfg.short_break_min).bind(cfg.long_break_min).bind(cfg.long_break_interval).bind(cfg.auto_start_breaks).bind(cfg.auto_start_work).bind(cfg.daily_goal).bind(&cfg.theme).bind(cfg.notify_desktop).bind(cfg.notify_sound)
        .execute(pool).await?;
    Ok(
        sqlx::query_as::<_, UserConfig>("SELECT * FROM user_configs WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(pool)
            .await?,
    )
}

// --- Epic Groups ---
