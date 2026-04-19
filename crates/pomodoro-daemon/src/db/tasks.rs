use super::*;

// B12: FTS5 availability flag — set by migrate, read by search
static FTS5_AVAILABLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub async fn check_fts5(_pool: &Pool) -> bool {
    FTS5_AVAILABLE.load(std::sync::atomic::Ordering::Relaxed)
}
pub fn set_fts5_available(v: bool) { FTS5_AVAILABLE.store(v, std::sync::atomic::Ordering::Relaxed); }
fn fts5_ok() -> bool { FTS5_AVAILABLE.load(std::sync::atomic::Ordering::Relaxed) }

fn search_clause() -> &'static str {
    if fts5_ok() { " AND t.id IN (SELECT rowid FROM tasks_fts WHERE tasks_fts MATCH ?)" }
    else { " AND (t.title LIKE ? ESCAPE '\\' OR t.tags LIKE ? ESCAPE '\\' OR t.description LIKE ? ESCAPE '\\')" }
}

fn escape_like(s: &str) -> String {
    format!("%{}%", s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"))
}

pub fn escape_like_pub(s: &str) -> String { escape_like(s) }

/// Strip all HTML tags except <mark> and </mark> from FTS5 snippets
fn sanitize_snippet(s: &str) -> String {
    // Replace <mark> and </mark> with placeholders, strip all other tags, restore placeholders
    s.replace("<mark>", "\x00MARK\x00")
     .replace("</mark>", "\x00/MARK\x00")
     .replace('<', "&lt;").replace('>', "&gt;")
     .replace("\x00MARK\x00", "<mark>")
     .replace("\x00/MARK\x00", "</mark>")
}

// P3: Populate temp table for large ID sets to avoid SQLite bind parameter limit (999)
async fn populate_team_scope_table(pool: &Pool, ids: &[i64]) -> Result<()> {
    sqlx::query("CREATE TEMP TABLE IF NOT EXISTS _team_scope (id INTEGER PRIMARY KEY)").execute(pool).await?;
    sqlx::query("DELETE FROM _team_scope").execute(pool).await?;
    for chunk in ids.chunks(400) {
        let ph: String = chunk.iter().map(|_| "(?)").collect::<Vec<_>>().join(",");
        let sql = format!("INSERT OR IGNORE INTO _team_scope (id) VALUES {}", ph);
        let mut q = sqlx::query(&sql);
        for id in chunk { q = q.bind(id); }
        q.execute(pool).await?;
    }
    Ok(())
}

fn append_team_scope_filter(q: &mut String, ids: &[i64]) -> bool {
    if ids.len() > 500 {
        q.push_str(" AND t.id IN (SELECT id FROM _team_scope)");
        true
    } else {
        let ph: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        q.push_str(&format!(" AND t.id IN ({})", ph));
        false
    }
}

pub const TASK_SELECT: &str = "SELECT t.id, t.parent_id, t.user_id, u.username as user, t.title, t.description, t.project, t.tags, t.priority, t.estimated, t.actual, t.estimated_hours, t.remaining_points, t.due_date, t.status, t.sort_order, t.created_at, t.updated_at, COALESCE(ac.cnt, 0) as attachment_count, t.deleted_at, t.work_duration_minutes, t.estimate_optimistic, t.estimate_pessimistic, t.updated_by FROM tasks t JOIN users u ON t.user_id = u.id LEFT JOIN (SELECT task_id, COUNT(*) as cnt FROM task_attachments GROUP BY task_id) ac ON ac.task_id = t.id";

pub struct CreateTaskOpts<'a> {
    pub user_id: i64,
    pub parent_id: Option<i64>,
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub project: Option<&'a str>,
    pub tags: Option<&'a str>,
    pub priority: i64,
    pub estimated: i64,
    pub estimated_hours: f64,
    pub remaining_points: f64,
    pub due_date: Option<&'a str>,
}

