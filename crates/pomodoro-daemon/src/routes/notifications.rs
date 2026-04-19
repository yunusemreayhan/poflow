use super::*;
use crate::engine::ChangeEvent;

#[utoipa::path(get, path = "/api/notifications", responses((status = 200, body = Vec<db::Notification>)), security(("bearer" = [])))]
pub async fn list_notifications(State(engine): State<AppState>, claims: Claims, Query(q): Query<std::collections::HashMap<String, String>>) -> ApiResult<Vec<db::Notification>> {
    let limit = q.get("limit").and_then(|l| l.parse().ok()).unwrap_or(50i64).min(200);
    db::list_notifications(&engine.pool, claims.user_id, limit).await.map(Json).map_err(internal)
}

#[utoipa::path(get, path = "/api/notifications/unread", responses((status = 200)), security(("bearer" = [])))]
pub async fn unread_count(State(engine): State<AppState>, claims: Claims) -> ApiResult<serde_json::Value> {
    let count = db::unread_count(&engine.pool, claims.user_id).await.map_err(internal)?;
    Ok(Json(serde_json::json!({ "count": count })))
}

#[utoipa::path(post, path = "/api/notifications/read", responses((status = 204)), security(("bearer" = [])))]
pub async fn mark_notifications_read(State(engine): State<AppState>, claims: Claims, body: Option<Json<serde_json::Value>>) -> Result<StatusCode, ApiError> {
    let id = body.and_then(|b| b.get("id").and_then(|v| v.as_i64()));
    db::mark_read(&engine.pool, claims.user_id, id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Notifications);
    Ok(StatusCode::NO_CONTENT)
}
