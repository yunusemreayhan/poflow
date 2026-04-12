use super::*;


pub const SESSION_SELECT: &str = "SELECT s.id, s.task_id, s.user_id, u.username as user, s.session_type, s.status, s.started_at, s.ended_at, s.duration_s, s.notes FROM sessions s JOIN users u ON s.user_id = u.id";

pub async fn get_task_sessions(pool: &Pool, task_id: i64) -> Result<Vec<Session>> {
    Ok(sqlx::query_as::<_, Session>(&format!("{} WHERE s.task_id = ? ORDER BY s.started_at DESC LIMIT 200", SESSION_SELECT))
        .bind(task_id).fetch_all(pool).await?)
}

pub async fn create_session(pool: &Pool, user_id: i64, task_id: Option<i64>, session_type: &str) -> Result<Session> {
    let now = now_str();
    let id = sqlx::query("INSERT INTO sessions (task_id, user_id, session_type, status, started_at) VALUES (?, ?, ?, 'running', ?)")
        .bind(task_id).bind(user_id).bind(session_type).bind(&now)
        .execute(pool).await?.last_insert_rowid();
    Ok(sqlx::query_as::<_, Session>(&format!("{} WHERE s.id = ?", SESSION_SELECT)).bind(id).fetch_one(pool).await?)
}

fn parse_timestamp(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S"))
        .unwrap_or_else(|e| { tracing::warn!("Failed to parse timestamp '{}': {}", s, e); Utc::now().naive_utc() })
}

pub async fn end_session(pool: &Pool, id: i64, status: &str) -> Result<Session> {
    let now = now_str();
    let row: (String, String) = sqlx::query_as("SELECT started_at, status FROM sessions WHERE id = ?").bind(id).fetch_one(pool).await?;
    if row.1 != "running" { return Err(anyhow::anyhow!("Session is not running (status: {})", row.1)); }
    let started = parse_timestamp(&row.0);
    let duration = (Utc::now().naive_utc() - started).num_seconds();
    sqlx::query("UPDATE sessions SET status=?, ended_at=?, duration_s=? WHERE id=?")
        .bind(status).bind(&now).bind(duration).bind(id).execute(pool).await?;
    Ok(sqlx::query_as::<_, Session>(&format!("{} WHERE s.id = ?", SESSION_SELECT)).bind(id).fetch_one(pool).await?)
}

// F7: Update session note
pub async fn update_session_note(pool: &Pool, id: i64, note: &str) -> Result<Session> {
    sqlx::query("UPDATE sessions SET notes = ? WHERE id = ?").bind(note).bind(id).execute(pool).await?;
    Ok(sqlx::query_as::<_, Session>(&format!("{} WHERE s.id = ?", SESSION_SELECT)).bind(id).fetch_one(pool).await?)
}

pub async fn get_session(pool: &Pool, id: i64) -> Result<Session> {
    Ok(sqlx::query_as::<_, Session>(&format!("{} WHERE s.id = ?", SESSION_SELECT)).bind(id).fetch_one(pool).await?)
}

pub async fn recover_interrupted(pool: &Pool) -> Result<Vec<Session>> {
    let sessions: Vec<Session> = sqlx::query_as(&format!("{} WHERE s.status = 'running'", SESSION_SELECT)).fetch_all(pool).await?;
    if !sessions.is_empty() {
        let now = now_str();
        for s in &sessions {
            let started = parse_timestamp(&s.started_at);
            let duration = (Utc::now().naive_utc() - started).num_seconds();
            sqlx::query("UPDATE sessions SET status='interrupted', ended_at=?, duration_s=? WHERE id=?")
                .bind(&now).bind(duration).bind(s.id).execute(pool).await?;
        }
    }
    Ok(sessions)
}

