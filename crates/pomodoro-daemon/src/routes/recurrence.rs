use super::*;



#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetRecurrenceRequest { pub pattern: String, pub next_due: String }

const VALID_PATTERNS: &[&str] = &["daily", "weekly", "biweekly", "monthly"];

#[utoipa::path(put, path = "/api/tasks/{id}/recurrence", responses((status = 200)), security(("bearer" = [])))]
pub async fn set_recurrence(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<SetRecurrenceRequest>) -> ApiResult<db::TaskRecurrence> {
    let task = db::get_task(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    if !is_owner_or_root(task.user_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    if !VALID_PATTERNS.contains(&req.pattern.as_str()) {
        return Err(err(StatusCode::BAD_REQUEST, "Pattern must be: daily, weekly, biweekly, monthly"));
    }
    if chrono::NaiveDate::parse_from_str(&req.next_due, "%Y-%m-%d").is_err() {
        return Err(err(StatusCode::BAD_REQUEST, "next_due must be YYYY-MM-DD"));
    }
    db::set_recurrence(&engine.pool, id, &req.pattern, &req.next_due).await.map(Json).map_err(internal)
}

#[utoipa::path(get, path = "/api/tasks/{id}/recurrence", responses((status = 200)), security(("bearer" = [])))]
pub async fn get_recurrence(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Option<db::TaskRecurrence>> {
    db::get_recurrence(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(delete, path = "/api/tasks/{id}/recurrence", responses((status = 204)), security(("bearer" = [])))]
pub async fn remove_recurrence(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    let task = db::get_task(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    if !is_owner_or_root(task.user_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    db::remove_recurrence(&engine.pool, id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}
