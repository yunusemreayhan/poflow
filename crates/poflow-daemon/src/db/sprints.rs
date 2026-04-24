use super::*;

const SPRINT_SELECT: &str = "SELECT sp.id, sp.name, sp.project, sp.project_id, p.name as project_name, sp.goal, sp.status, sp.start_date, sp.end_date, sp.retro_notes, sp.capacity_hours, sp.created_by_id, u.username as created_by, sp.created_at, sp.updated_at FROM sprints sp JOIN users u ON sp.created_by_id = u.id LEFT JOIN projects p ON sp.project_id = p.id";

pub struct CreateSprintOpts<'a> {
    pub user_id: i64,
    pub name: &'a str,
    pub project: Option<&'a str>,
    pub project_id: Option<i64>,
    pub goal: Option<&'a str>,
    pub start_date: Option<&'a str>,
    pub end_date: Option<&'a str>,
    pub capacity_hours: Option<f64>,
}

pub async fn create_sprint(pool: &Pool, opts: CreateSprintOpts<'_>) -> Result<Sprint> {
    let now = now_str();
    let id = sqlx::query("INSERT INTO sprints (name, project, project_id, goal, start_date, end_date, capacity_hours, created_by_id, created_at, updated_at) VALUES (?,?,?,?,?,?,?,?,?,?)")
        .bind(opts.name).bind(opts.project).bind(opts.project_id).bind(opts.goal).bind(opts.start_date).bind(opts.end_date).bind(opts.capacity_hours).bind(opts.user_id).bind(&now).bind(&now)
        .execute(pool).await?.last_insert_rowid();
    get_sprint(pool, id).await
}

pub async fn get_sprint(pool: &Pool, id: i64) -> Result<Sprint> {
    Ok(
        sqlx::query_as::<_, Sprint>(&format!("{} WHERE sp.id = ?", SPRINT_SELECT))
            .bind(id)
            .fetch_one(pool)
            .await?,
    )
}

pub async fn list_sprints(
    pool: &Pool,
    status: Option<&str>,
    project: Option<&str>,
) -> Result<Vec<Sprint>> {
    let mut q = format!("{} WHERE 1=1", SPRINT_SELECT);
    if status.is_some() {
        q.push_str(" AND sp.status = ?");
    }
    if project.is_some() {
        q.push_str(" AND sp.project = ?");
    }
    q.push_str(" ORDER BY sp.created_at DESC LIMIT 200");
    let mut query = sqlx::query_as::<_, Sprint>(&q);
    if let Some(s) = status {
        query = query.bind(s);
    }
    if let Some(p) = project {
        query = query.bind(p);
    }
    Ok(query.fetch_all(pool).await?)
}

#[derive(Default)]
pub struct UpdateSprintOpts<'a> {
    pub name: Option<&'a str>,
    pub project: Option<Option<&'a str>>,
    pub project_id: Option<Option<i64>>,
    pub goal: Option<Option<&'a str>>,
    pub status: Option<&'a str>,
    pub start_date: Option<Option<&'a str>>,
    pub end_date: Option<Option<&'a str>>,
    pub retro_notes: Option<Option<&'a str>>,
    pub capacity_hours: Option<Option<f64>>,
}

pub async fn update_sprint(pool: &Pool, id: i64, opts: UpdateSprintOpts<'_>) -> Result<Sprint> {
    let now = now_str();
    let current = get_sprint(pool, id).await?;
    let new_project = match opts.project {
        Some(v) => v.map(|s| s.to_string()),
        None => current.project,
    };
    let new_project_id = match opts.project_id {
        Some(v) => v,
        None => current.project_id,
    };
    let new_goal = match opts.goal {
        Some(v) => v.map(|s| s.to_string()),
        None => current.goal,
    };
    let new_start = match opts.start_date {
        Some(v) => v.map(|s| s.to_string()),
        None => current.start_date,
    };
    let new_end = match opts.end_date {
        Some(v) => v.map(|s| s.to_string()),
        None => current.end_date,
    };
    let new_retro = match opts.retro_notes {
        Some(v) => v.map(|s| s.to_string()),
        None => current.retro_notes,
    };
    let new_cap = match opts.capacity_hours {
        Some(v) => v,
        None => current.capacity_hours,
    };
    sqlx::query("UPDATE sprints SET name=?, project=?, project_id=?, goal=?, status=?, start_date=?, end_date=?, retro_notes=?, capacity_hours=?, updated_at=? WHERE id=?")
        .bind(opts.name.unwrap_or(&current.name)).bind(&new_project).bind(new_project_id)
        .bind(&new_goal).bind(opts.status.unwrap_or(&current.status))
        .bind(&new_start).bind(&new_end).bind(&new_retro).bind(new_cap)
        .bind(&now).bind(id).execute(pool).await?;
    get_sprint(pool, id).await
}

