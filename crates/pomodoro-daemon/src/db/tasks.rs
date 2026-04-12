use super::*;


pub const TASK_SELECT: &str = "SELECT t.id, t.parent_id, t.user_id, u.username as user, t.title, t.description, t.project, t.tags, t.priority, t.estimated, t.actual, t.estimated_hours, t.remaining_points, t.due_date, t.status, t.sort_order, t.created_at, t.updated_at, (SELECT COUNT(*) FROM task_attachments WHERE task_id = t.id) as attachment_count, t.deleted_at, t.work_duration_minutes FROM tasks t JOIN users u ON t.user_id = u.id";

pub async fn create_task(pool: &Pool, user_id: i64, parent_id: Option<i64>, title: &str, description: Option<&str>, project: Option<&str>, tags: Option<&str>, priority: i64, estimated: i64, estimated_hours: f64, remaining_points: f64, due_date: Option<&str>) -> Result<Task> {
    let now = now_str();
    let max_order: (i64,) = match parent_id {
        Some(pid) => sqlx::query_as("SELECT COALESCE(MAX(sort_order), 0) FROM tasks WHERE parent_id = ?").bind(pid).fetch_one(pool).await?,
        None => sqlx::query_as("SELECT COALESCE(MAX(sort_order), 0) FROM tasks WHERE parent_id IS NULL AND user_id = ?").bind(user_id).fetch_one(pool).await?,
    };
    let id = sqlx::query("INSERT INTO tasks (parent_id, user_id, title, description, project, tags, priority, estimated, estimated_hours, remaining_points, due_date, status, sort_order, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'backlog', ?, ?, ?)")
        .bind(parent_id).bind(user_id).bind(title).bind(description).bind(project).bind(tags).bind(priority).bind(estimated).bind(estimated_hours).bind(remaining_points).bind(due_date).bind(max_order.0 + 1).bind(&now).bind(&now)
        .execute(pool).await?.last_insert_rowid();
    get_task(pool, id).await
}

pub async fn get_task(pool: &Pool, id: i64) -> Result<Task> {
    Ok(sqlx::query_as::<_, Task>(&format!("{} WHERE t.id = ?", TASK_SELECT)).bind(id).fetch_one(pool).await?)
}

pub struct TaskFilter<'a> {
    pub status: Option<&'a str>,
    pub project: Option<&'a str>,
    pub search: Option<&'a str>,
    pub assignee: Option<&'a str>,
    pub due_before: Option<&'a str>,
    pub due_after: Option<&'a str>,
    pub priority: Option<i64>,
    pub team_id: Option<i64>,
    pub user_id: Option<i64>,
}

pub async fn list_tasks(pool: &Pool, status: Option<&str>, project: Option<&str>) -> Result<Vec<Task>> {
    list_tasks_paged(pool, TaskFilter { status, project, search: None, assignee: None, due_before: None, due_after: None, priority: None, team_id: None, user_id: None }, 5000, 0).await
}

pub async fn list_deleted_tasks(pool: &Pool, user_id: Option<i64>) -> Result<Vec<Task>> {
    let mut q = format!("{} WHERE t.deleted_at IS NOT NULL", TASK_SELECT);
    if user_id.is_some() { q.push_str(" AND t.user_id = ?"); }
    q.push_str(" ORDER BY t.deleted_at DESC LIMIT 500");
    let mut query = sqlx::query_as::<_, Task>(&q);
    if let Some(uid) = user_id { query = query.bind(uid); }
    Ok(query.fetch_all(pool).await?)
}

