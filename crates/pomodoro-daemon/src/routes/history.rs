use super::*;

// F4: User hours report
#[derive(Deserialize)]
pub struct UserHoursQuery { pub from: Option<String>, pub to: Option<String> }

#[utoipa::path(get, path = "/api/reports/user-hours", responses((status = 200)), security(("bearer" = [])))]
pub async fn user_hours_report(State(engine): State<AppState>, claims: Claims, Query(q): Query<UserHoursQuery>) -> ApiResult<Vec<serde_json::Value>> {
    if claims.role != "root" { return Err(err(StatusCode::FORBIDDEN, "Root only")); }
    let from = q.from.as_deref().unwrap_or("2000-01-01");
    let to = q.to.as_deref().unwrap_or("2099-12-31");
    let rows: Vec<(String, f64, i64)> = sqlx::query_as(
        "SELECT u.username, COALESCE(SUM(s.duration_s),0)/3600.0 as hours, COUNT(s.id) as sessions \
         FROM users u LEFT JOIN sessions s ON s.user_id = u.id AND s.status = 'completed' AND s.started_at >= ? AND s.started_at <= ? \
         GROUP BY u.id ORDER BY hours DESC")
        .bind(from).bind(to).fetch_all(&engine.pool).await.map_err(internal)?;
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
pub async fn get_stats(State(engine): State<AppState>, _claims: Claims, Query(q): Query<StatsQuery>) -> ApiResult<Vec<db::DayStat>> {
    db::get_day_stats(&engine.pool, q.days.unwrap_or(30)).await.map(Json).map_err(internal)
}

// --- Config ---
