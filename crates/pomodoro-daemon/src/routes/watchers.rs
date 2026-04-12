use super::*;

pub async fn watch_task(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    db::get_task(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    db::watch_task(&engine.pool, id, claims.user_id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn unwatch_task(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    db::unwatch_task(&engine.pool, id, claims.user_id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_task_watchers(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<String>> {
    db::get_task_watchers(&engine.pool, id).await.map(Json).map_err(internal)
}

pub async fn get_watched_tasks(State(engine): State<AppState>, claims: Claims) -> ApiResult<Vec<i64>> {
    db::get_watched_tasks(&engine.pool, claims.user_id).await.map(Json).map_err(internal)
}

// F11: Task dependencies
pub async fn add_task_dependency(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>, Json(req): Json<serde_json::Value>) -> Result<StatusCode, ApiError> {
    let dep_id = req["depends_on_id"].as_i64().ok_or_else(|| err(StatusCode::BAD_REQUEST, "depends_on_id required"))?;
    let task = db::get_task(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    if !is_owner_or_root(task.user_id, &_claims) { return Err(err(StatusCode::FORBIDDEN, "Not task owner")); }
    db::get_task(&engine.pool, dep_id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Dependency task not found"))?;
    db::add_dependency(&engine.pool, id, dep_id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Tasks);
    Ok(StatusCode::NO_CONTENT)
}

pub async fn remove_task_dependency(State(engine): State<AppState>, _claims: Claims, Path((id, dep_id)): Path<(i64, i64)>) -> Result<StatusCode, ApiError> {
    let task = db::get_task(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    if !is_owner_or_root(task.user_id, &_claims) { return Err(err(StatusCode::FORBIDDEN, "Not task owner")); }
    db::remove_dependency(&engine.pool, id, dep_id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Tasks);
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_task_blocking(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<i64>> {
    db::get_dependencies(&engine.pool, id).await.map(Json).map_err(internal)
}
