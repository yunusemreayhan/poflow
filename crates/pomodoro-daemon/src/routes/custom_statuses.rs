use super::*;

const VALID_CATEGORIES: &[&str] = &["todo", "in_progress", "done"];

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateCustomStatusRequest {
    pub name: String,
    pub color: Option<String>,
    pub category: Option<String>,
    pub sort_order: Option<i64>,
}

#[utoipa::path(get, path = "/api/statuses", responses((status = 200, body = Vec<db::CustomStatus>)), security(("bearer" = [])))]
pub async fn list_custom_statuses(State(engine): State<AppState>, _claims: Claims) -> ApiResult<Vec<db::CustomStatus>> {
    db::list_custom_statuses(&engine.pool).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/statuses", request_body = CreateCustomStatusRequest, responses((status = 201, body = db::CustomStatus)), security(("bearer" = [])))]
pub async fn create_custom_status(State(engine): State<AppState>, claims: Claims, Json(req): Json<CreateCustomStatusRequest>) -> Result<(StatusCode, Json<db::CustomStatus>), ApiError> {
    if claims.role != "root" { return Err(err(StatusCode::FORBIDDEN, "Only root can create statuses")); }
    let name = req.name.trim().to_lowercase().replace(' ', "_");
    if name.is_empty() { return Err(err(StatusCode::BAD_REQUEST, "Status name cannot be empty")); }
    if name.len() > 50 { return Err(err(StatusCode::BAD_REQUEST, "Status name too long (max 50)")); }
    if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(err(StatusCode::BAD_REQUEST, "Status name must be alphanumeric/underscore"));
    }
    // Don't allow overriding built-in statuses
    if VALID_TASK_STATUSES.contains(&name.as_str()) {
        return Err(err(StatusCode::CONFLICT, "Cannot override built-in status"));
    }
    let category = req.category.as_deref().unwrap_or("todo");
    if !VALID_CATEGORIES.contains(&category) {
        return Err(err(StatusCode::BAD_REQUEST, format!("Category must be one of: {}", VALID_CATEGORIES.join(", "))));
    }
    let color = req.color.as_deref().unwrap_or("#6366f1");
    let sort_order = req.sort_order.unwrap_or(0);
    let status = db::create_custom_status(&engine.pool, &name, color, category, sort_order, claims.user_id).await
        .map_err(|e| if e.to_string().contains("UNIQUE") { err(StatusCode::CONFLICT, "Status name already exists") } else { internal(e) })?;
    Ok((StatusCode::CREATED, Json(status)))
}

#[utoipa::path(put, path = "/api/statuses/{id}", request_body = CreateCustomStatusRequest, responses((status = 200, body = db::CustomStatus)), security(("bearer" = [])))]
pub async fn update_custom_status(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<CreateCustomStatusRequest>) -> ApiResult<db::CustomStatus> {
    if claims.role != "root" { return Err(err(StatusCode::FORBIDDEN, "Only root can update statuses")); }
    let name = req.name.trim().to_lowercase().replace(' ', "_");
    if name.is_empty() { return Err(err(StatusCode::BAD_REQUEST, "Status name cannot be empty")); }
    if name.len() > 50 { return Err(err(StatusCode::BAD_REQUEST, "Status name too long (max 50)")); }
    let category = req.category.as_deref().unwrap_or("todo");
    if !VALID_CATEGORIES.contains(&category) {
        return Err(err(StatusCode::BAD_REQUEST, format!("Category must be one of: {}", VALID_CATEGORIES.join(", "))));
    }
    let color = req.color.as_deref().unwrap_or("#6366f1");
    let sort_order = req.sort_order.unwrap_or(0);
    db::update_custom_status(&engine.pool, id, &name, color, category, sort_order).await.map(Json)
        .map_err(|e| if e.to_string().contains("not found") { err(StatusCode::NOT_FOUND, "Status not found") } else { internal(e) })
}

#[utoipa::path(delete, path = "/api/statuses/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_custom_status(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    if claims.role != "root" { return Err(err(StatusCode::FORBIDDEN, "Only root can delete statuses")); }
    db::delete_custom_status(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Status not found"))?;
    Ok(StatusCode::NO_CONTENT)
}