pub async fn delete_sprint(pool: &Pool, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM sprints WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

const SPRINT_TASK_SELECT: &str = "SELECT st.sprint_id, st.task_id, st.added_by_id, u.username as added_by, st.added_at FROM sprint_tasks st JOIN users u ON st.added_by_id = u.id";

pub async fn add_sprint_tasks(
    pool: &Pool,
    sprint_id: i64,
    task_ids: &[i64],
    user_id: i64,
) -> Result<Vec<SprintTask>> {
    let now = now_str();
    // B9: Wrap in transaction to prevent partial inserts
    let mut tx = pool.begin().await?;
    for tid in task_ids {
        sqlx::query("INSERT OR IGNORE INTO sprint_tasks (sprint_id, task_id, added_by_id, added_at) VALUES (?,?,?,?)")
            .bind(sprint_id).bind(tid).bind(user_id).bind(&now).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    get_sprint_task_entries(pool, sprint_id).await
}

pub async fn remove_sprint_task(pool: &Pool, sprint_id: i64, task_id: i64) -> Result<()> {
    let r = sqlx::query("DELETE FROM sprint_tasks WHERE sprint_id = ? AND task_id = ?")
        .bind(sprint_id)
        .bind(task_id)
        .execute(pool)
        .await?;
    if r.rows_affected() == 0 {
        return Err(anyhow::anyhow!("Task not in sprint"));
    }
    Ok(())
}

pub async fn get_sprint_task_entries(pool: &Pool, sprint_id: i64) -> Result<Vec<SprintTask>> {
    Ok(
        sqlx::query_as::<_, SprintTask>(&format!("{} WHERE st.sprint_id = ?", SPRINT_TASK_SELECT))
            .bind(sprint_id)
            .fetch_all(pool)
            .await?,
    )
}

pub async fn get_sprint_tasks(pool: &Pool, sprint_id: i64) -> Result<Vec<Task>> {
    Ok(sqlx::query_as::<_, Task>(&format!("{} JOIN sprint_tasks st ON t.id = st.task_id WHERE st.sprint_id = ? AND t.deleted_at IS NULL ORDER BY t.sort_order", super::tasks::TASK_SELECT))
        .bind(sprint_id).fetch_all(pool).await?)
}

pub async fn get_sprint_board(pool: &Pool, sprint_id: i64) -> Result<SprintBoard> {
    let tasks = get_sprint_tasks(pool, sprint_id).await?;
    // Load custom status categories for board column mapping
    let custom: Vec<(String, String)> =
        sqlx::query_as("SELECT name, category FROM custom_statuses")
            .fetch_all(pool)
            .await
            .unwrap_or_default();
    let category_map: std::collections::HashMap<&str, &str> = custom
        .iter()
        .map(|(n, c)| (n.as_str(), c.as_str()))
        .collect();
    let (mut todo, mut in_progress, mut blocked, mut done) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for t in tasks {
        let cat = category_map.get(t.status.as_str()).copied();
        match cat.unwrap_or(t.status.as_str()) {
            "completed" | "done" => done.push(t),
            "blocked" => blocked.push(t),
            "in_progress" | "active" => in_progress.push(t),
            _ => todo.push(t),
        }
    }
    Ok(SprintBoard {
        todo,
        in_progress,
        blocked,
        done,
    })
}

pub async fn get_sprint_detail(pool: &Pool, id: i64) -> Result<SprintDetail> {
    let sprint = get_sprint(pool, id).await?;
    let tasks = get_sprint_tasks(pool, id).await?;
    let stats = get_sprint_burndown(pool, id).await?;
    Ok(SprintDetail {
        sprint,
        tasks,
        stats,
    })
}

pub async fn get_sprint_burndown(pool: &Pool, sprint_id: i64) -> Result<Vec<SprintDailyStat>> {
    Ok(sqlx::query_as::<_, SprintDailyStat>(
        "SELECT * FROM sprint_daily_stats WHERE sprint_id = ? ORDER BY date",
    )
    .bind(sprint_id)
    .fetch_all(pool)
    .await?)
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
    let date = Utc::now().naive_utc().format("%Y-%m-%d").to_string();
    // Single SQL aggregate instead of loading all task rows
    let (total_tasks, done_tasks, total_points, done_points, total_hours, done_hours): (i64, i64, f64, f64, f64, f64) =
        sqlx::query_as("SELECT COALESCE(COUNT(*),0), \
            COALESCE(SUM(CASE WHEN t.status IN ('completed','done') THEN 1 ELSE 0 END),0), \
            COALESCE(SUM(t.remaining_points),0.0), \
            COALESCE(SUM(CASE WHEN t.status IN ('completed','done') THEN t.remaining_points ELSE 0.0 END),0.0), \
            COALESCE(SUM(t.estimated_hours),0.0), \
            COALESCE(SUM(CASE WHEN t.status IN ('completed','done') THEN t.estimated_hours ELSE 0.0 END),0.0) \
            FROM sprint_tasks st JOIN tasks t ON st.task_id = t.id WHERE st.sprint_id = ? AND t.deleted_at IS NULL")
        .bind(sprint_id).fetch_one(pool).await?;
    // Upsert: keep latest snapshot per day
    sqlx::query("INSERT INTO sprint_daily_stats (sprint_id, date, total_points, done_points, total_hours, done_hours, total_tasks, done_tasks) VALUES (?,?,?,?,?,?,?,?) ON CONFLICT(sprint_id, date) DO UPDATE SET total_points=excluded.total_points, done_points=excluded.done_points, total_hours=excluded.total_hours, done_hours=excluded.done_hours, total_tasks=excluded.total_tasks, done_tasks=excluded.done_tasks")
        .bind(sprint_id).bind(&date).bind(total_points).bind(done_points).bind(total_hours).bind(done_hours).bind(total_tasks).bind(done_tasks)
        .execute(pool).await?;
    Ok(sqlx::query_as::<_, SprintDailyStat>(
        "SELECT * FROM sprint_daily_stats WHERE sprint_id = ? AND date = ?",
    )
    .bind(sprint_id)
    .bind(&date)
    .fetch_one(pool)
    .await?)
}

pub async fn snapshot_active_sprints(pool: &Pool) -> Result<()> {
    let sprints = list_sprints(pool, Some("active"), None).await?;
    for s in sprints {
        let _ = snapshot_sprint(pool, s.id).await;
    }
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
