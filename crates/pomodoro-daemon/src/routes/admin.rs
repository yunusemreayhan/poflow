use super::*;


#[utoipa::path(get, path = "/api/admin/users", responses((status = 200, body = Vec<db::User>)), security(("bearer" = [])))]
pub async fn list_users(State(engine): State<AppState>, claims: Claims) -> Result<Json<Vec<db::User>>, ApiError> {
    if claims.role != "root" { return Err(err(StatusCode::FORBIDDEN, "Root only")); }
    db::list_users(&engine.pool).await.map(Json).map_err(internal)
}

#[utoipa::path(put, path = "/api/admin/users/{id}/role", request_body = UpdateRoleRequest, responses((status = 200, body = db::User)), security(("bearer" = [])))]
pub async fn update_user_role(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<UpdateRoleRequest>) -> ApiResult<db::User> {
    if claims.role != "root" { return Err(err(StatusCode::FORBIDDEN, "Root only")); }
    if !VALID_ROLES.contains(&req.role.as_str()) { return Err(err(StatusCode::BAD_REQUEST, format!("Invalid role '{}'. Must be one of: {}", req.role, VALID_ROLES.join(", ")))); }
    db::update_user_role(&engine.pool, id, &req.role).await.map(Json).map_err(internal)
}

#[utoipa::path(delete, path = "/api/admin/users/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_user(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    if claims.role != "root" { return Err(err(StatusCode::FORBIDDEN, "Root only")); }
    if claims.user_id == id { return Err(err(StatusCode::BAD_REQUEST, "Cannot delete yourself")); }
    db::delete_user(&engine.pool, id).await.map_err(|e| err(StatusCode::BAD_REQUEST, e.to_string()))?;
    // Invalidate user cache so deleted user's tokens are rejected immediately
    auth::invalidate_user_cache(id).await;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct AdminResetPasswordRequest { pub password: String }

#[utoipa::path(put, path = "/api/admin/users/{id}/password", request_body = AdminResetPasswordRequest, responses((status = 204)), security(("bearer" = [])))]
pub async fn admin_reset_password(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<AdminResetPasswordRequest>) -> Result<StatusCode, ApiError> {
    if claims.role != "root" { return Err(err(StatusCode::FORBIDDEN, "Root only")); }
    validate_password(&req.password)?;
    let pw = req.password.clone();
    let hash = tokio::task::spawn_blocking(move || bcrypt::hash(&pw, 12))
        .await.map_err(internal)?.map_err(internal)?;
    db::update_user_password(&engine.pool, id, &hash).await.map_err(internal)?;
    if let Err(e) = db::audit(&engine.pool, claims.user_id, "admin_reset_password", "user", Some(id), None).await { tracing::warn!("Audit log failed: {}", e); }
    Ok(StatusCode::NO_CONTENT)
}

// --- Task votes ---

#[utoipa::path(post, path = "/api/admin/backup", responses((status = 200)), security(("bearer" = [])))]
pub async fn create_backup(State(engine): State<AppState>, claims: Claims) -> Result<axum::response::Response, ApiError> {
    if claims.role != "root" { return Err(err(StatusCode::FORBIDDEN, "Root only")); }
    let db_path = db::db_path();
    let backup_dir = db_path.parent().unwrap_or(std::path::Path::new("/tmp")).join("backups");
    std::fs::create_dir_all(&backup_dir).map_err(|e| internal(format!("Failed to create backup dir: {}", e)))?;
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let backup_path = backup_dir.join(format!("pomodoro_{}.db", timestamp));
    // B1: Sanitize path to prevent SQL injection — escape single quotes
    let path_str = backup_path.display().to_string().replace('\'', "''");
    sqlx::query(&format!("VACUUM INTO '{}'", path_str))
        .execute(&engine.pool).await.map_err(|e| internal(format!("Backup failed: {}", e)))?;
    // S1: Restrict backup file permissions
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&backup_path, std::fs::Permissions::from_mode(0o600)).ok();
    }
    let size = std::fs::metadata(&backup_path).map(|m| m.len()).unwrap_or(0);
    // O3: Retain only last 10 backups
    if let Ok(entries) = std::fs::read_dir(&backup_dir) {
        let mut backups: Vec<_> = entries.filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().starts_with("pomodoro_") && e.file_name().to_string_lossy().ends_with(".db")).collect();
        backups.sort_by_key(|e| std::cmp::Reverse(e.file_name()));
        for old in backups.into_iter().skip(10) { std::fs::remove_file(old.path()).ok(); }
    }
    Ok(axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(serde_json::to_vec(&serde_json::json!({
            "path": backup_path.display().to_string(),
            "size_bytes": size,
        })).map_err(|e| internal(e.to_string()))?))
        .map_err(|e| internal(e.to_string()))?)
}

