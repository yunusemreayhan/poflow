use super::*;


#[utoipa::path(get, path = "/api/tasks/{id}/comments", responses((status = 200, body = Vec<db::Comment>)), security(("bearer" = [])))]
pub async fn list_comments(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<db::Comment>> {
    db::list_comments(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/tasks/{id}/comments", request_body = AddCommentRequest, responses((status = 201, body = db::Comment)), security(("bearer" = [])))]
pub async fn add_comment(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<AddCommentRequest>) -> Result<(StatusCode, Json<db::Comment>), ApiError> {
    if req.content.trim().is_empty() { return Err(err(StatusCode::BAD_REQUEST, "Comment cannot be empty")); }
    if req.content.len() > 10000 { return Err(err(StatusCode::BAD_REQUEST, "Comment too long (max 10000 chars)")); }
    db::add_comment(&engine.pool, claims.user_id, id, req.session_id, &req.content)
        .await.map(|c| { engine.notify(ChangeEvent::Tasks); (StatusCode::CREATED, Json(c)) }).map_err(internal)
}

#[utoipa::path(delete, path = "/api/comments/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_comment(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    let comment = db::get_comment(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Comment not found"))?;
    if !is_owner_or_root(comment.user_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    db::delete_comment(&engine.pool, id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Tasks);
    Ok(StatusCode::NO_CONTENT)
}

// --- Task Time/Burns ---
