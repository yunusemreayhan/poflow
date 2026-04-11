use super::*;


#[utoipa::path(post, path = "/api/sprints/{id}/burn", request_body = LogBurnRequest, responses((status = 201, body = db::BurnEntry)), security(("bearer" = [])))]
pub async fn log_burn(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<LogBurnRequest>) -> Result<(StatusCode, Json<db::BurnEntry>), ApiError> {
    let pts = req.points.unwrap_or(0.0);
    let hrs = req.hours.unwrap_or(0.0);
    if pts < 0.0 || hrs < 0.0 { return Err(err(StatusCode::BAD_REQUEST, "Points and hours must be non-negative")); }
    if pts > 1000.0 || hrs > 24.0 { return Err(err(StatusCode::BAD_REQUEST, "Points max 1000, hours max 24")); }
    // Verify task belongs to this sprint
    let sprint_tasks = db::get_sprint_task_entries(&engine.pool, id).await.map_err(internal)?;
    if !sprint_tasks.iter().any(|st| st.task_id == req.task_id) {
        return Err(err(StatusCode::BAD_REQUEST, "Task does not belong to this sprint"));
    }
    let b = db::log_burn(&engine.pool, Some(id), req.task_id, None, claims.user_id, pts, hrs, "manual", req.note.as_deref())
        .await.map_err(internal)?;
    engine.notify(ChangeEvent::Sprints);
    Ok((StatusCode::CREATED, Json(b)))
}

#[utoipa::path(get, path = "/api/sprints/{id}/burns", responses((status = 200, body = Vec<db::BurnEntry>)), security(("bearer" = [])))]
pub async fn list_burns(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<db::BurnEntry>> {
    db::list_burns(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(delete, path = "/api/sprints/{id}/burns/{burn_id}", responses((status = 200, body = db::BurnEntry)), security(("bearer" = [])))]
pub async fn cancel_burn(State(engine): State<AppState>, claims: Claims, Path((sprint_id, burn_id)): Path<(i64, i64)>) -> ApiResult<db::BurnEntry> {
    let burn = db::get_burn(&engine.pool, burn_id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Burn not found"))?;
    if burn.sprint_id != Some(sprint_id) { return Err(err(StatusCode::BAD_REQUEST, "Burn does not belong to this sprint")); }
    if burn.cancelled != 0 { return Err(err(StatusCode::BAD_REQUEST, "Burn already cancelled")); }
    if burn.user_id != claims.user_id && claims.role != "root" {
        return Err(err(StatusCode::FORBIDDEN, "Not owner"));
    }
    let b = db::cancel_burn(&engine.pool, burn_id, claims.user_id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Sprints);
    Ok(Json(b))
}

#[utoipa::path(get, path = "/api/sprints/{id}/burn-summary", responses((status = 200, body = Vec<db::BurnSummaryEntry>)), security(("bearer" = [])))]
pub async fn get_burn_summary(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<db::BurnSummaryEntry>> {
    db::get_burn_summary(&engine.pool, id).await.map(Json).map_err(internal)
}
