use super::*;

// F4: User hours report
#[derive(Deserialize)]
pub struct UserHoursQuery { pub from: Option<String>, pub to: Option<String> }

#[utoipa::path(get, path = "/api/reports/user-hours", responses((status = 200)), security(("bearer" = [])))]
pub async fn user_hours_report(State(engine): State<AppState>, claims: Claims, Query(q): Query<UserHoursQuery>) -> ApiResult<Vec<serde_json::Value>> {
    if claims.role != "root" { return Err(err(StatusCode::FORBIDDEN, "Root only")); }
    let from = q.from.as_deref().unwrap_or("2000-01-01");
    let to = q.to.as_deref().unwrap_or("2099-12-31");
    if chrono::NaiveDate::parse_from_str(from, "%Y-%m-%d").is_err() { return Err(err(StatusCode::BAD_REQUEST, "from must be YYYY-MM-DD")); }
    if chrono::NaiveDate::parse_from_str(to, "%Y-%m-%d").is_err() { return Err(err(StatusCode::BAD_REQUEST, "to must be YYYY-MM-DD")); }
    // B5: Append T23:59:59 to include the full end day (started_at is a full ISO timestamp)
    let to_ts = format!("{}T23:59:59", to);
    let rows: Vec<(String, f64, i64)> = sqlx::query_as(
        "SELECT u.username, COALESCE(SUM(s.duration_s),0)/3600.0 as hours, COUNT(s.id) as sessions \
         FROM users u LEFT JOIN sessions s ON s.user_id = u.id AND s.status = 'completed' AND s.started_at >= ? AND s.started_at <= ? \
         GROUP BY u.id ORDER BY hours DESC")
        .bind(from).bind(&to_ts).fetch_all(&engine.pool).await.map_err(internal)?;
    Ok(Json(rows.into_iter().map(|(u, h, s)| serde_json::json!({"username": u, "hours": (h * 100.0).round() / 100.0, "sessions": s})).collect()))
}

#[utoipa::path(get, path = "/api/history", responses((status = 200, body = Vec<db::SessionWithPath>)), security(("bearer" = [])))]
pub async fn get_history(State(engine): State<AppState>, claims: Claims, Query(q): Query<HistoryQuery>) -> ApiResult<Vec<db::SessionWithPath>> {
    let from = q.from.unwrap_or_else(|| "2000-01-01T00:00:00".to_string());
    let to = q.to.unwrap_or_else(|| "2099-12-31T23:59:59".to_string());
    // S5: Non-root users can only see their own history
    let user_id = if claims.role == "root" { q.user_id } else { Some(claims.user_id) };
    db::get_history(&engine.pool, &from, &to, user_id).await.map(Json).map_err(internal)
}

#[utoipa::path(get, path = "/api/stats", responses((status = 200, body = Vec<db::DayStat>)), security(("bearer" = [])))]
pub async fn get_stats(State(engine): State<AppState>, claims: Claims, Query(q): Query<StatsQuery>) -> ApiResult<Vec<db::DayStat>> {
    let user_id = if claims.role == "root" { None } else { Some(claims.user_id) };
    db::get_day_stats(&engine.pool, q.days.unwrap_or(30).min(365), user_id).await.map(Json).map_err(internal)
}

// --- Config ---

// F6: Estimation accuracy report
#[derive(Deserialize)]
pub struct AccuracyQuery { pub project: Option<String> }

