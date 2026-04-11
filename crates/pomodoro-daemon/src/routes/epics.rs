use super::*;

#[utoipa::path(get, path = "/api/epics", responses((status = 200, body = Vec<db::EpicGroup>)), security(("bearer" = [])))]
pub async fn list_epic_groups(State(engine): State<AppState>, _claims: Claims) -> ApiResult<Vec<db::EpicGroup>> {
    db::list_epic_groups(&engine.pool).await.map(Json).map_err(internal)
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateEpicGroupRequest { pub name: String }

#[utoipa::path(post, path = "/api/epics", responses((status = 201, body = db::EpicGroup)), security(("bearer" = [])))]
pub async fn create_epic_group(State(engine): State<AppState>, claims: Claims, Json(req): Json<CreateEpicGroupRequest>) -> Result<(StatusCode, Json<db::EpicGroup>), ApiError> {
    let g = db::create_epic_group(&engine.pool, &req.name, claims.user_id).await.map_err(internal)?;
    Ok((StatusCode::CREATED, Json(g)))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct EpicGroupTasksRequest { pub task_ids: Vec<i64> }

#[utoipa::path(get, path = "/api/epics/{id}", responses((status = 200, body = db::EpicGroupDetail)), security(("bearer" = [])))]
pub async fn get_epic_group(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<db::EpicGroupDetail> {
    db::get_epic_group_detail(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(delete, path = "/api/epics/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_epic_group(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    db::delete_epic_group(&engine.pool, id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/epics/{id}/tasks", responses((status = 204)), security(("bearer" = [])))]
pub async fn add_epic_group_tasks(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>, Json(req): Json<EpicGroupTasksRequest>) -> Result<StatusCode, ApiError> {
    for tid in req.task_ids { db::add_epic_group_task(&engine.pool, id, tid).await.map_err(internal)?; }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/api/epics/{id}/tasks/{task_id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn remove_epic_group_task(State(engine): State<AppState>, _claims: Claims, Path((id, task_id)): Path<(i64, i64)>) -> Result<StatusCode, ApiError> {
    db::remove_epic_group_task(&engine.pool, id, task_id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/epics/{id}/snapshot", responses((status = 204)), security(("bearer" = [])))]
pub async fn snapshot_epic_group(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    db::snapshot_epic_group(&engine.pool, id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

// --- Sprint Root Tasks ---

#[utoipa::path(get, path = "/api/sprints/{id}/roots", responses((status = 200, body = Vec<i64>)), security(("bearer" = [])))]
pub async fn get_sprint_root_tasks(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<i64>> {
    db::get_sprint_root_tasks(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/sprints/{id}/roots", responses((status = 204)), security(("bearer" = [])))]
pub async fn add_sprint_root_tasks(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>, Json(req): Json<EpicGroupTasksRequest>) -> Result<StatusCode, ApiError> {
    for tid in req.task_ids { db::add_sprint_root_task(&engine.pool, id, tid).await.map_err(internal)?; }
    engine.notify(ChangeEvent::Sprints);
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/api/sprints/{id}/roots/{task_id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn remove_sprint_root_task(State(engine): State<AppState>, _claims: Claims, Path((id, task_id)): Path<(i64, i64)>) -> Result<StatusCode, ApiError> {
    db::remove_sprint_root_task(&engine.pool, id, task_id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Sprints);
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/sprints/{id}/scope", responses((status = 200, body = Vec<i64>)), security(("bearer" = [])))]
pub async fn get_sprint_scope(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<i64>> {
    let roots = db::get_sprint_root_tasks(&engine.pool, id).await.map_err(internal)?;
    if roots.is_empty() { return Ok(Json(vec![])); }
    db::get_descendant_ids(&engine.pool, &roots).await.map(Json).map_err(internal)
}
