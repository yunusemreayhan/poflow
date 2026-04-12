use super::*;

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateTemplateRequest { pub name: String, pub data: serde_json::Value }

#[utoipa::path(get, path = "/api/templates", responses((status = 200)), security(("bearer" = [])))]
pub async fn list_templates(State(engine): State<AppState>, claims: Claims) -> ApiResult<Vec<db::TaskTemplate>> {
    db::list_templates(&engine.pool, claims.user_id).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/templates", responses((status = 201)), security(("bearer" = [])))]
pub async fn create_template(State(engine): State<AppState>, claims: Claims, Json(req): Json<CreateTemplateRequest>) -> Result<(StatusCode, Json<db::TaskTemplate>), ApiError> {
    if req.name.trim().is_empty() { return Err(err(StatusCode::BAD_REQUEST, "Name required")); }
    if req.name.len() > 200 { return Err(err(StatusCode::BAD_REQUEST, "Name too long (max 200 chars)")); }
    let data = serde_json::to_string(&req.data).map_err(internal)?;
    if data.len() > 65536 { return Err(err(StatusCode::BAD_REQUEST, "Template data too large (max 64KB)")); }
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM task_templates WHERE user_id = ?")
        .bind(claims.user_id).fetch_one(&engine.pool).await.map_err(internal)?;
    if count.0 >= 100 { return Err(err(StatusCode::BAD_REQUEST, "Template limit reached (max 100)")); }
    let t = db::create_template(&engine.pool, claims.user_id, req.name.trim(), &data).await.map_err(internal)?;
    Ok((StatusCode::CREATED, Json(t)))
}

#[utoipa::path(delete, path = "/api/templates/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_template(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    // S9: Verify ownership — templates are per-user
    let tmpl: (i64,) = sqlx::query_as("SELECT user_id FROM task_templates WHERE id = ?")
        .bind(id).fetch_one(&engine.pool).await.map_err(|_| err(StatusCode::NOT_FOUND, "Template not found"))?;
    if !is_owner_or_root(tmpl.0, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    db::delete_template(&engine.pool, id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

// F11: Instantiate template with variable resolution
#[utoipa::path(post, path = "/api/templates/{id}/instantiate", responses((status = 201)), security(("bearer" = [])))]
pub async fn instantiate_template(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<(StatusCode, Json<db::Task>), ApiError> {
    let tmpl = db::get_template(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Template not found"))?;
    if !is_owner_or_root(tmpl.user_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    let data: serde_json::Value = serde_json::from_str(&tmpl.data).map_err(internal)?;
    let today = chrono::Utc::now().naive_utc().format("%Y-%m-%d").to_string();
    // Resolve variables in title and description
    let resolve = |s: &str| s.replace("{{today}}", &today).replace("{{username}}", &claims.username);
    let title = resolve(data["title"].as_str().unwrap_or(&tmpl.name));
    let desc = data["description"].as_str().map(|s| resolve(s));
    let project = data["project"].as_str().map(|s| s.to_string());
    let priority = data["priority"].as_i64().unwrap_or(3).clamp(1, 5);
    let estimated = data["estimated"].as_i64().unwrap_or(0).max(0);
    let t = db::create_task(&engine.pool, claims.user_id, None, &title, desc.as_deref(), project.as_deref(), None, priority, estimated, 0.0, 0.0, None)
        .await.map_err(internal)?;
    engine.notify(ChangeEvent::Tasks);
    Ok((StatusCode::CREATED, Json(t)))
}