pub async fn create_task(pool: &Pool, opts: CreateTaskOpts<'_>) -> Result<Task> {
    let now = now_str();
    let max_order: (i64,) = match opts.parent_id {
        Some(pid) => sqlx::query_as("SELECT COALESCE(MAX(sort_order), 0) FROM tasks WHERE parent_id = ?").bind(pid).fetch_one(pool).await?,
        None => sqlx::query_as("SELECT COALESCE(MAX(sort_order), 0) FROM tasks WHERE parent_id IS NULL AND user_id = ?").bind(opts.user_id).fetch_one(pool).await?,
    };
    let id = sqlx::query("INSERT INTO tasks (parent_id, user_id, title, description, project, tags, priority, estimated, estimated_hours, remaining_points, due_date, status, sort_order, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'backlog', ?, ?, ?)")
        .bind(opts.parent_id).bind(opts.user_id).bind(opts.title).bind(opts.description).bind(opts.project).bind(opts.tags).bind(opts.priority).bind(opts.estimated).bind(opts.estimated_hours).bind(opts.remaining_points).bind(opts.due_date).bind(max_order.0 + 1).bind(&now).bind(&now)
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
    pub label: Option<&'a str>,
}

pub async fn list_tasks(pool: &Pool, status: Option<&str>, project: Option<&str>) -> Result<Vec<Task>> {
    list_tasks_paged(pool, TaskFilter { status, project, search: None, assignee: None, due_before: None, due_after: None, priority: None, team_id: None, user_id: None, label: None }, 5000, 0).await
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

    // P2: Use EXISTS subquery for assignee filter to avoid duplicates
    let mut q = format!("{} WHERE t.deleted_at IS NULL", TASK_SELECT);
    if f.assignee.is_some() {
        q.push_str(" AND EXISTS (SELECT 1 FROM task_assignees _ta JOIN users _au ON _au.id = _ta.user_id WHERE _ta.task_id = t.id AND _au.username = ?)");
    }
    if f.status.is_some() { q.push_str(" AND t.status = ?"); }
    if f.project.is_some() { q.push_str(" AND t.project = ?"); }
    if f.search.is_some() { q.push_str(search_clause()); }
    if f.priority.is_some() { q.push_str(" AND t.priority = ?"); }
    if f.due_before.is_some() { q.push_str(" AND t.due_date IS NOT NULL AND t.due_date <= ?"); }
    if f.due_after.is_some() { q.push_str(" AND t.due_date IS NOT NULL AND t.due_date >= ?"); }
    if f.user_id.is_some() { q.push_str(" AND t.user_id = ?"); }
    if f.label.is_some() { q.push_str(" AND EXISTS (SELECT 1 FROM task_labels _tl JOIN labels _l ON _l.id = _tl.label_id WHERE _tl.task_id = t.id AND _l.name = ?)"); }
    let used_temp = if let Some(ref ids) = team_scope {
        if ids.len() > 500 { populate_team_scope_table(pool, ids).await?; }
        append_team_scope_filter(&mut q, ids)
    } else { false };
    q.push_str(" ORDER BY t.sort_order ASC, t.id ASC LIMIT ? OFFSET ?");

    let mut query = sqlx::query_as::<_, Task>(&q);
    // Bind assignee first if using JOIN (it's in the WHERE clause)
    if let Some(a) = f.assignee { query = query.bind(a); }
    if let Some(s) = f.status { query = query.bind(s); }
    if let Some(p) = f.project { query = query.bind(p); }
    if let Some(s) = f.search {
        if fts5_ok() { let fts = format!("\"{}\"", s.replace('"', "\"\"")); query = query.bind(fts); }
        else { let like = escape_like(s); query = query.bind(like.clone()).bind(like.clone()).bind(like); }
    }
    if let Some(p) = f.priority { query = query.bind(p); }
    if let Some(d) = f.due_before { query = query.bind(d); }
    if let Some(d) = f.due_after { query = query.bind(d); }
    if let Some(uid) = f.user_id { query = query.bind(uid); }
    if let Some(l) = f.label { query = query.bind(l); }
    if let Some(ref ids) = team_scope { if !used_temp { for id in ids { query = query.bind(id); } } }
    query = query.bind(limit).bind(offset);
    Ok(query.fetch_all(pool).await?)
}

#[derive(Default)]
pub struct UpdateTaskOpts<'a> {
    pub title: Option<&'a str>,
    pub description: Option<Option<&'a str>>,
    pub project: Option<Option<&'a str>>,
    pub tags: Option<Option<&'a str>>,
    pub priority: Option<i64>,
    pub estimated: Option<i64>,
    pub estimated_hours: Option<f64>,
    pub remaining_points: Option<f64>,
    pub due_date: Option<Option<&'a str>>,
    pub status: Option<&'a str>,
    pub sort_order: Option<i64>,
    pub parent_id: Option<Option<i64>>,
    pub work_duration_minutes: Option<Option<i64>>,
    pub estimate_optimistic: Option<Option<f64>>,
    pub estimate_pessimistic: Option<Option<f64>>,
    pub updated_by: Option<i64>,
}