// O3: List available backups
#[utoipa::path(get, path = "/api/admin/backups", responses((status = 200)), security(("bearer" = [])))]
pub async fn list_backups(claims: Claims) -> ApiResult<Vec<serde_json::Value>> {
    if claims.role != "root" { return Err(err(StatusCode::FORBIDDEN, "Root only")); }
    let backup_dir = db::db_path().parent().unwrap_or(std::path::Path::new("/tmp")).join("backups");
    let mut backups = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&backup_dir) {
        for e in entries.filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("pomodoro_") && name.ends_with(".db") {
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                backups.push(serde_json::json!({"filename": name, "size_bytes": size}));
            }
        }
    }
    backups.sort_by(|a, b| b["filename"].as_str().cmp(&a["filename"].as_str()));
    Ok(Json(backups))
}

// O3: Restore from backup
#[derive(Deserialize, utoipa::ToSchema)]
pub struct RestoreRequest { pub filename: String }

#[utoipa::path(post, path = "/api/admin/restore", responses((status = 200)), security(("bearer" = [])))]
pub async fn restore_backup(State(engine): State<AppState>, claims: Claims, Json(req): Json<RestoreRequest>) -> Result<axum::response::Response, ApiError> {
    if claims.role != "root" { return Err(err(StatusCode::FORBIDDEN, "Root only")); }
    // Validate filename — must be alphanumeric + underscore + .db only
    if !req.filename.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.') || !req.filename.ends_with(".db") || req.filename.contains("..") {
        return Err(err(StatusCode::BAD_REQUEST, "Invalid backup filename"));
    }
    let backup_dir = db::db_path().parent().unwrap_or(std::path::Path::new("/tmp")).join("backups");
    let backup_path = backup_dir.join(&req.filename);
    if !backup_path.exists() { return Err(err(StatusCode::NOT_FOUND, "Backup not found")); }
    // Create a safety backup before restoring
    let safety = backup_dir.join(format!("pre_restore_{}.db", chrono::Utc::now().format("%Y%m%d_%H%M%S")));
    let safety_str = safety.display().to_string().replace('\'', "''");
    sqlx::query(&format!("VACUUM INTO '{}'", safety_str)).execute(&engine.pool).await.map_err(|e| internal(format!("Safety backup failed: {}", e)))?;
    // B8: Checkpoint WAL before overwriting to ensure consistency
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)").execute(&engine.pool).await.map_err(|e| internal(format!("WAL checkpoint failed: {}", e)))?;
    // Restore: copy backup over current DB
    let db_path = db::db_path();
    std::fs::copy(&backup_path, &db_path).map_err(|e| internal(format!("Restore failed: {}", e)))?;
    Ok(axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(serde_json::to_vec(&serde_json::json!({
            "restored_from": req.filename,
            "safety_backup": safety.display().to_string(),
            "note": "Restart the server to use the restored database"
        })).map_err(|e| internal(e.to_string()))?))
        .map_err(|e| internal(e.to_string()))?)
}
