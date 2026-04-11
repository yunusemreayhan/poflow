use super::*;


#[utoipa::path(get, path = "/api/history", responses((status = 200, body = Vec<db::SessionWithPath>)), security(("bearer" = [])))]
pub async fn get_history(State(engine): State<AppState>, _claims: Claims, Query(q): Query<HistoryQuery>) -> ApiResult<Vec<db::SessionWithPath>> {
    let from = q.from.unwrap_or_else(|| "2000-01-01T00:00:00".to_string());
    let to = q.to.unwrap_or_else(|| "2099-12-31T23:59:59".to_string());
    db::get_history(&engine.pool, &from, &to, q.user_id).await.map(Json).map_err(internal)
}

#[utoipa::path(get, path = "/api/stats", responses((status = 200, body = Vec<db::DayStat>)), security(("bearer" = [])))]
pub async fn get_stats(State(engine): State<AppState>, _claims: Claims, Query(q): Query<StatsQuery>) -> ApiResult<Vec<db::DayStat>> {
    db::get_day_stats(&engine.pool, q.days.unwrap_or(30)).await.map(Json).map_err(internal)
}

// --- Config ---