pub async fn update_task(pool: &Pool, id: i64, opts: UpdateTaskOpts<'_>) -> Result<Task> {
    let now = now_str();
    let existing = get_task(pool, id).await?;
    let new_parent = match opts.parent_id { Some(p) => p, None => existing.parent_id };
    let new_desc = match opts.description { Some(v) => v.map(|s| s.to_string()), None => existing.description };
    let new_project = match opts.project { Some(v) => v.map(|s| s.to_string()), None => existing.project };
    let new_tags = match opts.tags { Some(v) => v.map(|s| s.to_string()), None => existing.tags };
    let new_due = match opts.due_date { Some(v) => v.map(|s| s.to_string()), None => existing.due_date };
    let new_wdm = match opts.work_duration_minutes { Some(v) => v, None => existing.work_duration_minutes };
    let new_eo = match opts.estimate_optimistic { Some(v) => v, None => existing.estimate_optimistic };
    let new_ep = match opts.estimate_pessimistic { Some(v) => v, None => existing.estimate_pessimistic };
    sqlx::query("UPDATE tasks SET parent_id=?, title=?, description=?, project=?, tags=?, priority=?, estimated=?, estimated_hours=?, remaining_points=?, due_date=?, status=?, sort_order=?, work_duration_minutes=?, estimate_optimistic=?, estimate_pessimistic=?, updated_at=?, updated_by=? WHERE id=?")
        .bind(new_parent).bind(opts.title.unwrap_or(&existing.title)).bind(&new_desc)
        .bind(&new_project).bind(&new_tags)
        .bind(opts.priority.unwrap_or(existing.priority)).bind(opts.estimated.unwrap_or(existing.estimated))
        .bind(opts.estimated_hours.unwrap_or(existing.estimated_hours)).bind(opts.remaining_points.unwrap_or(existing.remaining_points))
        .bind(&new_due).bind(opts.status.unwrap_or(&existing.status))
        .bind(opts.sort_order.unwrap_or(existing.sort_order)).bind(new_wdm).bind(new_eo).bind(new_ep).bind(&now).bind(opts.updated_by).bind(id)
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
    if f.search.is_some() { q.push_str(search_clause()); }
    if f.priority.is_some() { q.push_str(" AND t.priority = ?"); }
    if f.due_before.is_some() { q.push_str(" AND t.due_date IS NOT NULL AND t.due_date <= ?"); }
    if f.due_after.is_some() { q.push_str(" AND t.due_date IS NOT NULL AND t.due_date >= ?"); }
    if f.user_id.is_some() { q.push_str(" AND t.user_id = ?"); }
    if f.label.is_some() { q.push_str(" AND EXISTS (SELECT 1 FROM task_labels _tl JOIN labels _l ON _l.id = _tl.label_id WHERE _tl.task_id = t.id AND _l.name = ?)"); }
    let used_temp = if let Some(ref ids) = team_scope {
        if ids.len() > 500 { populate_team_scope_table(pool, ids).await?; }
        append_team_scope_filter(&mut q, ids)
    } else { false };
    let mut query = sqlx::query_as::<_, (i64,)>(&q);
    if let Some(a) = f.assignee { query = query.bind(a); }
    if let Some(s) = f.status { query = query.bind(s); }
    if let Some(p) = f.project { query = query.bind(p); }
    if let Some(s) = f.search {
        if fts5_ok() { let fts = format!("\"{}\"", s.replace('"', "\"\"")); query = query.bind(fts); }
        else { let like = escape_like(s); query = query.bind(like.clone()).bind(like.clone()).bind(like); }
    }
    if let Some(p) = f.priority { query = query.bind(p); }
    if let Some(d) = f.due_before { query = query.bind(d); }
    if let Some(d) = f.due_after { query = query.bind(d); }
    if let Some(uid) = f.user_id { query = query.bind(uid); }
    if let Some(l) = f.label { query = query.bind(l); }
    if let Some(ref ids) = team_scope { if !used_temp { for id in ids { query = query.bind(id); } } }
    let (count,) = query.fetch_one(pool).await?;
    Ok(count)
}
// F1: FTS5 search with snippets
// B2: Added user_id filter — non-root users only see their own tasks
pub async fn search_tasks_fts(pool: &Pool, query: &str, limit: i64, user_id: Option<i64>) -> Result<Vec<(i64, String, String)>> {
    if !fts5_ok() {
        let like = escape_like(query);
        let sql = if user_id.is_some() {
            "SELECT id, title, COALESCE(SUBSTR(description, 1, 200), '') FROM tasks WHERE deleted_at IS NULL AND user_id = ? AND (title LIKE ? ESCAPE '\\' OR description LIKE ? ESCAPE '\\' OR tags LIKE ? ESCAPE '\\') LIMIT ?".to_string()
        } else {
            "SELECT id, title, COALESCE(SUBSTR(description, 1, 200), '') FROM tasks WHERE deleted_at IS NULL AND (title LIKE ? ESCAPE '\\' OR description LIKE ? ESCAPE '\\' OR tags LIKE ? ESCAPE '\\') LIMIT ?".to_string()
        };
        let mut q = sqlx::query_as::<_, (i64, String, String)>(&sql);
        if let Some(uid) = user_id { q = q.bind(uid); }
        q = q.bind(&like).bind(&like).bind(&like).bind(limit);
        return Ok(q.fetch_all(pool).await?);
    }
    let fts = format!("\"{}\"", query.replace('"', "\"\""));
    let sql = if user_id.is_some() {
        "SELECT f.rowid, snippet(tasks_fts, 0, '<mark>', '</mark>', '...', 32), snippet(tasks_fts, 1, '<mark>', '</mark>', '...', 48) FROM tasks_fts f JOIN tasks t ON t.id = f.rowid WHERE tasks_fts MATCH ? AND t.user_id = ? AND t.deleted_at IS NULL ORDER BY rank LIMIT ?".to_string()
    } else {
        "SELECT f.rowid, snippet(tasks_fts, 0, '<mark>', '</mark>', '...', 32), snippet(tasks_fts, 1, '<mark>', '</mark>', '...', 48) FROM tasks_fts f JOIN tasks t ON t.id = f.rowid WHERE tasks_fts MATCH ? AND t.deleted_at IS NULL ORDER BY rank LIMIT ?".to_string()
    };
    let mut q = sqlx::query_as::<_, (i64, String, String)>(&sql);
    q = q.bind(&fts);
    if let Some(uid) = user_id { q = q.bind(uid); }
    q = q.bind(limit);
    let rows: Vec<(i64, String, String)> = q.fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(id, title, desc)| (id, sanitize_snippet(&title), sanitize_snippet(&desc))).collect())
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
    for (id, sort_order) in orders {
        // V35-19: Only update sort_order, skip updated_at to avoid triggering full reloads
        sqlx::query("UPDATE tasks SET sort_order = ? WHERE id = ?")
            .bind(sort_order).bind(id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn get_due_tasks(pool: &Pool, before_date: &str) -> Result<Vec<(i64, String, String)>> {
    Ok(sqlx::query_as(
        "SELECT id, title, due_date FROM tasks WHERE due_date IS NOT NULL AND due_date <= ? AND status != 'completed' AND status != 'done' AND deleted_at IS NULL"
    ).bind(before_date).fetch_all(pool).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_snippet_strips_html() {
        assert_eq!(sanitize_snippet("<img onerror=alert(1)>"), "&lt;img onerror=alert(1)&gt;");
        assert_eq!(sanitize_snippet("<mark>hi</mark>"), "<mark>hi</mark>");
        assert_eq!(sanitize_snippet("<script>alert(1)</script> <mark>ok</mark>"),
            "&lt;script&gt;alert(1)&lt;/script&gt; <mark>ok</mark>");
    }

    #[test]
    fn test_escape_like_wildcards() {
        assert_eq!(escape_like("a_b"), "%a\\_b%");
        assert_eq!(escape_like("100%"), "%100\\%%");
        assert_eq!(escape_like("a\\b"), "%a\\\\b%");
        assert_eq!(escape_like("normal"), "%normal%");
    }
}