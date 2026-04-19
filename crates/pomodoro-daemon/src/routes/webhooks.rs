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
            let is_blocked = blocked.contains(&host) || host.ends_with(".local")
                || host.parse::<std::net::IpAddr>().map(|ip| crate::webhook::is_private_ip_pub(&ip)).unwrap_or(false);
            if is_blocked {
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
    db::delete_webhook(&engine.pool, id, claims.user_id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Webhook not found"))?;
    Ok(StatusCode::NO_CONTENT)
}

// V32-15: Update webhook URL/events
#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateWebhookRequest { pub url: Option<String>, pub events: Option<String>, pub active: Option<bool> }

#[utoipa::path(put, path = "/api/webhooks/{id}", responses((status = 200)), security(("bearer" = [])))]
pub async fn update_webhook(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<UpdateWebhookRequest>) -> ApiResult<db::Webhook> {
    let wh: (i64,) = sqlx::query_as("SELECT user_id FROM webhooks WHERE id = ?")
        .bind(id).fetch_one(&engine.pool).await.map_err(|_| err(StatusCode::NOT_FOUND, "Webhook not found"))?;
    if !is_owner_or_root(wh.0, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    if let Some(ref url) = req.url {
        if !url.starts_with("http://") && !url.starts_with("https://") { return Err(err(StatusCode::BAD_REQUEST, "URL must start with http:// or https://")); }
        if let Ok(parsed) = url::Url::parse(url) {
            if parsed.username() != "" || parsed.password().is_some() {
                return Err(err(StatusCode::BAD_REQUEST, "Webhook URL must not contain credentials"));
            }
            if let Some(host) = parsed.host_str() {
                let blocked = ["localhost", "127.0.0.1", "0.0.0.0", "::1", "[::1]"];
                let is_blocked = blocked.contains(&host) || host.ends_with(".local")
                    || host.parse::<std::net::IpAddr>().map(|ip| crate::webhook::is_private_ip_pub(&ip)).unwrap_or(false);
                if is_blocked {
                    return Err(err(StatusCode::BAD_REQUEST, "Webhook URL must not point to private/loopback addresses"));
                }
            }
        }
    }
    if let Some(ref events) = req.events {
        if events != "*" {
            const VALID_EVENTS: &[&str] = &["task.created", "task.updated", "task.deleted", "sprint.created", "sprint.started", "sprint.completed"];
            for ev in events.split(',') {
                if !VALID_EVENTS.contains(&ev.trim()) {
                    return Err(err(StatusCode::BAD_REQUEST, format!("Unknown event '{}'. Valid: {}", ev.trim(), VALID_EVENTS.join(", "))));
                }
            }
        }
    }
    let mut sql_parts = Vec::new();
    if req.url.is_some() { sql_parts.push("url = ?"); }
    if req.events.is_some() { sql_parts.push("events = ?"); }
    if req.active.is_some() { sql_parts.push("active = ?"); }
    if sql_parts.is_empty() { return Err(err(StatusCode::BAD_REQUEST, "No fields to update")); }
    let sql = format!("UPDATE webhooks SET {} WHERE id = ? AND user_id = ?", sql_parts.join(", "));
    let mut q = sqlx::query(&sql);
    if let Some(ref url) = req.url { q = q.bind(url); }
    if let Some(ref events) = req.events { q = q.bind(events); }
    if let Some(active) = req.active { q = q.bind(if active { 1i64 } else { 0 }); }
    q = q.bind(id).bind(claims.user_id);
    q.execute(&engine.pool).await.map_err(internal)?;
    let updated = sqlx::query_as::<_, db::Webhook>("SELECT * FROM webhooks WHERE id = ?").bind(id).fetch_one(&engine.pool).await.map_err(internal)?;
    Ok(Json(updated))
}