#[utoipa::path(get, path = "/api/analytics/estimation-accuracy", responses((status = 200)), security(("bearer" = [])))]
pub async fn estimation_accuracy(State(engine): State<AppState>, claims: Claims, Query(q): Query<AccuracyQuery>) -> ApiResult<serde_json::Value> {
    let user_filter = if claims.role == "root" { None } else { Some(claims.user_id) };
    let mut sql = String::from("SELECT id, title, project, estimated, actual, estimated_hours FROM tasks WHERE status IN ('completed','done') AND estimated > 0 AND deleted_at IS NULL");
    if user_filter.is_some() { sql.push_str(" AND user_id = ?"); }
    if q.project.is_some() { sql.push_str(" AND project = ?"); }
    sql.push_str(" ORDER BY updated_at DESC LIMIT 500");
    let mut query = sqlx::query_as::<_, (i64, String, Option<String>, i64, i64, f64)>(&sql);
    if let Some(uid) = user_filter { query = query.bind(uid); }
    if let Some(ref p) = q.project { query = query.bind(p); }
    let rows = query.fetch_all(&engine.pool).await.map_err(internal)?;

    let mut total_est = 0i64;
    let mut total_act = 0i64;
    let mut over = 0i64;
    let mut under = 0i64;
    let mut exact = 0i64;
    let mut by_project: std::collections::HashMap<String, (i64, i64, i64)> = std::collections::HashMap::new();
    for (_, _, project, est, act, _) in &rows {
        total_est += est;
        total_act += act;
        if act > est { under += 1; } else if act < est { over += 1; } else { exact += 1; }
        let p = project.as_deref().unwrap_or("(none)").to_string();
        let e = by_project.entry(p).or_default();
        e.0 += est; e.1 += act; e.2 += 1;
    }
    let count = rows.len() as f64;
    let accuracy = if total_est > 0 { ((1.0 - (total_act as f64 - total_est as f64).abs() / total_est as f64) * 100.0).max(0.0) } else { 0.0 };
    let projects: Vec<serde_json::Value> = by_project.into_iter().map(|(p, (e, a, c))| {
        serde_json::json!({"project": p, "estimated": e, "actual": a, "count": c, "accuracy": if e > 0 { ((1.0 - (a as f64 - e as f64).abs() / e as f64) * 100.0).max(0.0) } else { 0.0 }})
    }).collect();

    Ok(Json(serde_json::json!({
        "total_tasks": count, "total_estimated": total_est, "total_actual": total_act,
        "accuracy_pct": (accuracy * 10.0).round() / 10.0,
        "over_estimated": over, "under_estimated": under, "exact": exact,
        "by_project": projects,
    })))
}

// F8: Personal focus score (0-100)
#[utoipa::path(get, path = "/api/analytics/focus-score", responses((status = 200)), security(("bearer" = [])))]
pub async fn focus_score(State(engine): State<AppState>, claims: Claims) -> ApiResult<serde_json::Value> {
    let config = engine.get_user_config(claims.user_id).await;
    let stats = db::get_day_stats(&engine.pool, 30, Some(claims.user_id)).await.map_err(internal)?;
    if stats.is_empty() { return Ok(Json(serde_json::json!({"score": 0, "components": {}}))); }

    // Component 1: Goal achievement (0-30 pts) — avg daily sessions vs goal
    let goal = config.daily_goal.max(1) as f64;
    let avg_sessions = stats.iter().map(|s| s.completed as f64).sum::<f64>() / stats.len() as f64;
    let goal_score = ((avg_sessions / goal).min(1.0) * 30.0).round();

    // Component 2: Consistency (0-30 pts) — days with ≥1 session / total days
    let active_days = stats.iter().filter(|s| s.completed > 0).count() as f64;
    let consistency_score = ((active_days / stats.len() as f64) * 30.0).round();

    // Component 3: Completion rate (0-20 pts) — completed / (completed + interrupted)
    let total_completed: i64 = stats.iter().map(|s| s.completed).sum();
    let total_interrupted: i64 = stats.iter().map(|s| s.interrupted).sum();
    let total_sessions = total_completed + total_interrupted;
    let completion_rate = if total_sessions > 0 { total_completed as f64 / total_sessions as f64 } else { 0.0 };
    let completion_score = (completion_rate * 20.0).round();

    // Component 4: Streak (0-20 pts) — current consecutive calendar days with ≥1 session
    // V28-1: Check actual date continuity, not just consecutive entries
    let mut streak = 0i64;
    let active_dates: std::collections::HashSet<&str> = stats.iter().filter(|s| s.completed > 0).map(|s| s.date.as_str()).collect();
    let today = chrono::Utc::now().naive_utc().date();
    let mut check_date = today;
    loop {
        let ds = check_date.format("%Y-%m-%d").to_string();
        if active_dates.contains(ds.as_str()) { streak += 1; } else if check_date < today { break; } // allow today to be missing (day not over)
        else { break; }
        check_date -= chrono::Duration::days(1);
    }
    let streak_score = ((streak as f64 / 14.0).min(1.0) * 20.0).round(); // 14-day streak = max

    let score = (goal_score + consistency_score + completion_score + streak_score).min(100.0);

    Ok(Json(serde_json::json!({
        "score": score,
        "streak_days": streak,
        "components": {
            "goal_achievement": goal_score,
            "consistency": consistency_score,
            "completion_rate": completion_score,
            "streak": streak_score,
        }
    })))
}