pub async fn get_history(pool: &Pool, from: &str, to: &str, user_id: Option<i64>) -> Result<Vec<SessionWithPath>> {
    let mut sql = format!("{} WHERE s.started_at >= ? AND s.started_at <= ?", SESSION_SELECT);
    if user_id.is_some() { sql.push_str(" AND s.user_id = ?"); }
    sql.push_str(" ORDER BY s.started_at DESC LIMIT 500");
    let mut query = sqlx::query_as::<_, Session>(&sql).bind(from).bind(to);
    if let Some(uid) = user_id { query = query.bind(uid); }
    let sessions: Vec<Session> = query.fetch_all(pool).await?;
    // Only load tasks referenced by these sessions
    let task_ids: Vec<i64> = sessions.iter().filter_map(|s| s.task_id).collect();
    if task_ids.is_empty() {
        return Ok(sessions.into_iter().map(|s| SessionWithPath { session: s, task_path: vec![] }).collect());
    }
    let placeholders = task_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    // Use CTE to get all ancestors
    let cte_sql = format!(
        "WITH RECURSIVE ancestors AS (SELECT id, parent_id, title FROM tasks WHERE id IN ({}) UNION ALL SELECT t.id, t.parent_id, t.title FROM tasks t JOIN ancestors a ON t.id = a.parent_id) SELECT DISTINCT id, parent_id, title FROM ancestors",
        placeholders
    );
    let mut q = sqlx::query_as::<_, (i64, Option<i64>, String)>(&cte_sql);
    for tid in &task_ids { q = q.bind(tid); }
    let rows: Vec<(i64, Option<i64>, String)> = q.fetch_all(pool).await?;
    let task_map: std::collections::HashMap<i64, (Option<i64>, String)> = rows.into_iter().map(|(id, pid, title)| (id, (pid, title))).collect();
    Ok(sessions.into_iter().map(|s| {
        let mut path = Vec::new();
        let mut current = s.task_id;
        while let Some(id) = current {
            if let Some((pid, title)) = task_map.get(&id) { path.push(title.clone()); current = *pid; } else { break; }
        }
        path.reverse();
        SessionWithPath { session: s, task_path: path }
    }).collect())
}

pub async fn get_day_stats(pool: &Pool, days: i64, user_id: Option<i64>) -> Result<Vec<DayStat>> {
    let from = (Utc::now().naive_utc() - chrono::Duration::days(days)).format("%Y-%m-%dT00:00:00").to_string();
    let mut sql = format!("{} WHERE s.session_type = 'work' AND s.started_at >= ?", SESSION_SELECT);
    if user_id.is_some() { sql.push_str(" AND s.user_id = ?"); }
    sql.push_str(" ORDER BY s.started_at");
    let mut query = sqlx::query_as::<_, Session>(&sql).bind(&from);
    if let Some(uid) = user_id { query = query.bind(uid); }
    let rows: Vec<Session> = query.fetch_all(pool).await?;
    let mut map: std::collections::BTreeMap<String, DayStat> = std::collections::BTreeMap::new();
    for r in rows {
        let date = r.started_at.get(..10).unwrap_or("").to_string();
        let entry = map.entry(date.clone()).or_insert(DayStat { date, completed: 0, interrupted: 0, total_focus_s: 0 });
        match r.status.as_str() { "completed" => entry.completed += 1, "interrupted" => entry.interrupted += 1, _ => {} }
        entry.total_focus_s += r.duration_s.unwrap_or(0);
    }
    Ok(map.into_values().collect())
}

pub async fn get_today_completed_for_user(pool: &Pool, user_id: Option<i64>) -> Result<i64> {
    let today = Utc::now().naive_utc().format("%Y-%m-%dT00:00:00").to_string();
    let (count,): (i64,) = if let Some(uid) = user_id {
        sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE session_type = 'work' AND status = 'completed' AND started_at >= ? AND user_id = ?")
            .bind(&today).bind(uid).fetch_one(pool).await?
    } else {
        sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE session_type = 'work' AND status = 'completed' AND started_at >= ?")
            .bind(&today).fetch_one(pool).await?
    };
    Ok(count)
}

// --- Comment CRUD ---

pub async fn recover_interrupted_sessions(pool: &Pool) -> Result<u64> {
    let sessions = recover_interrupted(pool).await?;
    Ok(sessions.len() as u64)
}
