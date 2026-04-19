use super::*;

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateProjectRequest {
    pub name: String,
    pub description: Option<String>,
    pub key: Option<String>,
    pub lead_user_id: Option<i64>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateProjectRequest {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub description: Option<Option<String>>,
    pub key: Option<String>,
    #[serde(default)]
    pub lead_user_id: Option<Option<i64>>,
    pub status: Option<String>,
}

fn generate_key(name: &str) -> String {
    let key: String = name.trim().to_uppercase().chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
        .collect::<String>()
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(6)
        .collect();
    if key.is_empty() { "PROJ".to_string() } else { key }
}

#[utoipa::path(get, path = "/api/projects", responses((status = 200)), security(("bearer" = [])))]
pub async fn list_projects(State(engine): State<AppState>, _claims: Claims) -> ApiResult<Vec<db::Project>> {
    db::list_projects(&engine.pool).await.map(Json).map_err(internal)
}

#[utoipa::path(get, path = "/api/projects/{id}", responses((status = 200)), security(("bearer" = [])))]
pub async fn get_project(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<db::Project> {
    db::get_project(&engine.pool, id).await.map(Json).map_err(|_| err(StatusCode::NOT_FOUND, "Project not found"))
}

#[utoipa::path(post, path = "/api/projects", responses((status = 201)), security(("bearer" = [])))]
pub async fn create_project(State(engine): State<AppState>, claims: Claims, Json(req): Json<CreateProjectRequest>) -> Result<(StatusCode, Json<db::Project>), ApiError> {
    if !auth::is_admin_or_root(&claims) { return Err(err(StatusCode::FORBIDDEN, "Admin or root required")); }
    let name = req.name.trim().to_string();
    if name.is_empty() { return Err(err(StatusCode::BAD_REQUEST, "Project name cannot be empty")); }
    if name.len() > 200 { return Err(err(StatusCode::BAD_REQUEST, "Project name too long (max 200)")); }
    let key = req.key.as_deref().map(|k| k.trim().to_uppercase()).unwrap_or_else(|| generate_key(&name));
    if key.is_empty() || key.len() > 10 { return Err(err(StatusCode::BAD_REQUEST, "Key must be 1-10 characters")); }
    if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(err(StatusCode::BAD_REQUEST, "Key must be alphanumeric (with - or _)"));
    }
    let project = db::create_project(&engine.pool, &name, req.description.as_deref(), &key, req.lead_user_id).await
        .map_err(|e| if e.to_string().contains("UNIQUE") { err(StatusCode::CONFLICT, "Project key already exists") } else { internal(e) })?;
    Ok((StatusCode::CREATED, Json(project)))
}

#[utoipa::path(put, path = "/api/projects/{id}", responses((status = 200)), security(("bearer" = [])))]
pub async fn update_project(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<UpdateProjectRequest>) -> ApiResult<db::Project> {
    if !auth::is_admin_or_root(&claims) { return Err(err(StatusCode::FORBIDDEN, "Admin or root required")); }
    if let Some(ref name) = req.name {
        if name.trim().is_empty() { return Err(err(StatusCode::BAD_REQUEST, "Project name cannot be empty")); }
        if name.len() > 200 { return Err(err(StatusCode::BAD_REQUEST, "Project name too long (max 200)")); }
    }
    if let Some(ref key) = req.key {
        let key = key.trim();
        if key.is_empty() || key.len() > 10 { return Err(err(StatusCode::BAD_REQUEST, "Key must be 1-10 characters")); }
        if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(err(StatusCode::BAD_REQUEST, "Key must be alphanumeric (with - or _)"));
        }
    }
    if let Some(ref status) = req.status {
        if !matches!(status.as_str(), "active" | "archived") {
            return Err(err(StatusCode::BAD_REQUEST, "Status must be 'active' or 'archived'"));
        }
    }
    let desc = req.description.as_ref().map(|o| o.as_deref());
    db::update_project(&engine.pool, id, req.name.as_deref(), desc, req.key.as_deref(), req.lead_user_id, req.status.as_deref()).await
        .map(Json).map_err(|e| if e.to_string().contains("UNIQUE") { err(StatusCode::CONFLICT, "Project key already exists") }
            else if e.to_string().contains("not found") { err(StatusCode::NOT_FOUND, "Project not found") }
            else { internal(e) })
}

#[utoipa::path(delete, path = "/api/projects/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_project(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    if !auth::is_admin_or_root(&claims) { return Err(err(StatusCode::FORBIDDEN, "Admin or root required")); }
    db::delete_project(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Project not found"))?;
    Ok(StatusCode::NO_CONTENT)
}