// F22: Streaks & Achievements
const ACHIEVEMENT_DEFS: &[(&str, &str)] = &[
    ("streak_7", "7-day focus streak"),
    ("streak_30", "30-day focus streak"),
    ("sessions_100", "100 completed sessions"),
    ("sessions_500", "500 completed sessions"),
    ("first_sprint", "First sprint completed"),
    ("accuracy_80", "Estimation accuracy >80%"),
];

#[utoipa::path(get, path = "/api/achievements", responses((status = 200)), security(("bearer" = [])))]
pub async fn list_achievements(State(engine): State<AppState>, claims: Claims) -> ApiResult<Vec<serde_json::Value>> {
    let unlocked: Vec<(String, String)> = sqlx::query_as("SELECT achievement_type, unlocked_at FROM achievements WHERE user_id = ? ORDER BY unlocked_at DESC")
        .bind(claims.user_id).fetch_all(&engine.pool).await.map_err(internal)?;
    let result: Vec<serde_json::Value> = ACHIEVEMENT_DEFS.iter().map(|(typ, desc)| {
        let u = unlocked.iter().find(|(t, _)| t == typ);
        serde_json::json!({"type": typ, "description": desc, "unlocked": u.is_some(), "unlocked_at": u.map(|(_, d)| d.as_str())})
    }).collect();
    Ok(Json(result))
}

