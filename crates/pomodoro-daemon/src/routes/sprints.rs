use super::*;


#[utoipa::path(get, path = "/api/sprints", responses((status = 200, body = Vec<db::Sprint>)), security(("bearer" = [])))]
pub async fn list_sprints(State(engine): State<AppState>, _claims: Claims, Query(q): Query<SprintQuery>) -> ApiResult<Vec<db::Sprint>> {
    db::list_sprints(&engine.pool, q.status.as_deref(), q.project.as_deref()).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/sprints", request_body = CreateSprintRequest, responses((status = 201, body = db::Sprint)), security(("bearer" = [])))]
pub async fn create_sprint(State(engine): State<AppState>, claims: Claims, Json(req): Json<CreateSprintRequest>) -> Result<(StatusCode, Json<db::Sprint>), ApiError> {
    if req.name.trim().is_empty() { return Err(err(StatusCode::BAD_REQUEST, "Sprint name cannot be empty")); }
    if req.name.len() > 200 { return Err(err(StatusCode::BAD_REQUEST, "Sprint name too long (max 200 chars)")); }
    if let Some(ref d) = req.start_date { if d.len() != 10 { return Err(err(StatusCode::BAD_REQUEST, "start_date must be YYYY-MM-DD")); } }
    if let Some(ref d) = req.end_date { if d.len() != 10 { return Err(err(StatusCode::BAD_REQUEST, "end_date must be YYYY-MM-DD")); } }
    let s = db::create_sprint(&engine.pool, claims.user_id, &req.name, req.project.as_deref(), req.goal.as_deref(), req.start_date.as_deref(), req.end_date.as_deref())
        .await.map_err(internal)?;
    engine.notify(ChangeEvent::Sprints);
    Ok((StatusCode::CREATED, Json(s)))
}

#[utoipa::path(get, path = "/api/sprints/{id}", responses((status = 200, body = db::SprintDetail)), security(("bearer" = [])))]
pub async fn get_sprint_detail(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<db::SprintDetail> {
    db::get_sprint_detail(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(put, path = "/api/sprints/{id}", request_body = UpdateSprintRequest, responses((status = 200, body = db::Sprint)), security(("bearer" = [])))]
pub async fn update_sprint(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<UpdateSprintRequest>) -> ApiResult<db::Sprint> {
    let sprint = db::get_sprint(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Sprint not found"))?;
    if !is_owner_or_root(sprint.created_by_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    if req.status.is_some() { return Err(err(StatusCode::BAD_REQUEST, "Use /start or /complete endpoints to change sprint status")); }
    if let Some(ref expected) = req.expected_updated_at {
        if *expected != sprint.updated_at {
            return Err(err(StatusCode::CONFLICT, "Sprint was modified by another user. Please refresh and try again."));
        }
    }
    let s = db::update_sprint(&engine.pool, id, req.name.as_deref(),
        req.project.as_ref().map(|o| o.as_deref()),
        req.goal.as_ref().map(|o| o.as_deref()),
        req.status.as_deref(),
        req.start_date.as_ref().map(|o| o.as_deref()),
        req.end_date.as_ref().map(|o| o.as_deref()),
        req.retro_notes.as_ref().map(|o| o.as_deref()))
        .await.map_err(internal)?;
    engine.notify(ChangeEvent::Sprints);
    Ok(Json(s))
}

#[utoipa::path(delete, path = "/api/sprints/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_sprint(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    let sprint = db::get_sprint(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Sprint not found"))?;
    if !is_owner_or_root(sprint.created_by_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    db::delete_sprint(&engine.pool, id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Sprints);
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/sprints/{id}/start", responses((status = 200, body = db::Sprint)), security(("bearer" = [])))]
pub async fn start_sprint(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> ApiResult<db::Sprint> {
    let sprint = db::get_sprint(&engine.pool, id).await.map_err(internal)?;
    if !is_owner_or_root(sprint.created_by_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    if sprint.status != "planning" { return Err(err(StatusCode::BAD_REQUEST, format!("Cannot start sprint in '{}' status", sprint.status))); }
    let s = db::update_sprint(&engine.pool, id, None, None, None, Some("active"), None, None, None).await.map_err(internal)?;
    if let Err(e) = db::snapshot_sprint(&engine.pool, id).await { tracing::warn!("Snapshot failed: {}", e); }
    engine.notify(ChangeEvent::Sprints);
    Ok(Json(s))
}

#[utoipa::path(post, path = "/api/sprints/{id}/complete", responses((status = 200, body = db::Sprint)), security(("bearer" = [])))]
pub async fn complete_sprint(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> ApiResult<db::Sprint> {
    let sprint = db::get_sprint(&engine.pool, id).await.map_err(internal)?;
    if !is_owner_or_root(sprint.created_by_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    if sprint.status != "active" { return Err(err(StatusCode::BAD_REQUEST, format!("Cannot complete sprint in '{}' status", sprint.status))); }
    if let Err(e) = db::snapshot_sprint(&engine.pool, id).await { tracing::warn!("Snapshot failed: {}", e); }
    let s = db::update_sprint(&engine.pool, id, None, None, None, Some("completed"), None, None, None).await.map_err(internal)?;
    engine.notify(ChangeEvent::Sprints);
    Ok(Json(s))
}

#[utoipa::path(get, path = "/api/sprints/{id}/tasks", responses((status = 200, body = Vec<db::Task>)), security(("bearer" = [])))]
pub async fn get_sprint_tasks(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<db::Task>> {
    db::get_sprint_tasks(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/sprints/{id}/tasks", request_body = AddSprintTasksRequest, responses((status = 200, body = Vec<db::SprintTask>)), security(("bearer" = [])))]
pub async fn add_sprint_tasks(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<AddSprintTasksRequest>) -> ApiResult<Vec<db::SprintTask>> {
    let sprint = db::get_sprint(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Sprint not found"))?;
    if !is_owner_or_root(sprint.created_by_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not sprint owner")); }
    if req.task_ids.len() > 500 { return Err(err(StatusCode::BAD_REQUEST, "Too many task IDs (max 500)")); }
    if req.task_ids.is_empty() { return Err(err(StatusCode::BAD_REQUEST, "task_ids cannot be empty")); }
    // Validate all tasks exist
    for &tid in &req.task_ids {
        db::get_task(&engine.pool, tid).await.map_err(|_| err(StatusCode::NOT_FOUND, &format!("Task {} not found", tid)))?;
    }
    let result = db::add_sprint_tasks(&engine.pool, id, &req.task_ids, claims.user_id).await.map_err(internal)?;
    if db::get_sprint(&engine.pool, id).await.map(|s| s.status == "active").unwrap_or(false) {
        if let Err(e) = db::snapshot_sprint(&engine.pool, id).await { tracing::warn!("Snapshot failed: {}", e); }
    }
    engine.notify(ChangeEvent::Sprints);
    Ok(Json(result))
}

#[utoipa::path(delete, path = "/api/sprints/{id}/tasks/{task_id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn remove_sprint_task(State(engine): State<AppState>, claims: Claims, Path((id, task_id)): Path<(i64, i64)>) -> Result<StatusCode, ApiError> {
    let sprint = db::get_sprint(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Sprint not found"))?;
    if !is_owner_or_root(sprint.created_by_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not sprint owner")); }
    db::remove_sprint_task(&engine.pool, id, task_id).await.map_err(internal)?;
    if db::get_sprint(&engine.pool, id).await.map(|s| s.status == "active").unwrap_or(false) {
        if let Err(e) = db::snapshot_sprint(&engine.pool, id).await { tracing::warn!("Snapshot failed: {}", e); }
    }
    engine.notify(ChangeEvent::Sprints);
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/sprints/{id}/burndown", responses((status = 200, body = Vec<db::SprintDailyStat>)), security(("bearer" = [])))]
pub async fn get_sprint_burndown(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<db::SprintDailyStat>> {
    db::get_sprint_burndown(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(get, path = "/api/sprints/burndown", responses((status = 200, body = Vec<db::SprintDailyStat>)), security(("bearer" = [])))]
pub async fn get_global_burndown(State(engine): State<AppState>, _claims: Claims) -> ApiResult<Vec<db::SprintDailyStat>> {
    db::get_global_burndown(&engine.pool).await.map(Json).map_err(internal)
}

// --- Epic Groups ---

#[derive(Deserialize)]
pub struct VelocityQuery { pub sprints: Option<i64> }

#[utoipa::path(get, path = "/api/sprints/velocity", responses((status = 200)), security(("bearer" = [])))]
pub async fn get_velocity(State(engine): State<AppState>, _claims: Claims, Query(q): Query<VelocityQuery>) -> ApiResult<Vec<serde_json::Value>> {
    let n = q.sprints.unwrap_or(10).min(50);
    let rows = db::get_velocity(&engine.pool, n).await.map_err(internal)?;
    let result: Vec<serde_json::Value> = rows.into_iter().map(|(name, points, hours, tasks)| {
        serde_json::json!({ "sprint": name, "points": points, "hours": hours, "tasks_done": tasks })
    }).collect();
    Ok(Json(result))
}