pub async fn list_tasks_paged(pool: &Pool, f: TaskFilter<'_>, limit: i64, offset: i64) -> Result<Vec<Task>> {
    let team_scope: Option<Vec<i64>> = if let Some(tid) = f.team_id {
        let roots: Vec<(i64,)> = sqlx::query_as("SELECT task_id FROM team_root_tasks WHERE team_id = ?").bind(tid).fetch_all(pool).await?;
        if roots.is_empty() { return Ok(vec![]); }
        let rids: Vec<i64> = roots.into_iter().map(|r| r.0).collect();
        Some(get_descendant_ids(pool, &rids).await?)
    } else { None };

    // P2: Use JOIN for assignee filter instead of pre-fetching IDs
    let mut q = format!("{} WHERE t.deleted_at IS NULL", TASK_SELECT);
    if f.assignee.is_some() {
        q = format!("{} JOIN task_assignees _ta ON _ta.task_id = t.id JOIN users _au ON _au.id = _ta.user_id WHERE t.deleted_at IS NULL AND _au.username = ?", TASK_SELECT);
    }
    if f.status.is_some() { q.push_str(" AND t.status = ?"); }
    if f.project.is_some() { q.push_str(" AND t.project = ?"); }
    if f.search.is_some() { q.push_str(" AND t.id IN (SELECT rowid FROM tasks_fts WHERE tasks_fts MATCH ?)"); }
    if f.priority.is_some() { q.push_str(" AND t.priority = ?"); }
    if f.due_before.is_some() { q.push_str(" AND t.due_date IS NOT NULL AND t.due_date <= ?"); }
    if f.due_after.is_some() { q.push_str(" AND t.due_date IS NOT NULL AND t.due_date >= ?"); }
    if f.user_id.is_some() { q.push_str(" AND t.user_id = ?"); }
    if let Some(ref ids) = team_scope {
        let ph: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        q.push_str(&format!(" AND t.id IN ({})", ph));
    }
    q.push_str(" ORDER BY t.sort_order ASC, t.id ASC LIMIT ? OFFSET ?");

    let mut query = sqlx::query_as::<_, Task>(&q);
    // Bind assignee first if using JOIN (it's in the WHERE clause)
    if let Some(a) = f.assignee { query = query.bind(a); }
    if let Some(s) = f.status { query = query.bind(s); }
    if let Some(p) = f.project { query = query.bind(p); }
    if let Some(s) = f.search { let fts = format!("\"{}\"", s.replace('"', "\"\"")); query = query.bind(fts); }
    if let Some(p) = f.priority { query = query.bind(p); }
    if let Some(d) = f.due_before { query = query.bind(d); }
    if let Some(d) = f.due_after { query = query.bind(d); }
    if let Some(uid) = f.user_id { query = query.bind(uid); }
    if let Some(ref ids) = team_scope { for id in ids { query = query.bind(id); } }
    query = query.bind(limit).bind(offset);
    Ok(query.fetch_all(pool).await?)
}

pub async fn update_task(pool: &Pool, id: i64, title: Option<&str>, description: Option<Option<&str>>, project: Option<Option<&str>>, tags: Option<Option<&str>>, priority: Option<i64>, estimated: Option<i64>, estimated_hours: Option<f64>, remaining_points: Option<f64>, due_date: Option<Option<&str>>, status: Option<&str>, sort_order: Option<i64>, parent_id: Option<Option<i64>>, work_duration_minutes: Option<Option<i64>>) -> Result<Task> {
    let now = now_str();
    let existing = get_task(pool, id).await?;
    let new_parent = match parent_id { Some(p) => p, None => existing.parent_id };
    let new_desc = match description { Some(v) => v.map(|s| s.to_string()), None => existing.description };
    let new_project = match project { Some(v) => v.map(|s| s.to_string()), None => existing.project };
    let new_tags = match tags { Some(v) => v.map(|s| s.to_string()), None => existing.tags };
    let new_due = match due_date { Some(v) => v.map(|s| s.to_string()), None => existing.due_date };
    let new_wdm = match work_duration_minutes { Some(v) => v, None => existing.work_duration_minutes };
    sqlx::query("UPDATE tasks SET parent_id=?, title=?, description=?, project=?, tags=?, priority=?, estimated=?, estimated_hours=?, remaining_points=?, due_date=?, status=?, sort_order=?, work_duration_minutes=?, updated_at=? WHERE id=?")
        .bind(new_parent).bind(title.unwrap_or(&existing.title)).bind(&new_desc)
        .bind(&new_project).bind(&new_tags)
        .bind(priority.unwrap_or(existing.priority)).bind(estimated.unwrap_or(existing.estimated))
        .bind(estimated_hours.unwrap_or(existing.estimated_hours)).bind(remaining_points.unwrap_or(existing.remaining_points))
        .bind(&new_due).bind(status.unwrap_or(&existing.status))
        .bind(sort_order.unwrap_or(existing.sort_order)).bind(new_wdm).bind(&now).bind(id)
        .execute(pool).await?;
    get_task(pool, id).await
}

