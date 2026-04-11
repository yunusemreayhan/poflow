use super::*;


#[utoipa::path(post, path = "/api/auth/register", request_body = RegisterRequest, responses((status = 200, body = AuthResponse)))]
pub async fn register(headers: axum::http::HeaderMap, State(engine): State<AppState>, Json(req): Json<RegisterRequest>) -> ApiResult<AuthResponse> {
    check_auth_rate_limit(&headers)?;
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

#[utoipa::path(post, path = "/api/auth/login", request_body = LoginRequest, responses((status = 200, body = AuthResponse)))]
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
            db::update_user_password(&engine.pool, user.id, &new_hash).await.ok();
        }
    }
    let token = auth::create_token(user.id, &user.username, &user.role).map_err(internal)?;
    let refresh_token = auth::create_refresh_token(user.id, &user.username, &user.role).map_err(internal)?;
    Ok(Json(AuthResponse { token, refresh_token, user_id: user.id, username: user.username, role: user.role }))
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

#[derive(Deserialize, utoipa::ToSchema)]
pub struct RefreshRequest { pub refresh_token: String }

#[utoipa::path(post, path = "/api/auth/refresh", responses((status = 200)), security(()))]
pub async fn refresh_token(State(engine): State<AppState>, headers: axum::http::HeaderMap, Json(req): Json<RefreshRequest>) -> ApiResult<AuthResponse> {
    check_auth_rate_limit(&headers)?;
    if auth::is_revoked(&req.refresh_token).await {
        return Err(err(StatusCode::UNAUTHORIZED, "Token revoked"));
    }
    let claims = auth::verify_token(&req.refresh_token).map_err(|_| err(StatusCode::UNAUTHORIZED, "Invalid refresh token"))?;
    if claims.typ != "refresh" { return Err(err(StatusCode::UNAUTHORIZED, "Not a refresh token")); }
    // Re-fetch user from DB to get current role/username (not stale claims)
    let user = db::get_user(&engine.pool, claims.user_id).await.map_err(|_| err(StatusCode::UNAUTHORIZED, "User not found"))?;
    let token = auth::create_token(user.id, &user.username, &user.role).map_err(internal)?;
    let refresh_token = auth::create_refresh_token(user.id, &user.username, &user.role).map_err(internal)?;
    auth::revoke_token(&req.refresh_token).await;
    Ok(Json(AuthResponse { token, refresh_token, user_id: user.id, username: user.username, role: user.role }))
}

// --- Timer ---
