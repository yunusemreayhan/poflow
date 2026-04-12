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
    Ok(axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(serde_json::to_vec(&serde_json::json!({
            "path": backup_path.display().to_string(),
            "size_bytes": size,
        })).unwrap()))
        .map_err(|e| internal(e.to_string()))?)
}
