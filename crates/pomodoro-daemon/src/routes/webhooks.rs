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
    if !req.url.starts_with("http://") && !req.url.starts_with("https://") {
        return Err(err(StatusCode::BAD_REQUEST, "URL must start with http:// or https://"));
    }
    // Block private/loopback IPs to prevent SSRF
    if let Ok(url) = url::Url::parse(&req.url) {
        if let Some(host) = url.host_str() {
            let blocked = ["localhost", "127.0.0.1", "0.0.0.0", "::1", "[::1]"];
            if blocked.contains(&host) || host.starts_with("10.") || host.starts_with("192.168.") || host.starts_with("172.") {
                return Err(err(StatusCode::BAD_REQUEST, "Webhook URL must not point to private/loopback addresses"));
            }
        }
    }
    let events = req.events.as_deref().unwrap_or("*");
    let wh = db::create_webhook(&engine.pool, claims.user_id, &req.url, events, req.secret.as_deref()).await.map_err(internal)?;
    Ok((StatusCode::CREATED, Json(wh)))
}

#[utoipa::path(delete, path = "/api/webhooks/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_webhook(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    db::delete_webhook(&engine.pool, id, claims.user_id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}