#[utoipa::path(post, path = "/api/achievements/check", responses((status = 200)), security(("bearer" = [])))]
pub async fn check_achievements(State(engine): State<AppState>, claims: Claims) -> ApiResult<Vec<serde_json::Value>> {
    let stats = db::get_day_stats(&engine.pool, 365, Some(claims.user_id)).await.map_err(internal)?;
    let mut newly_unlocked = Vec::new();
    let now = db::now_str();

    // Streak achievements — V28-1: check consecutive calendar days
    let active_dates: std::collections::HashSet<&str> = stats.iter().filter(|s| s.completed > 0).map(|s| s.date.as_str()).collect();
    let mut streak = 0i64;
    let today = chrono::Utc::now().naive_utc().date();
    let mut check_date = today;
    loop {
        let ds = check_date.format("%Y-%m-%d").to_string();
        if active_dates.contains(ds.as_str()) { streak += 1; } else if check_date < today { break; } else { break; }
        check_date -= chrono::Duration::days(1);
    }
    if streak >= 7 { try_unlock(&engine.pool, claims.user_id, "streak_7", &now, &mut newly_unlocked).await; }
    if streak >= 30 { try_unlock(&engine.pool, claims.user_id, "streak_30", &now, &mut newly_unlocked).await; }

    // Session count achievements
    let total: i64 = stats.iter().map(|s| s.completed).sum();
    if total >= 100 { try_unlock(&engine.pool, claims.user_id, "sessions_100", &now, &mut newly_unlocked).await; }
    if total >= 500 { try_unlock(&engine.pool, claims.user_id, "sessions_500", &now, &mut newly_unlocked).await; }

    // Sprint achievement
    let (sprint_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sprints WHERE created_by_id = ? AND status = 'completed'")
        .bind(claims.user_id).fetch_one(&engine.pool).await.map_err(internal)?;
    if sprint_count >= 1 { try_unlock(&engine.pool, claims.user_id, "first_sprint", &now, &mut newly_unlocked).await; }

    // Estimation accuracy
    let (est_total, act_total): (i64, i64) = sqlx::query_as("SELECT COALESCE(SUM(estimated),0), COALESCE(SUM(actual),0) FROM tasks WHERE user_id = ? AND status IN ('completed','done') AND estimated > 0 AND deleted_at IS NULL")
        .bind(claims.user_id).fetch_one(&engine.pool).await.map_err(internal)?;
    if est_total > 0 {
        let accuracy = (1.0 - (act_total as f64 - est_total as f64).abs() / est_total as f64) * 100.0;
        if accuracy >= 80.0 { try_unlock(&engine.pool, claims.user_id, "accuracy_80", &now, &mut newly_unlocked).await; }
    }

    Ok(Json(newly_unlocked))
}

async fn try_unlock(pool: &db::Pool, user_id: i64, typ: &str, now: &str, newly: &mut Vec<serde_json::Value>) {
    let result = sqlx::query("INSERT OR IGNORE INTO achievements (user_id, achievement_type, unlocked_at) VALUES (?, ?, ?)")
        .bind(user_id).bind(typ).bind(now).execute(pool).await;
    if let Ok(r) = result {
        if r.rows_affected() > 0 {
            let desc = ACHIEVEMENT_DEFS.iter().find(|(t, _)| *t == typ).map(|(_, d)| *d).unwrap_or(typ);
            newly.push(serde_json::json!({"type": typ, "description": desc}));
        }
    }
}

// F23: Focus leaderboard — weekly/monthly team stats
#[derive(Deserialize)]
pub struct LeaderboardQuery { pub period: Option<String> }

#[utoipa::path(get, path = "/api/leaderboard", responses((status = 200)), security(("bearer" = [])))]
pub async fn leaderboard(State(engine): State<AppState>, _claims: Claims, Query(q): Query<LeaderboardQuery>) -> ApiResult<Vec<serde_json::Value>> {
    let days = match q.period.as_deref() { Some("month") => 30, Some("year") => 365, _ => 7 };
    let rows: Vec<(String, f64, i64)> = sqlx::query_as(
        "SELECT u.username, COALESCE(SUM(s.duration_s),0)/3600.0, COUNT(s.id) \
         FROM users u LEFT JOIN sessions s ON s.user_id = u.id AND s.status = 'completed' AND s.started_at >= date('now', ?) \
         GROUP BY u.id ORDER BY SUM(s.duration_s) DESC")
        .bind(format!("-{} days", days)).fetch_all(&engine.pool).await.map_err(internal)?;
    Ok(Json(rows.into_iter().map(|(u, h, s)| serde_json::json!({"username": u, "hours": (h * 100.0).round() / 100.0, "sessions": s})).collect()))
}

// F21: Auto-prioritization suggestions
#[utoipa::path(get, path = "/api/suggestions/priorities", responses((status = 200)), security(("bearer" = [])))]
pub async fn priority_suggestions(State(engine): State<AppState>, claims: Claims) -> ApiResult<Vec<serde_json::Value>> {
    let today = chrono::Utc::now().naive_utc().format("%Y-%m-%d").to_string();
    let rows: Vec<(i64, String, i64, Option<String>, String)> = sqlx::query_as(
        "SELECT id, title, priority, due_date, updated_at FROM tasks WHERE user_id = ? AND status IN ('backlog','active','in_progress') AND deleted_at IS NULL")
        .bind(claims.user_id).fetch_all(&engine.pool).await.map_err(internal)?;

    let mut suggestions = Vec::new();
    for (id, title, priority, due_date, updated_at) in &rows {
        let mut suggested = *priority;
        let mut reasons = Vec::new();

        // Due date approaching
        if let Some(due) = due_date {
            if let (Ok(d), Ok(t)) = (chrono::NaiveDate::parse_from_str(due, "%Y-%m-%d"), chrono::NaiveDate::parse_from_str(&today, "%Y-%m-%d")) {
                let days_left = (d - t).num_days();
                if days_left < 0 { suggested = 5; reasons.push(format!("Overdue by {} days", -days_left)); }
                else if days_left <= 1 { suggested = suggested.max(5); reasons.push("Due tomorrow or today".into()); }
                else if days_left <= 3 { suggested = suggested.max(4); reasons.push(format!("Due in {} days", days_left)); }
            }
        }

        // Stale task (not updated in 14+ days)
        if let Ok(updated) = chrono::NaiveDateTime::parse_from_str(updated_at, "%Y-%m-%dT%H:%M:%S%.f")
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(updated_at, "%Y-%m-%dT%H:%M:%S")) {
            let days_stale = (chrono::Utc::now().naive_utc() - updated).num_days();
            if days_stale > 14 && *priority < 4 { suggested = suggested.max(3); reasons.push(format!("Stale ({} days)", days_stale)); }
        }

        if suggested != *priority && !reasons.is_empty() {
            suggestions.push(serde_json::json!({"task_id": id, "title": title, "current_priority": priority, "suggested_priority": suggested, "reasons": reasons}));
        }
    }
    suggestions.sort_by(|a, b| b["suggested_priority"].as_i64().cmp(&a["suggested_priority"].as_i64()));
    Ok(Json(suggestions))
}

