use super::*;



#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateWebhookRequest { pub url: String, pub events: Option<String>, pub secret: Option<String> }

#[utoipa::path(get, path = "/api/webhooks", responses((status = 200)), security(("bearer" = [])))]
pub async fn list_webhooks(State(engine): State<AppState>, claims: Claims) -> ApiResult<Vec<db::Webhook>> {
    db::list_webhooks(&engine.pool, claims.user_id).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/webhooks", responses((status = 201)), security(("bearer" = [])))]
pub async fn create_webhook(State(engine): State<AppState>, claims: Claims, Json(req): Json<CreateWebhookRequest>) -> Result<(StatusCode, Json<db::Webhook>), ApiError> {
    if req.url.trim().is_empty() { return Err(err(StatusCode::BAD_REQUEST, "URL cannot be empty")); }
    if req.url.len() > 2000 { return Err(err(StatusCode::BAD_REQUEST, "URL too long (max 2000 chars)")); }
    if !req.url.starts_with("http://") && !req.url.starts_with("https://") {
        return Err(err(StatusCode::BAD_REQUEST, "URL must start with http:// or https://"));
    }
    // Reject URLs with embedded credentials or suspicious patterns
    if let Ok(url) = url::Url::parse(&req.url) {
        if url.username() != "" || url.password().is_some() {
            return Err(err(StatusCode::BAD_REQUEST, "Webhook URL must not contain credentials"));
        }
        if let Some(host) = url.host_str() {
            let blocked = ["localhost", "127.0.0.1", "0.0.0.0", "::1", "[::1]"];
            if blocked.contains(&host) || host.starts_with("10.") || host.starts_with("192.168.")
                || host.starts_with("172.16.") || host.starts_with("172.17.") || host.starts_with("172.18.")
                || host.starts_with("172.19.") || host.starts_with("172.20.") || host.starts_with("172.21.")
                || host.starts_with("172.22.") || host.starts_with("172.23.") || host.starts_with("172.24.")
                || host.starts_with("172.25.") || host.starts_with("172.26.") || host.starts_with("172.27.")
                || host.starts_with("172.28.") || host.starts_with("172.29.") || host.starts_with("172.30.") || host.starts_with("172.31.")
                || host.starts_with("169.254.") || host.ends_with(".local") {
                return Err(err(StatusCode::BAD_REQUEST, "Webhook URL must not point to private/loopback addresses"));
            }
        }
    }
    let events = req.events.as_deref().unwrap_or("*");
    // V4: Limit webhooks per user
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM webhooks WHERE user_id = ?")
        .bind(claims.user_id).fetch_one(&engine.pool).await.map_err(internal)?;
    if count >= 50 { return Err(err(StatusCode::BAD_REQUEST, "Too many webhooks (max 50)")); }
    if events != "*" {
        const VALID_EVENTS: &[&str] = &["task.created", "task.updated", "task.deleted", "sprint.created", "sprint.started", "sprint.completed"];
        for ev in events.split(',') {
            if !VALID_EVENTS.contains(&ev.trim()) {
                return Err(err(StatusCode::BAD_REQUEST, format!("Unknown event '{}'. Valid: {}", ev.trim(), VALID_EVENTS.join(", "))));
            }
        }
    }
    let wh = db::create_webhook(&engine.pool, claims.user_id, &req.url, events, req.secret.as_deref()).await.map_err(internal)?;
    Ok((StatusCode::CREATED, Json(wh)))
}

#[utoipa::path(delete, path = "/api/webhooks/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_webhook(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    db::delete_webhook(&engine.pool, id, claims.user_id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}
