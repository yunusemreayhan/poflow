use super::*;

#[utoipa::path(post, path = "/api/tasks/{id}/watch", responses((status = 204)), security(("bearer" = [])))]
pub async fn watch_task(
    State(engine): State<AppState>,
    claims: Claims,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    db::get_task(&engine.pool, id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    db::watch_task(&engine.pool, id, claims.user_id)
        .await
        .map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/api/tasks/{id}/watch", responses((status = 204)), security(("bearer" = [])))]
pub async fn unwatch_task(
    State(engine): State<AppState>,
    claims: Claims,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    db::get_task(&engine.pool, id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    db::unwatch_task(&engine.pool, id, claims.user_id)
        .await
        .map_err(|_| err(StatusCode::BAD_REQUEST, "Not watching this task"))?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/tasks/{id}/watchers", responses((status = 200)), security(("bearer" = [])))]
pub async fn get_task_watchers(
    State(engine): State<AppState>,
    _claims: Claims,
    Path(id): Path<i64>,
) -> ApiResult<Vec<String>> {
    db::get_task(&engine.pool, id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    db::get_task_watchers(&engine.pool, id)
        .await
        .map(Json)
        .map_err(internal)
}

#[utoipa::path(get, path = "/api/watched", responses((status = 200)), security(("bearer" = [])))]
pub async fn get_watched_tasks(
    State(engine): State<AppState>,
    claims: Claims,
) -> ApiResult<Vec<i64>> {
    db::get_watched_tasks(&engine.pool, claims.user_id)
        .await
        .map(Json)
        .map_err(internal)
}
