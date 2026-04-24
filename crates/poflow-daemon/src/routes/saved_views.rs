use super::*;

#[derive(Deserialize, utoipa::ToSchema)]
pub struct SavedViewRequest {
    pub name: String,
    pub filters: serde_json::Value,
}

#[utoipa::path(get, path = "/api/views", responses((status = 200, body = Vec<db::SavedView>)), security(("bearer" = [])))]
pub async fn list_saved_views(
    State(engine): State<AppState>,
    claims: Claims,
) -> ApiResult<Vec<db::SavedView>> {
    db::list_saved_views(&engine.pool, claims.user_id)
        .await
        .map(Json)
        .map_err(internal)
}

#[utoipa::path(post, path = "/api/views", request_body = SavedViewRequest, responses((status = 201, body = db::SavedView)), security(("bearer" = [])))]
pub async fn create_saved_view(
    State(engine): State<AppState>,
    claims: Claims,
    Json(req): Json<SavedViewRequest>,
) -> Result<(StatusCode, Json<db::SavedView>), ApiError> {
    if req.name.trim().is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "Name cannot be empty"));
    }
    if req.name.len() > 200 {
        return Err(err(StatusCode::BAD_REQUEST, "Name too long (max 200)"));
    }
    let filters_str = serde_json::to_string(&req.filters).map_err(internal)?;
    let view = db::create_saved_view(&engine.pool, claims.user_id, req.name.trim(), &filters_str)
        .await
        .map_err(internal)?;
    Ok((StatusCode::CREATED, Json(view)))
}

#[utoipa::path(put, path = "/api/views/{id}", request_body = SavedViewRequest, responses((status = 200, body = db::SavedView)), security(("bearer" = [])))]
pub async fn update_saved_view(
    State(engine): State<AppState>,
    claims: Claims,
    Path(id): Path<i64>,
    Json(req): Json<SavedViewRequest>,
) -> ApiResult<db::SavedView> {
    if req.name.trim().is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "Name cannot be empty"));
    }
    if req.name.len() > 200 {
        return Err(err(StatusCode::BAD_REQUEST, "Name too long (max 200)"));
    }
    let filters_str = serde_json::to_string(&req.filters).map_err(internal)?;
    db::update_saved_view(
        &engine.pool,
        id,
        claims.user_id,
        req.name.trim(),
        &filters_str,
    )
    .await
    .map(Json)
    .map_err(|_| err(StatusCode::NOT_FOUND, "View not found"))
}

#[utoipa::path(delete, path = "/api/views/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_saved_view(
    State(engine): State<AppState>,
    claims: Claims,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    db::delete_saved_view(&engine.pool, id, claims.user_id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| err(StatusCode::NOT_FOUND, "View not found"))
}
