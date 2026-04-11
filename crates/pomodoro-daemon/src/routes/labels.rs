use super::*;



#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateLabelRequest { pub name: String, pub color: Option<String> }

#[utoipa::path(get, path = "/api/labels", responses((status = 200)), security(("bearer" = [])))]
pub async fn list_labels(State(engine): State<AppState>, _claims: Claims) -> ApiResult<Vec<db::Label>> {
    db::list_labels(&engine.pool).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/labels", responses((status = 201)), security(("bearer" = [])))]
pub async fn create_label(State(engine): State<AppState>, _claims: Claims, Json(req): Json<CreateLabelRequest>) -> Result<(StatusCode, Json<db::Label>), ApiError> {
    if req.name.trim().is_empty() { return Err(err(StatusCode::BAD_REQUEST, "Label name cannot be empty")); }
    let color = req.color.as_deref().unwrap_or("#6366f1");
    let label = db::create_label(&engine.pool, req.name.trim(), color).await
        .map_err(|e| if e.to_string().contains("UNIQUE") { err(StatusCode::CONFLICT, "Label already exists") } else { internal(e) })?;
    Ok((StatusCode::CREATED, Json(label)))
}

#[utoipa::path(delete, path = "/api/labels/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_label(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    db::delete_label(&engine.pool, id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(put, path = "/api/tasks/{id}/labels/{label_id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn add_task_label(State(engine): State<AppState>, _claims: Claims, Path((task_id, label_id)): Path<(i64, i64)>) -> Result<StatusCode, ApiError> {
    db::add_task_label(&engine.pool, task_id, label_id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Tasks);
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/api/tasks/{id}/labels/{label_id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn remove_task_label(State(engine): State<AppState>, _claims: Claims, Path((task_id, label_id)): Path<(i64, i64)>) -> Result<StatusCode, ApiError> {
    db::remove_task_label(&engine.pool, task_id, label_id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Tasks);
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/tasks/{id}/labels", responses((status = 200)), security(("bearer" = [])))]
pub async fn get_task_labels(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<db::Label>> {
    db::get_task_labels(&engine.pool, id).await.map(Json).map_err(internal)
}
