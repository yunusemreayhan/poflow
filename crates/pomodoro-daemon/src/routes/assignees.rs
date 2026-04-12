use super::*;


#[utoipa::path(get, path = "/api/tasks/{id}/assignees", responses((status = 200, body = Vec<String>)), security(("bearer" = [])))]
pub async fn list_assignees(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<String>> {
    db::list_assignees(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/tasks/{id}/assignees", request_body = AssignRequest, responses((status = 200)), security(("bearer" = [])))]
pub async fn add_assignee(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>, Json(req): Json<AssignRequest>) -> Result<StatusCode, ApiError> {
    let uid = db::get_user_id_by_username(&engine.pool, &req.username).await.map_err(|_| err(StatusCode::NOT_FOUND, "User not found"))?;
    db::add_assignee(&engine.pool, id, uid).await.map_err(internal)?;
    // BL21: Notify assigned user
    let task = db::get_task(&engine.pool, id).await.ok();
    let title = task.as_ref().map(|t| t.title.as_str()).unwrap_or("a task");
    db::create_notification(&engine.pool, uid, "task_assigned", &format!("You were assigned to: {}", title), Some("task"), Some(id)).await.ok();
    engine.notify(ChangeEvent::Tasks);
    Ok(StatusCode::OK)
}

#[utoipa::path(delete, path = "/api/tasks/{id}/assignees/{username}", responses((status = 204)), security(("bearer" = [])))]
pub async fn remove_assignee(State(engine): State<AppState>, claims: Claims, Path((id, username)): Path<(i64, String)>) -> Result<StatusCode, ApiError> {
    let task = db::get_task(&engine.pool, id).await.map_err(internal)?;
    if !is_owner_or_root(task.user_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    let uid = db::get_user_id_by_username(&engine.pool, &username).await.map_err(|_| err(StatusCode::NOT_FOUND, "User not found"))?;
    db::remove_assignee(&engine.pool, id, uid).await.map_err(internal)?;
    engine.notify(ChangeEvent::Tasks);
    Ok(StatusCode::NO_CONTENT)
}

// --- History & Stats ---
