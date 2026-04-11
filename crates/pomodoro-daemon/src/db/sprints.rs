use super::*;


const SPRINT_SELECT: &str = "SELECT sp.id, sp.name, sp.project, sp.goal, sp.status, sp.start_date, sp.end_date, sp.retro_notes, sp.created_by_id, u.username as created_by, sp.created_at, sp.updated_at FROM sprints sp JOIN users u ON sp.created_by_id = u.id";

pub async fn create_sprint(pool: &Pool, user_id: i64, name: &str, project: Option<&str>, goal: Option<&str>, start_date: Option<&str>, end_date: Option<&str>) -> Result<Sprint> {
    let now = now_str();
    let id = sqlx::query("INSERT INTO sprints (name, project, goal, start_date, end_date, created_by_id, created_at, updated_at) VALUES (?,?,?,?,?,?,?,?)")
        .bind(name).bind(project).bind(goal).bind(start_date).bind(end_date).bind(user_id).bind(&now).bind(&now)
        .execute(pool).await?.last_insert_rowid();
    get_sprint(pool, id).await
}

pub async fn get_sprint(pool: &Pool, id: i64) -> Result<Sprint> {
    Ok(sqlx::query_as::<_, Sprint>(&format!("{} WHERE sp.id = ?", SPRINT_SELECT)).bind(id).fetch_one(pool).await?)
}

pub async fn list_sprints(pool: &Pool, status: Option<&str>, project: Option<&str>) -> Result<Vec<Sprint>> {
    let mut q = format!("{} WHERE 1=1", SPRINT_SELECT);
    if status.is_some() { q.push_str(" AND sp.status = ?"); }
    if project.is_some() { q.push_str(" AND sp.project = ?"); }
    q.push_str(" ORDER BY sp.created_at DESC LIMIT 200");
    let mut query = sqlx::query_as::<_, Sprint>(&q);
    if let Some(s) = status { query = query.bind(s); }
    if let Some(p) = project { query = query.bind(p); }
    Ok(query.fetch_all(pool).await?)
}

pub async fn update_sprint(pool: &Pool, id: i64, name: Option<&str>, project: Option<Option<&str>>, goal: Option<Option<&str>>, status: Option<&str>, start_date: Option<Option<&str>>, end_date: Option<Option<&str>>, retro_notes: Option<Option<&str>>) -> Result<Sprint> {
    let now = now_str();
    let current = get_sprint(pool, id).await?;
    let new_project = match project { Some(v) => v.map(|s| s.to_string()), None => current.project };
    let new_goal = match goal { Some(v) => v.map(|s| s.to_string()), None => current.goal };
    let new_start = match start_date { Some(v) => v.map(|s| s.to_string()), None => current.start_date };
    let new_end = match end_date { Some(v) => v.map(|s| s.to_string()), None => current.end_date };
    let new_retro = match retro_notes { Some(v) => v.map(|s| s.to_string()), None => current.retro_notes };
    sqlx::query("UPDATE sprints SET name=?, project=?, goal=?, status=?, start_date=?, end_date=?, retro_notes=?, updated_at=? WHERE id=?")
        .bind(name.unwrap_or(&current.name)).bind(&new_project)
        .bind(&new_goal).bind(status.unwrap_or(&current.status))
        .bind(&new_start).bind(&new_end).bind(&new_retro)
        .bind(&now).bind(id).execute(pool).await?;
    get_sprint(pool, id).await
}

