use super::*;



#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetRecurrenceRequest { pub pattern: String, pub next_due: String }

const VALID_PATTERNS: &[&str] = &["daily", "weekly", "biweekly", "monthly"];

#[utoipa::path(put, path = "/api/tasks/{id}/recurrence", responses((status = 200)), security(("bearer" = [])))]
pub async fn set_recurrence(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>, Json(req): Json<SetRecurrenceRequest>) -> ApiResult<db::TaskRecurrence> {
    if !VALID_PATTERNS.contains(&req.pattern.as_str()) {
        return Err(err(StatusCode::BAD_REQUEST, "Pattern must be: daily, weekly, biweekly, monthly"));
    }
    db::set_recurrence(&engine.pool, id, &req.pattern, &req.next_due).await.map(Json).map_err(internal)
}

#[utoipa::path(get, path = "/api/tasks/{id}/recurrence", responses((status = 200)), security(("bearer" = [])))]
pub async fn get_recurrence(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Option<db::TaskRecurrence>> {
    db::get_recurrence(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(delete, path = "/api/tasks/{id}/recurrence", responses((status = 204)), security(("bearer" = [])))]
pub async fn remove_recurrence(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    db::remove_recurrence(&engine.pool, id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}
