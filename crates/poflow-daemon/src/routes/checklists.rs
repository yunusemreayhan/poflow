use super::*;

#[derive(Deserialize, utoipa::ToSchema)]
pub struct AddChecklistItemRequest {
    pub title: String,
    pub sort_order: Option<i64>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateChecklistItemRequest {
    pub title: Option<String>,
    pub checked: Option<bool>,
    pub sort_order: Option<i64>,
}

#[utoipa::path(get, path = "/api/tasks/{id}/checklist", responses((status = 200, body = Vec<db::ChecklistItem>)), security(("bearer" = [])))]
pub async fn list_checklist(
    State(engine): State<AppState>,
    _claims: Claims,
    Path(id): Path<i64>,
) -> ApiResult<Vec<db::ChecklistItem>> {
    db::get_task(&engine.pool, id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    db::list_checklist(&engine.pool, id)
        .await
        .map(Json)
        .map_err(internal)
}

#[utoipa::path(post, path = "/api/tasks/{id}/checklist", request_body = AddChecklistItemRequest, responses((status = 201, body = db::ChecklistItem)), security(("bearer" = [])))]
pub async fn add_checklist_item(
    State(engine): State<AppState>,
    claims: Claims,
    Path(id): Path<i64>,
    Json(req): Json<AddChecklistItemRequest>,
) -> Result<(StatusCode, Json<db::ChecklistItem>), ApiError> {
    let task = db::get_task(&engine.pool, id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    if !is_owner_or_root(task.user_id, &claims) {
        let assignees = db::list_assignees(&engine.pool, id)
            .await
            .map_err(internal)?;
        if !assignees.contains(&claims.username) {
            return Err(err(StatusCode::FORBIDDEN, "Not owner or assignee"));
        }
    }
    if req.title.trim().is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "Title cannot be empty"));
    }
    if req.title.len() > 500 {
        return Err(err(StatusCode::BAD_REQUEST, "Title too long (max 500)"));
    }
    let item = db::add_checklist_item(
        &engine.pool,
        id,
        req.title.trim(),
        req.sort_order.unwrap_or(0),
    )
    .await
    .map_err(internal)?;
    Ok((StatusCode::CREATED, Json(item)))
}

#[utoipa::path(put, path = "/api/checklist/{id}", request_body = UpdateChecklistItemRequest, responses((status = 200, body = db::ChecklistItem)), security(("bearer" = [])))]
pub async fn update_checklist_item(
    State(engine): State<AppState>,
    claims: Claims,
    Path(id): Path<i64>,
    Json(req): Json<UpdateChecklistItemRequest>,
) -> ApiResult<db::ChecklistItem> {
    // Verify ownership via the parent task
    let item: (i64,) = sqlx::query_as("SELECT task_id FROM checklist_items WHERE id = ?")
        .bind(id)
        .fetch_one(&engine.pool)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Checklist item not found"))?;
    let task = db::get_task(&engine.pool, item.0).await.map_err(internal)?;
    if !is_owner_or_root(task.user_id, &claims) {
        let assignees = db::list_assignees(&engine.pool, item.0)
            .await
            .map_err(internal)?;
        if !assignees.contains(&claims.username) {
            return Err(err(StatusCode::FORBIDDEN, "Not owner or assignee"));
        }
    }
    if let Some(ref t) = req.title {
        if t.trim().is_empty() {
            return Err(err(StatusCode::BAD_REQUEST, "Title cannot be empty"));
        }
    }
    db::update_checklist_item(
        &engine.pool,
        id,
        req.title.as_deref(),
        req.checked,
        req.sort_order,
    )
    .await
    .map(Json)
    .map_err(|e| {
        if e.to_string().contains("not found") {
            err(StatusCode::NOT_FOUND, "Item not found")
        } else {
            internal(e)
        }
    })
}

#[utoipa::path(delete, path = "/api/checklist/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_checklist_item(
    State(engine): State<AppState>,
    claims: Claims,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let item: (i64,) = sqlx::query_as("SELECT task_id FROM checklist_items WHERE id = ?")
        .bind(id)
        .fetch_one(&engine.pool)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Checklist item not found"))?;
    let task = db::get_task(&engine.pool, item.0).await.map_err(internal)?;
    if !is_owner_or_root(task.user_id, &claims) {
        return Err(err(StatusCode::FORBIDDEN, "Not owner"));
    }
    db::delete_checklist_item(&engine.pool, id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Item not found"))?;
    Ok(StatusCode::NO_CONTENT)
}
