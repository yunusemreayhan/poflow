use super::*;


const COMMENT_SELECT: &str = "SELECT c.id, c.task_id, c.session_id, c.user_id, u.username as user, c.content, c.created_at, c.parent_id FROM comments c JOIN users u ON c.user_id = u.id";

pub async fn add_comment(pool: &Pool, user_id: i64, task_id: i64, session_id: Option<i64>, content: &str, parent_id: Option<i64>) -> Result<Comment> {
    let now = now_str();
    let id = sqlx::query("INSERT INTO comments (task_id, session_id, user_id, content, created_at, parent_id) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(task_id).bind(session_id).bind(user_id).bind(content).bind(&now).bind(parent_id)
        .execute(pool).await?.last_insert_rowid();
    Ok(sqlx::query_as::<_, Comment>(&format!("{} WHERE c.id = ?", COMMENT_SELECT)).bind(id).fetch_one(pool).await?)
}

pub async fn list_comments(pool: &Pool, task_id: i64) -> Result<Vec<Comment>> {
    // V30-15: Limit to 500 comments per task
    Ok(sqlx::query_as::<_, Comment>(&format!("{} WHERE c.task_id = ? ORDER BY c.created_at ASC LIMIT 500", COMMENT_SELECT))
        .bind(task_id).fetch_all(pool).await?)
}

pub async fn delete_comment(pool: &Pool, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM comments WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

pub async fn get_comment(pool: &Pool, id: i64) -> Result<Comment> {
    Ok(sqlx::query_as::<_, Comment>(&format!("{} WHERE c.id = ?", COMMENT_SELECT)).bind(id).fetch_one(pool).await?)
}

// --- Task detail ---

pub async fn get_task_detail(pool: &Pool, id: i64) -> Result<TaskDetail> {
    // Fetch all descendant tasks in one CTE query
    let all_tasks: Vec<Task> = sqlx::query_as(&format!(
        "WITH RECURSIVE descendants AS (\
            SELECT id FROM tasks WHERE id = ? \
            UNION ALL \
            SELECT t.id FROM tasks t JOIN descendants d ON t.parent_id = d.id WHERE t.deleted_at IS NULL\
        ) {select} WHERE t.id IN (SELECT id FROM descendants) AND t.deleted_at IS NULL ORDER BY t.sort_order ASC, t.id ASC",
        select = TASK_SELECT
    )).bind(id).fetch_all(pool).await?;

    if all_tasks.is_empty() { return Err(anyhow::anyhow!("Task not found")); }

    // Collect all task IDs
    let task_ids: Vec<i64> = all_tasks.iter().map(|t| t.id).collect();
    let ph = task_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

    // Batch fetch comments for all tasks
    let comments_sql = format!("{} WHERE c.task_id IN ({}) ORDER BY c.task_id, c.created_at ASC", COMMENT_SELECT, ph);
    let mut cq = sqlx::query_as::<_, Comment>(&comments_sql);
    for tid in &task_ids { cq = cq.bind(tid); }
    let all_comments: Vec<Comment> = cq.fetch_all(pool).await?;

    // Batch fetch sessions for all tasks
    let sessions_sql = format!("{} WHERE s.task_id IN ({}) ORDER BY s.task_id, s.started_at DESC LIMIT 2000", SESSION_SELECT, ph);
    let mut sq = sqlx::query_as::<_, Session>(&sessions_sql);
    for tid in &task_ids { sq = sq.bind(tid); }
    let all_sessions: Vec<Session> = sq.fetch_all(pool).await?;

    // Group by task_id
    let mut comments_map: std::collections::HashMap<i64, Vec<Comment>> = std::collections::HashMap::new();
    for c in all_comments { comments_map.entry(c.task_id).or_default().push(c); }
    let mut sessions_map: std::collections::HashMap<i64, Vec<Session>> = std::collections::HashMap::new();
    for s in all_sessions { if let Some(tid) = s.task_id { sessions_map.entry(tid).or_default().push(s); } }

    // Build tree from flat list
    let mut detail_map: std::collections::HashMap<i64, TaskDetail> = std::collections::HashMap::new();
    for t in &all_tasks {
        detail_map.insert(t.id, TaskDetail {
            task: t.clone(),
            comments: comments_map.remove(&t.id).unwrap_or_default(),
            sessions: sessions_map.remove(&t.id).unwrap_or_default(),
            children: vec![],
        });
    }

    // Assemble children (process in reverse to build bottom-up)
    let mut child_order: Vec<(i64, Option<i64>)> = all_tasks.iter().map(|t| (t.id, t.parent_id)).collect();
    child_order.reverse();
    for (tid, parent_id) in child_order {
        if tid == id { continue; }
        if let Some(pid) = parent_id {
            if let Some(child) = detail_map.remove(&tid) {
                if let Some(parent) = detail_map.get_mut(&pid) {
                    parent.children.push(child);
                }
            }
        }
    }
    // Sort children by sort_order
    fn sort_children(d: &mut TaskDetail) {
        d.children.sort_by_key(|c| (c.task.sort_order, c.task.id));
        for c in &mut d.children { sort_children(c); }
    }
    let mut root = detail_map.remove(&id).ok_or_else(|| anyhow::anyhow!("Task not found"))?;
    sort_children(&mut root);
    Ok(root)
}

// --- Time Reports ---
