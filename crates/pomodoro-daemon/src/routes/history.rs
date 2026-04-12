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

    // Component 4: Streak (0-20 pts) — current consecutive days with ≥1 session
    let mut streak = 0i64;
    for s in stats.iter().rev() {
        if s.completed > 0 { streak += 1; } else { break; }
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
    let unlocked_set: std::collections::HashSet<String> = unlocked.iter().map(|(t, _)| t.clone()).collect();
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

    // Streak achievements
    let mut streak = 0i64;
    for s in stats.iter().rev() {
        if s.completed > 0 { streak += 1; } else { break; }
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