// F9: Activity feed — unified stream of task updates, comments, sprint changes
#[derive(Deserialize)]
pub struct FeedQuery { pub since: Option<String>, pub types: Option<String>, pub limit: Option<i64> }

#[utoipa::path(get, path = "/api/feed", responses((status = 200)), security(("bearer" = [])))]
pub async fn activity_feed(State(engine): State<AppState>, _claims: Claims, Query(q): Query<FeedQuery>) -> ApiResult<Vec<serde_json::Value>> {
    let since = q.since.as_deref().unwrap_or("2000-01-01T00:00:00");
    // PF7: Basic datetime format validation
    if q.since.is_some() && chrono::NaiveDateTime::parse_from_str(since, "%Y-%m-%dT%H:%M:%S").is_err()
        && chrono::NaiveDateTime::parse_from_str(since, "%Y-%m-%dT%H:%M:%S%.f").is_err() {
        return Err(err(StatusCode::BAD_REQUEST, "Invalid 'since' format. Use ISO 8601 (YYYY-MM-DDTHH:MM:SS)"));
    }
    let limit = q.limit.unwrap_or(50).min(200);
    let types: Option<Vec<&str>> = q.types.as_deref().map(|t| t.split(',').map(|s| s.trim()).collect());

    let mut items: Vec<serde_json::Value> = Vec::new();

    // Audit log entries
    if types.as_ref().map_or(true, |t| t.iter().any(|x| *x == "audit" || *x == "all")) {
        let rows: Vec<(String, String, Option<String>, String, String)> = sqlx::query_as(
            "SELECT a.action, a.entity_type, a.detail, a.created_at, u.username FROM audit_log a JOIN users u ON a.user_id = u.id WHERE a.created_at > ? ORDER BY a.created_at DESC LIMIT ?")
            .bind(since).bind(limit).fetch_all(&engine.pool).await.map_err(internal)?;
        for (action, entity, detail, at, user) in rows {
            items.push(serde_json::json!({"type": "audit", "action": action, "entity_type": entity, "detail": detail, "created_at": at, "user": user}));
        }
    }

    // Recent comments
    if types.as_ref().map_or(true, |t| t.iter().any(|x| *x == "comment" || *x == "all")) {
        let rows: Vec<(i64, i64, String, String, String)> = sqlx::query_as(
            "SELECT c.id, c.task_id, c.content, c.created_at, u.username FROM comments c JOIN users u ON c.user_id = u.id WHERE c.created_at > ? ORDER BY c.created_at DESC LIMIT ?")
            .bind(since).bind(limit).fetch_all(&engine.pool).await.map_err(internal)?;
        for (id, task_id, content, at, user) in rows {
            items.push(serde_json::json!({"type": "comment", "id": id, "task_id": task_id, "content": content.chars().take(200).collect::<String>(), "created_at": at, "user": user}));
        }
    }

    // Sort by created_at descending, truncate
    items.sort_by(|a, b| b["created_at"].as_str().unwrap_or("").cmp(a["created_at"].as_str().unwrap_or("")));
    items.truncate(limit as usize);
    Ok(Json(items))
}

