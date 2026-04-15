use super::*;


#[utoipa::path(put, path = "/api/profile", request_body = UpdateProfileRequest, responses((status = 200, body = AuthResponse)), security(("bearer" = [])))]
pub async fn update_profile(State(engine): State<AppState>, claims: Claims, Json(req): Json<UpdateProfileRequest>) -> ApiResult<AuthResponse> {
    // B10: Validate password BEFORE making any changes
    if let Some(ref p) = req.password {
        let current_pw = req.current_password.as_ref()
            .ok_or_else(|| err(StatusCode::BAD_REQUEST, "current_password is required to change password"))?;
        let user = db::get_user(&engine.pool, claims.user_id).await.map_err(internal)?;
        let hash_check = user.password_hash.clone();
        let pw_check = current_pw.clone();
        let valid = tokio::task::spawn_blocking(move || bcrypt::verify(&pw_check, &hash_check).unwrap_or(false))
            .await.map_err(internal)?;
        if !valid { return Err(err(StatusCode::FORBIDDEN, "Current password is incorrect")); }
        validate_password(p)?;
    }
    if let Some(ref u) = req.username {
        validate_username(u)?;
        db::update_username(&engine.pool, claims.user_id, u).await
            .map_err(|e| if e.to_string().contains("already taken") { err(StatusCode::CONFLICT, "Username already taken") } else { internal(e) })?;
    }
    if let Some(ref p) = req.password {
        let pw = p.clone();
        let hash = tokio::task::spawn_blocking(move || bcrypt::hash(&pw, 12))
            .await.map_err(internal)?.map_err(internal)?;
        db::update_user_password(&engine.pool, claims.user_id, &hash).await.map_err(internal)?;
        auth::invalidate_user_cache(&engine.user_auth_cache, claims.user_id).await;
    }
    if let Some(ref email) = req.email {
        let email_trimmed = email.trim();
        if !email_trimmed.is_empty() && !email_trimmed.contains('@') {
            return Err(err(StatusCode::BAD_REQUEST, "Invalid email address"));
        }
        let val = if email_trimmed.is_empty() { None } else { Some(email_trimmed) };
        sqlx::query("UPDATE users SET email = ? WHERE id = ?").bind(val).bind(claims.user_id).execute(&engine.pool).await.map_err(internal)?;
    }
    let user = db::get_user(&engine.pool, claims.user_id).await.map_err(internal)?;
    let token = auth::create_token(user.id, &user.username, &user.role).map_err(internal)?;
    let refresh_token = auth::create_refresh_token(user.id, &user.username, &user.role).map_err(internal)?;
    Ok(Json(AuthResponse { token, refresh_token, user_id: user.id, username: user.username, role: user.role }))
}

// --- Admin ---

// F12: Notification preferences per event type
const EVENT_TYPES: &[&str] = &["task_assigned", "task_completed", "comment_added", "sprint_started", "sprint_completed", "time_logged", "task_added_to_sprint"];

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct NotifPref { pub event_type: String, pub enabled: bool }

#[utoipa::path(get, path = "/api/profile/notifications", responses((status = 200)), security(("bearer" = [])))]
pub async fn get_notif_prefs(State(engine): State<AppState>, claims: Claims) -> ApiResult<Vec<NotifPref>> {
    let rows: Vec<(String, bool)> = sqlx::query_as("SELECT event_type, enabled FROM notification_prefs WHERE user_id = ?")
        .bind(claims.user_id).fetch_all(&engine.pool).await.map_err(internal)?;
    let map: std::collections::HashMap<String, bool> = rows.into_iter().collect();
    let prefs = EVENT_TYPES.iter().map(|e| NotifPref { event_type: e.to_string(), enabled: *map.get(*e).unwrap_or(&true) }).collect();
    Ok(Json(prefs))
}

#[utoipa::path(put, path = "/api/profile/notifications", responses((status = 200)), security(("bearer" = [])))]
pub async fn update_notif_prefs(State(engine): State<AppState>, claims: Claims, Json(prefs): Json<Vec<NotifPref>>) -> Result<StatusCode, ApiError> {
    if prefs.len() > EVENT_TYPES.len() { return Err(err(StatusCode::BAD_REQUEST, format!("Too many prefs (max {})", EVENT_TYPES.len()))); }
    for p in &prefs {
        if !EVENT_TYPES.contains(&p.event_type.as_str()) { return Err(err(StatusCode::BAD_REQUEST, format!("Unknown event type: {}", p.event_type))); }
        sqlx::query("INSERT INTO notification_prefs (user_id, event_type, enabled) VALUES (?, ?, ?) ON CONFLICT(user_id, event_type) DO UPDATE SET enabled = excluded.enabled")
            .bind(claims.user_id).bind(&p.event_type).bind(p.enabled).execute(&engine.pool).await.map_err(internal)?;
    }
    Ok(StatusCode::OK)
}
