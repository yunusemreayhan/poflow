use super::*;


#[utoipa::path(get, path = "/api/tasks/{id}/assignees", responses((status = 200, body = Vec<String>)), security(("bearer" = [])))]
pub async fn list_assignees(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<String>> {
    db::list_assignees(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/tasks/{id}/assignees", request_body = AssignRequest, responses((status = 200)), security(("bearer" = [])))]
pub async fn add_assignee(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<AssignRequest>) -> Result<StatusCode, ApiError> {
    // S1: Verify task exists and user owns it (or is root)
    let task = db::get_task(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    if task.user_id != claims.user_id && claims.role != "root" { return Err(err(StatusCode::FORBIDDEN, "Not your task")); }
    let uid = db::get_user_id_by_username(&engine.pool, &req.username).await.map_err(|e| if e.to_string() == "not_found" { err(StatusCode::NOT_FOUND, "User not found") } else { internal(e) })?;
    db::add_assignee(&engine.pool, id, uid).await.map_err(internal)?;
    // BL21: Notify assigned user
    db::create_notification(&engine.pool, uid, "task_assigned", &format!("You were assigned to: {}", task.title), Some("task"), Some(id)).await.ok();
    engine.notify(ChangeEvent::Tasks);
    Ok(StatusCode::OK)
}

#[utoipa::path(delete, path = "/api/tasks/{id}/assignees/{username}", responses((status = 204)), security(("bearer" = [])))]
pub async fn remove_assignee(State(engine): State<AppState>, claims: Claims, Path((id, username)): Path<(i64, String)>) -> Result<StatusCode, ApiError> {
    let task = db::get_task(&engine.pool, id).await.map_err(internal)?;
    let uid = db::get_user_id_by_username(&engine.pool, &username).await.map_err(|e| if e.to_string() == "not_found" { err(StatusCode::NOT_FOUND, "User not found") } else { internal(e) })?;
    // Allow task owner, root, or the assigned user themselves to unassign
    if !is_owner_or_root(task.user_id, &claims) && uid != claims.user_id { return Err(err(StatusCode::FORBIDDEN, "Not owner or assignee")); }
    db::remove_assignee(&engine.pool, id, uid).await.map_err(internal)?;
    engine.notify(ChangeEvent::Tasks);
    Ok(StatusCode::NO_CONTENT)
}

// --- History & Stats ---
