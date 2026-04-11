use super::*;


#[utoipa::path(post, path = "/api/auth/register", request_body = RegisterRequest, responses((status = 200, body = AuthResponse)))]
pub async fn register(headers: axum::http::HeaderMap, State(engine): State<AppState>, Json(req): Json<RegisterRequest>) -> ApiResult<AuthResponse> {
    check_auth_rate_limit(&headers).await?;
    if req.username.trim().is_empty() { return Err(err(StatusCode::BAD_REQUEST, "Username cannot be empty")); }
    if req.username.len() > 32 { return Err(err(StatusCode::BAD_REQUEST, "Username too long (max 32 chars)")); }
    if !req.username.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return Err(err(StatusCode::BAD_REQUEST, "Username must be alphanumeric (underscores and hyphens allowed)"));
    }
    if req.password.len() < 8 { return Err(err(StatusCode::BAD_REQUEST, "Password must be at least 8 characters")); }
    if !req.password.chars().any(|c| c.is_uppercase()) { return Err(err(StatusCode::BAD_REQUEST, "Password must contain an uppercase letter")); }
    if !req.password.chars().any(|c| c.is_ascii_digit()) { return Err(err(StatusCode::BAD_REQUEST, "Password must contain a digit")); }
    let pw = req.password.clone();
    let hash = tokio::task::spawn_blocking(move || bcrypt::hash(&pw, 12))
        .await.map_err(internal)?.map_err(internal)?;
    let user = db::create_user(&engine.pool, &req.username, &hash, "user").await
        .map_err(|_| err(StatusCode::CONFLICT, "Username already taken"))?;
    if let Err(e) = db::audit(&engine.pool, user.id, "register", "user", Some(user.id), None).await { tracing::warn!("Audit log failed: {}", e); }
    let token = auth::create_token(user.id, &user.username, &user.role).map_err(internal)?;
    Ok(Json(AuthResponse { token, user_id: user.id, username: user.username, role: user.role }))
}

#[utoipa::path(post, path = "/api/auth/login", request_body = LoginRequest, responses((status = 200, body = AuthResponse)))]
pub async fn login(headers: axum::http::HeaderMap, State(engine): State<AppState>, Json(req): Json<LoginRequest>) -> ApiResult<AuthResponse> {
    check_auth_rate_limit(&headers).await?;
    let user = db::get_user_by_username(&engine.pool, &req.username).await
        .map_err(|_| err(StatusCode::UNAUTHORIZED, "Invalid credentials"))?;
    let pw = req.password.clone();
    let hash = user.password_hash.clone();
    let valid = tokio::task::spawn_blocking(move || bcrypt::verify(&pw, &hash).unwrap_or(false))
        .await.map_err(internal)?;
    if !valid {
        return Err(err(StatusCode::UNAUTHORIZED, "Invalid credentials"));
    }
    let token = auth::create_token(user.id, &user.username, &user.role).map_err(internal)?;
    Ok(Json(AuthResponse { token, user_id: user.id, username: user.username, role: user.role }))
}

#[utoipa::path(post, path = "/api/auth/logout", responses((status = 204)), security(("bearer" = [])))]
pub async fn logout(headers: axum::http::HeaderMap) -> Result<StatusCode, ApiError> {
    let token = headers.get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "Missing token"))?;
    auth::revoke_token(token).await;
    Ok(StatusCode::NO_CONTENT)
}

// --- Timer ---
