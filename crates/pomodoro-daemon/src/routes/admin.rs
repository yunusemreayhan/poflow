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
    Ok(StatusCode::NO_CONTENT)
}

// --- Task votes ---
