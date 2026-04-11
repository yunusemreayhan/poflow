use super::*;

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateTemplateRequest { pub name: String, pub data: serde_json::Value }

#[utoipa::path(get, path = "/api/templates", responses((status = 200)), security(("bearer" = [])))]
pub async fn list_templates(State(engine): State<AppState>, claims: Claims) -> ApiResult<Vec<db::TaskTemplate>> {
    db::list_templates(&engine.pool, claims.user_id).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/templates", responses((status = 201)), security(("bearer" = [])))]
pub async fn create_template(State(engine): State<AppState>, claims: Claims, Json(req): Json<CreateTemplateRequest>) -> Result<(StatusCode, Json<db::TaskTemplate>), ApiError> {
    if req.name.trim().is_empty() { return Err(err(StatusCode::BAD_REQUEST, "Name required")); }
    let data = serde_json::to_string(&req.data).map_err(internal)?;
    let t = db::create_template(&engine.pool, claims.user_id, req.name.trim(), &data).await.map_err(internal)?;
    Ok((StatusCode::CREATED, Json(t)))
}

#[utoipa::path(delete, path = "/api/templates/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_template(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    db::delete_template(&engine.pool, id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}
