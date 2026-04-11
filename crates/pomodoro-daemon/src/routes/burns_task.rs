use super::*;


#[utoipa::path(get, path = "/api/tasks/{id}/time", responses((status = 200, body = Vec<db::BurnEntry>)), security(("bearer" = [])))]
pub async fn list_time_reports(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<db::BurnEntry>> {
    db::list_task_burns(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/tasks/{id}/time", request_body = AddTimeReportRequest, responses((status = 201, body = db::BurnEntry)), security(("bearer" = [])))]
pub async fn add_time_report(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<AddTimeReportRequest>) -> Result<(StatusCode, Json<db::BurnEntry>), ApiError> {
    let sprint_id = db::find_task_active_sprint(&engine.pool, id).await.unwrap_or(None);
    let b = db::log_burn(&engine.pool, sprint_id, id, None, claims.user_id, req.points.unwrap_or(0.0), req.hours, "time_report", req.description.as_deref())
        .await.map_err(internal)?;
    engine.notify(ChangeEvent::Tasks);
    Ok((StatusCode::CREATED, Json(b)))
}

#[utoipa::path(get, path = "/api/tasks/{id}/burn-total", responses((status = 200, body = db::BurnTotal)), security(("bearer" = [])))]
pub async fn get_task_burn_total(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<db::BurnTotal> {
    db::get_task_burn_total(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(get, path = "/api/tasks/{id}/burn-users", responses((status = 200, body = Vec<String>)), security(("bearer" = [])))]
pub async fn get_task_burn_users(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<String>> {
    db::get_task_burn_users(&engine.pool, id).await.map(Json).map_err(internal)
}

// --- Assignees ---
