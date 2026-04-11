use super::*;




#[derive(Deserialize)]
pub struct TaskQuery {
    pub status: Option<String>, pub project: Option<String>,
    pub page: Option<i64>, pub per_page: Option<i64>, pub team_id: Option<i64>,
    pub search: Option<String>, pub assignee: Option<String>,
    pub due_before: Option<String>, pub due_after: Option<String>,
    pub priority: Option<i64>,
}

#[utoipa::path(get, path = "/api/tasks", responses((status = 200, body = Vec<db::Task>)), security(("bearer" = [])))]
pub async fn list_tasks(State(engine): State<AppState>, _claims: Claims, Query(q): Query<TaskQuery>) -> Result<axum::response::Response, ApiError> {
    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(5000).min(5000);
    let offset = (page - 1) * per_page;
    let filter = db::TaskFilter {
        status: q.status.as_deref(), project: q.project.as_deref(),
        search: q.search.as_deref(), assignee: q.assignee.as_deref(),
        due_before: q.due_before.as_deref(), due_after: q.due_after.as_deref(),
        priority: q.priority, team_id: q.team_id,
    };
    let tasks = db::list_tasks_paged(&engine.pool, filter, per_page, offset).await.map_err(internal)?;
    // Only compute total count if pagination is explicitly requested
    let total = if q.page.is_some() {
        let filter2 = db::TaskFilter {
            status: q.status.as_deref(), project: q.project.as_deref(),
            search: q.search.as_deref(), assignee: q.assignee.as_deref(),
            due_before: q.due_before.as_deref(), due_after: q.due_after.as_deref(),
            priority: q.priority, team_id: None, // team filter handled separately
        };
        Some(db::count_tasks(&engine.pool, filter2).await.map_err(internal)?)
    } else { None };
    let body = serde_json::to_vec(&tasks).map_err(internal)?;
    let mut resp = axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json");
    if let Some(total) = total {
        resp = resp
            .header("x-total-count", total.to_string())
            .header("x-page", page.to_string())
            .header("x-per-page", per_page.to_string());
    }
    Ok(resp.body(axum::body::Body::from(body)).map_err(|e| internal(e.to_string()))?)
}

#[utoipa::path(post, path = "/api/tasks", request_body = CreateTaskRequest, responses((status = 201, body = db::Task)), security(("bearer" = [])))]
pub async fn create_task(State(engine): State<AppState>, claims: Claims, Json(req): Json<CreateTaskRequest>) -> Result<(StatusCode, Json<db::Task>), ApiError> {
    if req.title.trim().is_empty() { return Err(err(StatusCode::BAD_REQUEST, "Title cannot be empty")); }
    let priority = req.priority.unwrap_or(3);
    if priority < 1 || priority > 5 { return Err(err(StatusCode::BAD_REQUEST, "Priority must be 1-5")); }
    let estimated = req.estimated.unwrap_or(1);
    if estimated < 0 { return Err(err(StatusCode::BAD_REQUEST, "Estimated cannot be negative")); }
    if req.estimated_hours.map_or(false, |h| h < 0.0) { return Err(err(StatusCode::BAD_REQUEST, "Estimated hours cannot be negative")); }
    let t = db::create_task(&engine.pool, claims.user_id, req.parent_id, req.title.trim(), req.description.as_deref(), req.project.as_deref(), req.tags.as_deref(), priority, estimated, req.estimated_hours.unwrap_or(0.0), req.remaining_points.unwrap_or(0.0), req.due_date.as_deref())
        .await.map_err(internal)?;
    if let Err(e) = db::audit(&engine.pool, claims.user_id, "create", "task", Some(t.id), Some(&t.title)).await { tracing::warn!("Audit log failed: {}", e); }
    crate::webhook::dispatch(engine.pool.clone(), "task.created", serde_json::json!({"id": t.id, "title": &t.title}));
    engine.notify(ChangeEvent::Tasks);
    Ok((StatusCode::CREATED, Json(t)))
}

#[utoipa::path(get, path = "/api/tasks/{id}", responses((status = 200, body = db::TaskDetail)), security(("bearer" = [])))]
pub async fn get_task_detail(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<db::TaskDetail> {
    db::get_task_detail(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(put, path = "/api/tasks/{id}", request_body = UpdateTaskRequest, responses((status = 200, body = db::Task)), security(("bearer" = [])))]
pub async fn update_task(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<UpdateTaskRequest>) -> ApiResult<db::Task> {
    let task = db::get_task(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    if !is_owner_or_root(task.user_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    if let Some(ref s) = req.status { validate_task_status(s)?; }
    if let Some(p) = req.priority { if p < 1 || p > 5 { return Err(err(StatusCode::BAD_REQUEST, "Priority must be 1-5")); } }
    if let Some(e) = req.estimated { if e < 0 { return Err(err(StatusCode::BAD_REQUEST, "Estimated cannot be negative")); } }
    if let Some(h) = req.estimated_hours { if h < 0.0 { return Err(err(StatusCode::BAD_REQUEST, "Estimated hours cannot be negative")); } }
    if let Some(ref expected) = req.expected_updated_at {
        if *expected != task.updated_at {
            return Err(err(StatusCode::CONFLICT, "Task was modified by another user. Please refresh and try again."));
        }
    }
    let t = db::update_task(&engine.pool, id, req.title.as_deref(),
        req.description.as_ref().map(|o| o.as_deref()),
        req.project.as_ref().map(|o| o.as_deref()),
        req.tags.as_ref().map(|o| o.as_deref()),
        req.priority, req.estimated, req.estimated_hours, req.remaining_points,
        req.due_date.as_ref().map(|o| o.as_deref()),
        req.status.as_deref(), req.sort_order, req.parent_id)
        .await.map_err(internal)?;
    if let Err(e) = db::audit(&engine.pool, claims.user_id, "update", "task", Some(id), None).await { tracing::warn!("Audit log failed: {}", e); }
    crate::webhook::dispatch(engine.pool.clone(), "task.updated", serde_json::json!({"id": id}));
    engine.notify(ChangeEvent::Tasks);
    Ok(Json(t))
}

#[utoipa::path(delete, path = "/api/tasks/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_task(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<impl IntoResponse, ApiError> {
    let task = db::get_task(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    if !is_owner_or_root(task.user_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    db::delete_task(&engine.pool, id).await.map_err(internal)?;
    if let Err(e) = db::audit(&engine.pool, claims.user_id, "delete", "task", Some(id), Some(&task.title)).await { tracing::warn!("Audit log failed: {}", e); }
    crate::webhook::dispatch(engine.pool.clone(), "task.deleted", serde_json::json!({"id": id, "title": &task.title}));
    engine.notify(ChangeEvent::Tasks);
    Ok(StatusCode::NO_CONTENT)
}

// --- Comments ---



#[derive(Deserialize, utoipa::ToSchema)]
pub struct ReorderRequest { pub orders: Vec<(i64, i64)> }

#[utoipa::path(post, path = "/api/tasks/reorder", responses((status = 204)), security(("bearer" = [])))]
pub async fn reorder_tasks(State(engine): State<AppState>, _claims: Claims, Json(req): Json<ReorderRequest>) -> Result<StatusCode, ApiError> {
    if req.orders.len() > 500 { return Err(err(StatusCode::BAD_REQUEST, "Too many items (max 500)")); }
    db::reorder_tasks(&engine.pool, &req.orders).await.map_err(internal)?;
    engine.notify(ChangeEvent::Tasks);
    Ok(StatusCode::NO_CONTENT)
}