// F3: Smart scheduling suggestions — analyze historical patterns
#[utoipa::path(get, path = "/api/suggestions/schedule", responses((status = 200)), security(("bearer" = [])))]
pub async fn schedule_suggestions(State(engine): State<AppState>, claims: Claims) -> ApiResult<serde_json::Value> {
    // Analyze completed sessions by hour of day
    let hourly: Vec<(i64, i64, f64)> = sqlx::query_as(
        "SELECT CAST(strftime('%H', started_at) AS INTEGER) as hour, COUNT(*) as cnt, AVG(duration_s)/60.0 as avg_min \
         FROM sessions WHERE user_id = ? AND status = 'completed' AND session_type = 'work' \
         GROUP BY hour ORDER BY cnt DESC")
        .bind(claims.user_id).fetch_all(&engine.pool).await.map_err(internal)?;

    let peak_hours: Vec<serde_json::Value> = hourly.iter().take(3).map(|(h, c, m)| {
        serde_json::json!({"hour": h, "sessions": c, "avg_minutes": (*m * 10.0).round() / 10.0})
    }).collect();

    // Average sessions per day
    let (avg_daily,): (f64,) = sqlx::query_as(
        "SELECT COALESCE(AVG(cnt), 0.0) FROM (SELECT COUNT(*) as cnt FROM sessions WHERE user_id = ? AND status = 'completed' AND session_type = 'work' GROUP BY date(started_at))")
        .bind(claims.user_id).fetch_one(&engine.pool).await.map_err(internal)?;

    // Day of week patterns
    let dow: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT CAST(strftime('%w', started_at) AS INTEGER) as dow, COUNT(*) FROM sessions WHERE user_id = ? AND status = 'completed' AND session_type = 'work' GROUP BY dow ORDER BY COUNT(*) DESC")
        .bind(claims.user_id).fetch_all(&engine.pool).await.map_err(internal)?;
    let dow_names = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let best_days: Vec<String> = dow.iter().take(3).map(|(d, _)| dow_names.get(*d as usize).unwrap_or(&"?").to_string()).collect();

    // Upcoming tasks needing scheduling
    let upcoming: Vec<(i64, String, f64, Option<String>)> = sqlx::query_as(
        "SELECT id, title, estimated_hours, due_date FROM tasks WHERE user_id = ? AND status IN ('backlog','active','in_progress') AND deleted_at IS NULL AND estimated_hours > 0 ORDER BY COALESCE(due_date, '9999') ASC LIMIT 10")
        .bind(claims.user_id).fetch_all(&engine.pool).await.map_err(internal)?;

    let suggestions: Vec<serde_json::Value> = upcoming.iter().map(|(id, title, hours, due)| {
        let sessions_needed = (*hours / 0.42).ceil() as i64; // ~25min per session
        let days_needed = (sessions_needed as f64 / avg_daily.max(1.0)).ceil() as i64;
        serde_json::json!({"task_id": id, "title": title, "estimated_hours": hours, "due_date": due, "sessions_needed": sessions_needed, "days_needed": days_needed})
    }).collect();

    Ok(Json(serde_json::json!({
        "peak_hours": peak_hours,
        "avg_daily_sessions": (avg_daily * 10.0).round() / 10.0,
        "best_days": best_days,
        "task_suggestions": suggestions,
    })))
}

// F20: On-demand report generation (can be called by cron/scheduler)
#[utoipa::path(get, path = "/api/reports/weekly-digest", responses((status = 200)), security(("bearer" = [])))]
pub async fn weekly_digest(State(engine): State<AppState>, claims: Claims) -> ApiResult<serde_json::Value> {
    let stats = db::get_day_stats(&engine.pool, 7, Some(claims.user_id)).await.map_err(internal)?;
    let total_focus: f64 = stats.iter().map(|s| s.total_focus_s as f64 / 3600.0).sum();
    let total_sessions: i64 = stats.iter().map(|s| s.completed).sum();

    // Tasks completed this week
    let (completed,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tasks WHERE user_id = ? AND status IN ('completed','done') AND updated_at >= date('now', '-7 days') AND deleted_at IS NULL")
        .bind(claims.user_id).fetch_one(&engine.pool).await.map_err(internal)?;

    // Upcoming due dates
    let upcoming: Vec<(String, String)> = sqlx::query_as("SELECT title, due_date FROM tasks WHERE user_id = ? AND due_date BETWEEN date('now') AND date('now', '+7 days') AND status NOT IN ('completed','done','archived') AND deleted_at IS NULL ORDER BY due_date")
        .bind(claims.user_id).fetch_all(&engine.pool).await.map_err(internal)?;

    Ok(Json(serde_json::json!({
        "period": "last_7_days",
        "focus_hours": (total_focus * 100.0).round() / 100.0,
        "sessions": total_sessions,
        "tasks_completed": completed,
        "upcoming_due": upcoming.into_iter().map(|(t, d)| serde_json::json!({"title": t, "due_date": d})).collect::<Vec<_>>(),
    })))
}
