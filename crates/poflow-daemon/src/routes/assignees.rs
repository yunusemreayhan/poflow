use super::*;

#[utoipa::path(get, path = "/api/tasks/{id}/assignees", responses((status = 200, body = Vec<String>)), security(("bearer" = [])))]
pub async fn list_assignees(
    State(engine): State<AppState>,
    _claims: Claims,
    Path(id): Path<i64>,
) -> ApiResult<Vec<String>> {
    db::get_task(&engine.pool, id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    db::list_assignees(&engine.pool, id)
        .await
        .map(Json)
        .map_err(internal)
}

#[utoipa::path(post, path = "/api/tasks/{id}/assignees", request_body = AssignRequest, responses((status = 200)), security(("bearer" = [])))]
pub async fn add_assignee(
    State(engine): State<AppState>,
    claims: Claims,
    Path(id): Path<i64>,
    Json(req): Json<AssignRequest>,
) -> Result<StatusCode, ApiError> {
    // S1: Verify task exists and user owns it (or is root)
    let task = db::get_task(&engine.pool, id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    if task.user_id != claims.user_id && !auth::is_admin_or_root(&claims) {
        return Err(err(StatusCode::FORBIDDEN, "Not your task"));
    }
    let uid = db::get_user_id_by_username(&engine.pool, &req.username)
        .await
        .map_err(internal)?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "User not found"))?;
    db::add_assignee(&engine.pool, id, uid)
        .await
        .map_err(internal)?;
    // BL21: Notify assigned user
    db::create_notification(
        &engine.pool,
        uid,
        "task_assigned",
        &format!("You were assigned to: {}", task.title),
        Some("task"),
        Some(id),
    )
    .await
    .ok();
    // F19: Run task.assigned automations
    let pool = engine.pool.clone();
    let uname = req.username.clone();
    tokio::spawn(async move {
        crate::automation::run_task_assigned(&pool, id, &uname).await;
    });
    // Email notification (if user has email configured)
    #[cfg(feature = "email")]
    if let Ok(user) = db::get_user(&engine.pool, uid).await {
        if let Some(ref email) = user.email {
            crate::email::notify_assigned(email, &task.title, &claims.username);
        }
    }
    engine.notify(ChangeEvent::Tasks);
    Ok(StatusCode::OK)
}

#[utoipa::path(delete, path = "/api/tasks/{id}/assignees/{username}", responses((status = 204)), security(("bearer" = [])))]
pub async fn remove_assignee(
    State(engine): State<AppState>,
    claims: Claims,
    Path((id, username)): Path<(i64, String)>,
) -> Result<StatusCode, ApiError> {
    let task = db::get_task(&engine.pool, id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    let uid = db::get_user_id_by_username(&engine.pool, &username)
        .await
        .map_err(internal)?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "User not found"))?;
    // Allow task owner, root, or the assigned user themselves to unassign
    if !is_owner_or_root(task.user_id, &claims) && uid != claims.user_id {
        return Err(err(StatusCode::FORBIDDEN, "Not owner or assignee"));
    }
    db::remove_assignee(&engine.pool, id, uid)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "User not assigned to this task"))?;
    engine.notify(ChangeEvent::Tasks);
    Ok(StatusCode::NO_CONTENT)
}

// --- History & Stats ---
