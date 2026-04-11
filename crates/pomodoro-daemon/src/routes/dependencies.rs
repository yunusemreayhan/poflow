use super::*;



#[derive(Deserialize, utoipa::ToSchema)]
pub struct AddDependencyRequest { pub depends_on: i64 }

#[utoipa::path(get, path = "/api/tasks/{id}/dependencies", responses((status = 200)), security(("bearer" = [])))]
pub async fn get_dependencies(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<i64>> {
    db::get_dependencies(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/tasks/{id}/dependencies", responses((status = 204)), security(("bearer" = [])))]
pub async fn add_dependency(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>, Json(req): Json<AddDependencyRequest>) -> Result<StatusCode, ApiError> {
    db::add_dependency(&engine.pool, id, req.depends_on).await
        .map_err(|e| err(StatusCode::BAD_REQUEST, e.to_string()))?;
    engine.notify(ChangeEvent::Tasks);
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/api/tasks/{id}/dependencies/{dep_id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn remove_dependency(State(engine): State<AppState>, _claims: Claims, Path((id, dep_id)): Path<(i64, i64)>) -> Result<StatusCode, ApiError> {
    db::remove_dependency(&engine.pool, id, dep_id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Tasks);
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/dependencies", responses((status = 200)), security(("bearer" = [])))]
pub async fn get_all_dependencies(State(engine): State<AppState>, _claims: Claims) -> ApiResult<Vec<db::TaskDependency>> {
    db::get_all_dependencies(&engine.pool).await.map(Json).map_err(internal)
}
