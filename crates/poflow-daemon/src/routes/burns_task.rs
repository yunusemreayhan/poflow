use super::*;

#[utoipa::path(get, path = "/api/tasks/{id}/time", responses((status = 200, body = Vec<db::BurnEntry>)), security(("bearer" = [])))]
pub async fn list_time_reports(
    State(engine): State<AppState>,
    _claims: Claims,
    Path(id): Path<i64>,
) -> ApiResult<Vec<db::BurnEntry>> {
    db::get_task(&engine.pool, id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    db::list_task_burns(&engine.pool, id)
        .await
        .map(Json)
        .map_err(internal)
}

#[utoipa::path(post, path = "/api/tasks/{id}/time", request_body = AddTimeReportRequest, responses((status = 201, body = db::BurnEntry)), security(("bearer" = [])))]
pub async fn add_time_report(
    State(engine): State<AppState>,
    claims: Claims,
    Path(id): Path<i64>,
    Json(req): Json<AddTimeReportRequest>,
) -> Result<(StatusCode, Json<db::BurnEntry>), ApiError> {
    if req.hours <= 0.0 {
        return Err(err(StatusCode::BAD_REQUEST, "Hours must be positive"));
    }
    if req.hours > 24.0 {
        return Err(err(StatusCode::BAD_REQUEST, "Hours must be at most 24"));
    }
    if req.points.is_some_and(|p| !(0.0..=1000.0).contains(&p)) {
        return Err(err(StatusCode::BAD_REQUEST, "Points must be 0-1000"));
    }
    if req.description.as_ref().is_some_and(|d| d.len() > 5000) {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "Description too long (max 5000)",
        ));
    }
    // Verify task exists and user is owner, assignee, or root
    let task = db::get_task(&engine.pool, id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    if task.deleted_at.is_some() {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "Cannot log time on deleted tasks",
        ));
    }
    if task.user_id != claims.user_id && !auth::is_admin_or_root(&claims) {
        let assignees = db::list_assignees(&engine.pool, id)
            .await
            .unwrap_or_default();
        if !assignees.contains(&claims.username) {
            return Err(err(StatusCode::FORBIDDEN, "Not task owner or assignee"));
        }
    }
    let sprint_id = db::find_task_active_sprint(&engine.pool, id)
        .await
        .unwrap_or(None);
    let b = db::log_burn(
        &engine.pool,
        db::LogBurnOpts {
            sprint_id,
            task_id: id,
            session_id: None,
            user_id: claims.user_id,
            points: req.points.unwrap_or(0.0),
            hours: req.hours,
            source: "time_report",
            note: req.description.as_deref(),
        },
    )
    .await
    .map_err(internal)?;
    engine.notify(ChangeEvent::Tasks);
    Ok((StatusCode::CREATED, Json(b)))
}

#[utoipa::path(get, path = "/api/tasks/{id}/burn-total", responses((status = 200, body = db::BurnTotal)), security(("bearer" = [])))]
pub async fn get_task_burn_total(
    State(engine): State<AppState>,
    _claims: Claims,
    Path(id): Path<i64>,
) -> ApiResult<db::BurnTotal> {
    db::get_task(&engine.pool, id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    db::get_task_burn_total(&engine.pool, id)
        .await
        .map(Json)
        .map_err(internal)
}

#[utoipa::path(get, path = "/api/tasks/{id}/burn-users", responses((status = 200, body = Vec<String>)), security(("bearer" = [])))]
pub async fn get_task_burn_users(
    State(engine): State<AppState>,
    _claims: Claims,
    Path(id): Path<i64>,
) -> ApiResult<Vec<String>> {
    db::get_task(&engine.pool, id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    db::get_task_burn_users(&engine.pool, id)
        .await
        .map(Json)
        .map_err(internal)
}

// --- Assignees ---