pub async fn delete_task(pool: &Pool, id: i64) -> Result<()> {
    let ids = get_descendant_ids(pool, &[id]).await?;
    let ph = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let now = now_str();
    // Soft delete: set deleted_at on task and all descendants
    let sql = format!("UPDATE tasks SET deleted_at = ?, updated_at = ? WHERE id IN ({}) AND deleted_at IS NULL", ph);
    let mut q = sqlx::query(&sql).bind(&now).bind(&now);
    for tid in &ids { q = q.bind(tid); }
    q.execute(pool).await?;
    Ok(())
}

pub async fn increment_task_actual(pool: &Pool, id: i64) -> Result<()> {
    sqlx::query("UPDATE tasks SET actual = actual + 1 WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

// --- Session CRUD ---

pub async fn count_tasks(pool: &Pool, f: TaskFilter<'_>) -> Result<i64> {
    let team_scope: Option<Vec<i64>> = if let Some(tid) = f.team_id {
        let roots: Vec<(i64,)> = sqlx::query_as("SELECT task_id FROM team_root_tasks WHERE team_id = ?").bind(tid).fetch_all(pool).await?;
        if roots.is_empty() { return Ok(0); }
        Some(get_descendant_ids(pool, &roots.into_iter().map(|r| r.0).collect::<Vec<_>>()).await?)
    } else { None };

    // P2: Use JOIN for assignee filter
    let mut q = if f.assignee.is_some() {
        "SELECT COUNT(*) FROM tasks t JOIN task_assignees _ta ON _ta.task_id = t.id JOIN users _au ON _au.id = _ta.user_id WHERE t.deleted_at IS NULL AND _au.username = ?".to_string()
    } else {
        "SELECT COUNT(*) FROM tasks t WHERE t.deleted_at IS NULL".to_string()
    };
    if f.status.is_some() { q.push_str(" AND t.status = ?"); }
    if f.project.is_some() { q.push_str(" AND t.project = ?"); }
    if f.search.is_some() { q.push_str(" AND t.id IN (SELECT rowid FROM tasks_fts WHERE tasks_fts MATCH ?)"); }
    if f.priority.is_some() { q.push_str(" AND t.priority = ?"); }
    if f.due_before.is_some() { q.push_str(" AND t.due_date IS NOT NULL AND t.due_date <= ?"); }
    if f.due_after.is_some() { q.push_str(" AND t.due_date IS NOT NULL AND t.due_date >= ?"); }
    if f.user_id.is_some() { q.push_str(" AND t.user_id = ?"); }
    if let Some(ref ids) = team_scope {
        let ph: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        q.push_str(&format!(" AND t.id IN ({})", ph));
    }
    let mut query = sqlx::query_as::<_, (i64,)>(&q);
    if let Some(a) = f.assignee { query = query.bind(a); }
    if let Some(s) = f.status { query = query.bind(s); }
    if let Some(p) = f.project { query = query.bind(p); }
    if let Some(s) = f.search { let fts = format!("\"{}\"", s.replace('"', "\"\"")); query = query.bind(fts); }
    if let Some(p) = f.priority { query = query.bind(p); }
    if let Some(d) = f.due_before { query = query.bind(d); }
    if let Some(d) = f.due_after { query = query.bind(d); }
    if let Some(uid) = f.user_id { query = query.bind(uid); }
    if let Some(ref ids) = team_scope { for id in ids { query = query.bind(id); } }
    let (count,) = query.fetch_one(pool).await?;
    Ok(count)
}

pub async fn restore_task(pool: &Pool, id: i64) -> Result<()> {
    let ids = get_descendant_ids(pool, &[id]).await?;
    let ph = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let now = now_str();
    let sql = format!("UPDATE tasks SET deleted_at = NULL, updated_at = ? WHERE id IN ({})", ph);
    let mut q = sqlx::query(&sql).bind(&now);
    for tid in &ids { q = q.bind(tid); }
    q.execute(pool).await?;
    Ok(())
}

pub async fn reorder_tasks(pool: &Pool, orders: &[(i64, i64)]) -> Result<()> {
    let mut tx = pool.begin().await?;
    let now = now_str();
    for (id, sort_order) in orders {
        sqlx::query("UPDATE tasks SET sort_order = ?, updated_at = ? WHERE id = ?")
            .bind(sort_order).bind(&now).bind(id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn get_due_tasks(pool: &Pool, before_date: &str) -> Result<Vec<(i64, String, String)>> {
    Ok(sqlx::query_as(
        "SELECT id, title, due_date FROM tasks WHERE due_date IS NOT NULL AND due_date <= ? AND status != 'completed' AND status != 'done'"
    ).bind(before_date).fetch_all(pool).await?)
}