pub async fn delete_sprint(pool: &Pool, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM sprints WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

const SPRINT_TASK_SELECT: &str = "SELECT st.sprint_id, st.task_id, st.added_by_id, u.username as added_by, st.added_at FROM sprint_tasks st JOIN users u ON st.added_by_id = u.id";

pub async fn add_sprint_tasks(pool: &Pool, sprint_id: i64, task_ids: &[i64], user_id: i64) -> Result<Vec<SprintTask>> {
    let now = now_str();
    for tid in task_ids {
        sqlx::query("INSERT OR IGNORE INTO sprint_tasks (sprint_id, task_id, added_by_id, added_at) VALUES (?,?,?,?)")
            .bind(sprint_id).bind(tid).bind(user_id).bind(&now).execute(pool).await?;
    }
    get_sprint_task_entries(pool, sprint_id).await
}

pub async fn remove_sprint_task(pool: &Pool, sprint_id: i64, task_id: i64) -> Result<()> {
    sqlx::query("DELETE FROM sprint_tasks WHERE sprint_id = ? AND task_id = ?").bind(sprint_id).bind(task_id).execute(pool).await?;
    Ok(())
}

pub async fn get_sprint_task_entries(pool: &Pool, sprint_id: i64) -> Result<Vec<SprintTask>> {
    Ok(sqlx::query_as::<_, SprintTask>(&format!("{} WHERE st.sprint_id = ?", SPRINT_TASK_SELECT)).bind(sprint_id).fetch_all(pool).await?)
}

pub async fn get_sprint_tasks(pool: &Pool, sprint_id: i64) -> Result<Vec<Task>> {
    Ok(sqlx::query_as::<_, Task>(&format!("SELECT t.id, t.parent_id, t.user_id, u.username as user, t.title, t.description, t.project, t.tags, t.priority, t.estimated, t.actual, t.estimated_hours, t.remaining_points, t.due_date, t.status, t.sort_order, t.created_at, t.updated_at FROM tasks t JOIN users u ON t.user_id = u.id JOIN sprint_tasks st ON t.id = st.task_id WHERE st.sprint_id = ? ORDER BY t.sort_order"))
        .bind(sprint_id).fetch_all(pool).await?)
}

pub async fn get_sprint_board(pool: &Pool, sprint_id: i64) -> Result<SprintBoard> {
    let tasks = get_sprint_tasks(pool, sprint_id).await?;
    let (mut todo, mut in_progress, mut done) = (Vec::new(), Vec::new(), Vec::new());
    for t in tasks {
        match t.status.as_str() { "completed" | "done" => done.push(t), "in_progress" | "active" => in_progress.push(t), _ => todo.push(t) }
    }
    Ok(SprintBoard { todo, in_progress, done })
}

pub async fn get_sprint_detail(pool: &Pool, id: i64) -> Result<SprintDetail> {
    let sprint = get_sprint(pool, id).await?;
    let tasks = get_sprint_tasks(pool, id).await?;
    let stats = get_sprint_burndown(pool, id).await?;
    Ok(SprintDetail { sprint, tasks, stats })
}

pub async fn get_sprint_burndown(pool: &Pool, sprint_id: i64) -> Result<Vec<SprintDailyStat>> {
    Ok(sqlx::query_as::<_, SprintDailyStat>("SELECT * FROM sprint_daily_stats WHERE sprint_id = ? ORDER BY date")
        .bind(sprint_id).fetch_all(pool).await?)
}

pub async fn get_global_burndown(pool: &Pool) -> Result<Vec<SprintDailyStat>> {
    Ok(sqlx::query_as::<_, SprintDailyStat>(
        "SELECT 0 as id, 0 as sprint_id, date, SUM(total_points) as total_points, SUM(done_points) as done_points, \
         SUM(total_hours) as total_hours, SUM(done_hours) as done_hours, SUM(total_tasks) as total_tasks, SUM(done_tasks) as done_tasks \
         FROM sprint_daily_stats WHERE sprint_id IN (SELECT id FROM sprints WHERE status = 'active') \
         GROUP BY date ORDER BY date"
    ).fetch_all(pool).await?)
}

pub async fn snapshot_sprint(pool: &Pool, sprint_id: i64) -> Result<SprintDailyStat> {
    let tasks = get_sprint_tasks(pool, sprint_id).await?;
    let date = Utc::now().naive_utc().format("%Y-%m-%d").to_string();
    // total_points = sum of estimated story points (fixed scope), done_points = completed tasks' estimated points
    let total_points: f64 = tasks.iter().map(|t| t.estimated as f64).sum();
    let done_points: f64 = tasks.iter().filter(|t| t.status == "completed" || t.status == "done").map(|t| t.estimated as f64).sum();
    let total_hours: f64 = tasks.iter().map(|t| t.estimated_hours).sum();
    let done_hours: f64 = tasks.iter().filter(|t| t.status == "completed" || t.status == "done").map(|t| t.estimated_hours).sum();
    let total_tasks = tasks.len() as i64;
    let done_tasks = tasks.iter().filter(|t| t.status == "completed" || t.status == "done").count() as i64;
    // Upsert: keep latest snapshot per day
    sqlx::query("INSERT INTO sprint_daily_stats (sprint_id, date, total_points, done_points, total_hours, done_hours, total_tasks, done_tasks) VALUES (?,?,?,?,?,?,?,?) ON CONFLICT(sprint_id, date) DO UPDATE SET total_points=excluded.total_points, done_points=excluded.done_points, total_hours=excluded.total_hours, done_hours=excluded.done_hours, total_tasks=excluded.total_tasks, done_tasks=excluded.done_tasks")
        .bind(sprint_id).bind(&date).bind(total_points).bind(done_points).bind(total_hours).bind(done_hours).bind(total_tasks).bind(done_tasks)
        .execute(pool).await?;
    Ok(sqlx::query_as::<_, SprintDailyStat>("SELECT * FROM sprint_daily_stats WHERE sprint_id = ? AND date = ?")
        .bind(sprint_id).bind(&date).fetch_one(pool).await?)
}

pub async fn snapshot_active_sprints(pool: &Pool) -> Result<()> {
    let sprints = list_sprints(pool, Some("active"), None).await?;
    for s in sprints { let _ = snapshot_sprint(pool, s.id).await; }
    Ok(())
}

pub async fn get_all_task_sprints(pool: &Pool) -> Result<Vec<TaskSprintInfo>> {
    Ok(sqlx::query_as::<_, TaskSprintInfo>(
        "SELECT st.task_id, sp.id as sprint_id, sp.name as sprint_name, sp.status as sprint_status FROM sprint_tasks st JOIN sprints sp ON st.sprint_id = sp.id ORDER BY st.task_id"
    ).fetch_all(pool).await?)
}

// --- Burn log ---

pub async fn get_velocity(pool: &Pool, sprints: i64) -> Result<Vec<(String, f64, f64, i64)>> {
    let rows: Vec<(String, f64, f64, i64)> = sqlx::query_as(
        "SELECT s.name, COALESCE(SUM(CASE WHEN b.cancelled = 0 THEN b.points ELSE 0 END), 0) as points,
                COALESCE(SUM(CASE WHEN b.cancelled = 0 THEN b.hours ELSE 0 END), 0) as hours,
                COUNT(DISTINCT CASE WHEN t.status IN ('completed','done') THEN t.id END) as done_tasks
         FROM sprints s
         LEFT JOIN burn_log b ON b.sprint_id = s.id
         LEFT JOIN sprint_tasks st ON st.sprint_id = s.id
         LEFT JOIN tasks t ON t.id = st.task_id
         WHERE s.status = 'completed'
         GROUP BY s.id ORDER BY s.created_at DESC LIMIT ?"
    ).bind(sprints).fetch_all(pool).await?;
    Ok(rows)
}
