use super::*;


#[utoipa::path(post, path = "/api/auth/register", request_body = RegisterRequest, responses((status = 200, body = AuthResponse), (status = 400, body = ApiErrorBody), (status = 409, body = ApiErrorBody), (status = 429, body = ApiErrorBody)))]
pub async fn register(headers: axum::http::HeaderMap, State(engine): State<AppState>, Json(req): Json<RegisterRequest>) -> ApiResult<AuthResponse> {
    check_auth_rate_limit(&headers)?;
    // Check if registration is disabled (env var overrides config)
    let allowed = match std::env::var("POFLOW_ALLOW_REGISTRATION") {
        Ok(v) => v != "0" && v != "false",
        Err(_) => engine.config.lock().await.allow_registration,
    };
    if !allowed { return Err(err(StatusCode::FORBIDDEN, "Registration is disabled")); }
    validate_username(&req.username)?;
    validate_password(&req.password)?;
    let pw = req.password.clone();
    let hash = tokio::task::spawn_blocking(move || bcrypt::hash(&pw, 12))
        .await.map_err(internal)?.map_err(internal)?;
    let user = db::create_user(&engine.pool, &req.username, &hash, "user").await
        .map_err(|_| err(StatusCode::CONFLICT, "Username already taken"))?;
    if let Err(e) = db::audit(&engine.pool, user.id, "register", "user", Some(user.id), None).await { tracing::warn!("Audit log failed: {}", e); }
    let token = auth::create_token(user.id, &user.username, &user.role).map_err(internal)?;
    let refresh_token = auth::create_refresh_token(user.id, &user.username, &user.role).map_err(internal)?;
    Ok(Json(AuthResponse { token, refresh_token, user_id: user.id, username: user.username, role: user.role }))
}

#[utoipa::path(post, path = "/api/auth/login", request_body = LoginRequest, responses((status = 200, body = AuthResponse), (status = 401, body = ApiErrorBody), (status = 429, body = ApiErrorBody)))]
pub async fn login(headers: axum::http::HeaderMap, State(engine): State<AppState>, Json(req): Json<LoginRequest>) -> ApiResult<AuthResponse> {
    check_auth_rate_limit(&headers)?;
    let user = db::get_user_by_username(&engine.pool, &req.username).await
        .map_err(|_| err(StatusCode::UNAUTHORIZED, "Invalid credentials"))?;
    let pw = req.password.clone();
    let hash = user.password_hash.clone();
    let valid = tokio::task::spawn_blocking(move || bcrypt::verify(&pw, &hash).unwrap_or(false))
        .await.map_err(internal)?;
    if !valid {
        return Err(err(StatusCode::UNAUTHORIZED, "Invalid credentials"));
    }
    // Rehash if bcrypt cost is outdated (upgrade path)
    let current_cost = user.password_hash.split('$').nth(2).and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
    if current_cost < 12 {
        let pw2 = req.password.clone();
        if let Ok(new_hash) = tokio::task::spawn_blocking(move || bcrypt::hash(&pw2, 12)).await.map_err(internal)? {
            // S2: Use rehash (no password_changed_at update) to avoid invalidating the fresh token
            db::rehash_user_password(&engine.pool, user.id, &new_hash).await.ok();
        }
    }
    let token = auth::create_token(user.id, &user.username, &user.role).map_err(internal)?;
    let refresh_token = auth::create_refresh_token(user.id, &user.username, &user.role).map_err(internal)?;
    Ok(Json(AuthResponse { token, refresh_token, user_id: user.id, username: user.username, role: user.role }))
}

#[derive(Deserialize, Default, utoipa::ToSchema)]
pub struct LogoutRequest {
    pub refresh_token: Option<String>,
}

#[utoipa::path(post, path = "/api/auth/logout", request_body(content = Option<LogoutRequest>, content_type = "application/json"), responses((status = 204)), security(("bearer" = [])))]
pub async fn logout(headers: axum::http::HeaderMap, body: axum::body::Bytes) -> Result<StatusCode, ApiError> {
    let token = headers.get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "Missing token"))?;
    auth::revoke_token(token).await;
    // Also revoke refresh token if provided in body
    if let Ok(req) = serde_json::from_slice::<LogoutRequest>(&body) {
        if let Some(rt) = req.refresh_token {
            if !rt.is_empty() {
                auth::revoke_token(&rt).await;
            }
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct RefreshRequest { pub refresh_token: String }

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ChangePasswordRequest { pub current_password: String, pub new_password: String }

#[utoipa::path(put, path = "/api/auth/password", responses((status = 204)), security(("bearer" = [])))]
pub async fn change_password(State(engine): State<AppState>, claims: Claims, Json(req): Json<ChangePasswordRequest>) -> Result<StatusCode, ApiError> {
    validate_password(&req.new_password)?;
    let user = db::get_user(&engine.pool, claims.user_id).await.map_err(|_| err(StatusCode::NOT_FOUND, "User not found"))?;
    let hash = user.password_hash.clone();
    let pw = req.current_password.clone();
    let valid = tokio::task::spawn_blocking(move || bcrypt::verify(&pw, &hash).unwrap_or(false)).await.map_err(internal)?;
    if !valid { return Err(err(StatusCode::UNAUTHORIZED, "Current password is incorrect")); }
    let new_pw = req.new_password.clone();
    let new_hash = tokio::task::spawn_blocking(move || bcrypt::hash(&new_pw, 12)).await.map_err(internal)?.map_err(internal)?;
    db::update_user_password(&engine.pool, claims.user_id, &new_hash).await.map_err(internal)?;
    // S1: Invalidate user cache so existing tokens are re-validated against password_changed_at
    auth::invalidate_user_cache(&engine.user_auth_cache, claims.user_id).await;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/auth/refresh", responses((status = 200)), security(()))]
pub async fn refresh_token(State(engine): State<AppState>, headers: axum::http::HeaderMap, Json(req): Json<RefreshRequest>) -> ApiResult<AuthResponse> {
    check_auth_rate_limit(&headers)?;
    if auth::is_revoked(&req.refresh_token).await {
        return Err(err(StatusCode::UNAUTHORIZED, "Token revoked"));
    }
    // Revoke old token BEFORE issuing new ones to prevent replay
    auth::revoke_token(&req.refresh_token).await;
    let claims = auth::verify_token(&req.refresh_token).map_err(|_| err(StatusCode::UNAUTHORIZED, "Invalid refresh token"))?;
    if claims.typ != "refresh" { return Err(err(StatusCode::UNAUTHORIZED, "Not a refresh token")); }
    // Re-fetch user from DB to get current role/username (not stale claims)
    let user = db::get_user(&engine.pool, claims.user_id).await.map_err(|_| err(StatusCode::UNAUTHORIZED, "User not found"))?;
    let token = auth::create_token(user.id, &user.username, &user.role).map_err(internal)?;
    let refresh_token = auth::create_refresh_token(user.id, &user.username, &user.role).map_err(internal)?;
    Ok(Json(AuthResponse { token, refresh_token, user_id: user.id, username: user.username, role: user.role }))
}

// --- Timer ---
