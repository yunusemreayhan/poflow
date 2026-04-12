use super::*;

#[utoipa::path(post, path = "/api/tasks/{id}/duplicate", responses((status = 201, body = db::Task)), security(("bearer" = [])))]
pub async fn duplicate_task(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<(StatusCode, Json<db::Task>), ApiError> {
    let task = db::get_task(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    if task.deleted_at.is_some() { return Err(err(StatusCode::BAD_REQUEST, "Cannot duplicate deleted task")); }
    // BL7: Only owner or root can duplicate
    if task.user_id != claims.user_id && claims.role != "root" { return Err(err(StatusCode::FORBIDDEN, "Not your task")); }
    let t = db::create_task(&engine.pool, claims.user_id, task.parent_id,
        &format!("{} (copy)", task.title), task.description.as_deref(),
        task.project.as_deref(), task.tags.as_deref(),
        task.priority as i64, task.estimated as i64,
        task.estimated_hours, task.remaining_points, task.due_date.as_deref())
        .await.map_err(internal)?;
    // Copy work_duration_minutes if set
    if task.work_duration_minutes.is_some() {
        db::update_task(&engine.pool, t.id, None, None, None, None, None, None, None, None, None, None, None, None, task.work_duration_minutes.map(Some)).await.ok();
    }
    // Copy labels
    if let Ok(labels) = db::get_task_labels(&engine.pool, id).await {
        for label in labels { db::add_task_label(&engine.pool, t.id, label.id).await.ok(); }
    }
    // B5: Copy assignees
    let assignee_ids: Vec<(i64,)> = sqlx::query_as("SELECT user_id FROM task_assignees WHERE task_id = ?")
        .bind(id).fetch_all(&engine.pool).await.unwrap_or_default();
    for (uid,) in assignee_ids { db::add_assignee(&engine.pool, t.id, uid).await.ok(); }
    engine.notify(ChangeEvent::Tasks);
    Ok((StatusCode::CREATED, Json(t)))
}

pub(super) fn valid_date(s: &str) -> bool {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok()
}

// F1: Search endpoint with highlighted snippets
#[derive(Deserialize)]
pub struct SearchQuery { pub q: String, pub limit: Option<i64> }

#[utoipa::path(get, path = "/api/tasks/search", responses((status = 200)), security(("bearer" = [])))]
pub async fn search_tasks(State(engine): State<AppState>, claims: Claims, Query(q): Query<SearchQuery>) -> ApiResult<Vec<serde_json::Value>> {
    if q.q.trim().is_empty() { return Ok(Json(vec![])); }
    let limit = q.limit.unwrap_or(20).min(100);
    // B2: Non-root users only see their own tasks
    let user_id = if claims.role == "root" { None } else { Some(claims.user_id) };
    let results = db::search_tasks_fts(&engine.pool, &q.q, limit, user_id).await.map_err(internal)?;
    Ok(Json(results.into_iter().map(|(id, title, snippet)| serde_json::json!({"id": id, "title": title, "snippet": snippet})).collect()))
}

// F4: Task time tracking summary
#[utoipa::path(get, path = "/api/tasks/{id}/time-summary", responses((status = 200)), security(("bearer" = [])))]
pub async fn get_task_time_summary(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> ApiResult<serde_json::Value> {
    // B3: Verify task exists
    db::get_task(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    let rows: Vec<(String, f64, i64)> = sqlx::query_as(
        "SELECT u.username, COALESCE(SUM(s.duration_s),0)/3600.0, COUNT(s.id) \
         FROM sessions s JOIN users u ON s.user_id = u.id \
         WHERE s.task_id = ? AND s.status = 'completed' GROUP BY s.user_id ORDER BY SUM(s.duration_s) DESC")
        .bind(id).fetch_all(&engine.pool).await.map_err(internal)?;
    let total_hours: f64 = rows.iter().map(|(_, h, _)| h).sum();
    let by_user: Vec<serde_json::Value> = rows.into_iter().map(|(u, h, c)| serde_json::json!({"username": u, "hours": (h * 100.0).round() / 100.0, "sessions": c})).collect();
    Ok(Json(serde_json::json!({"task_id": id, "total_hours": (total_hours * 100.0).round() / 100.0, "by_user": by_user})))
}

#[utoipa::path(get, path = "/api/tasks/trash", responses((status = 200, body = Vec<db::Task>)), security(("bearer" = [])))]
pub async fn list_deleted_tasks(State(engine): State<AppState>, claims: Claims) -> ApiResult<Vec<db::Task>> {
    let user_filter = if claims.role == "root" { None } else { Some(claims.user_id) };
    db::list_deleted_tasks(&engine.pool, user_filter).await.map(Json).map_err(internal)
}

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
        priority: q.priority, team_id: q.team_id, user_id: None,
    };
    let tasks = db::list_tasks_paged(&engine.pool, filter, per_page, offset).await.map_err(internal)?;
    // Only compute total count if pagination is explicitly requested
    let total = if q.page.is_some() {
        let filter2 = db::TaskFilter {
            status: q.status.as_deref(), project: q.project.as_deref(),
            search: q.search.as_deref(), assignee: q.assignee.as_deref(),
            due_before: q.due_before.as_deref(), due_after: q.due_after.as_deref(),
            priority: q.priority, team_id: q.team_id, user_id: None,
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
    if req.title.len() > 500 { return Err(err(StatusCode::BAD_REQUEST, "Title too long (max 500 chars)")); }
    if req.description.as_ref().map_or(false, |d| d.len() > 10000) { return Err(err(StatusCode::BAD_REQUEST, "Description too long (max 10000 chars)")); }
    if req.project.as_ref().map_or(false, |p| p.len() > 200) { return Err(err(StatusCode::BAD_REQUEST, "Project too long (max 200 chars)")); }
    if req.tags.as_ref().map_or(false, |t| t.len() > 500) { return Err(err(StatusCode::BAD_REQUEST, "Tags too long (max 500 chars)")); }
    let priority = req.priority.unwrap_or(3);
    if priority < 1 || priority > 5 { return Err(err(StatusCode::BAD_REQUEST, "Priority must be 1-5")); }
    let estimated = req.estimated.unwrap_or(1);
    if estimated < 0 { return Err(err(StatusCode::BAD_REQUEST, "Estimated cannot be negative")); }
    if req.estimated_hours.map_or(false, |h| h < 0.0) { return Err(err(StatusCode::BAD_REQUEST, "Estimated hours cannot be negative")); }
    if let Some(ref d) = req.due_date { if !valid_date(d) { return Err(err(StatusCode::BAD_REQUEST, "due_date must be YYYY-MM-DD")); } }
    // BL6: Validate parent_id exists and belongs to user (or is root)
    if let Some(pid) = req.parent_id {
        let parent = db::get_task(&engine.pool, pid).await.map_err(|_| err(StatusCode::NOT_FOUND, "Parent task not found"))?;
        if parent.user_id != claims.user_id && claims.role != "root" { return Err(err(StatusCode::FORBIDDEN, "Parent task not owned by you")); }
    }
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

#[utoipa::path(get, path = "/api/tasks/{id}/sessions", responses((status = 200, body = Vec<db::Session>)), security(("bearer" = [])))]
pub async fn get_task_sessions(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<db::Session>> {
    db::get_task_sessions(&engine.pool, id).await.map(Json).map_err(internal)
}

// F7: Update session note
#[utoipa::path(put, path = "/api/sessions/{id}/note", request_body = UpdateSessionNoteRequest, responses((status = 200, body = db::Session)), security(("bearer" = [])))]
pub async fn update_session_note(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<UpdateSessionNoteRequest>) -> ApiResult<db::Session> {
    let session = db::get_session(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Session not found"))?;
    if !is_owner_or_root(session.user_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not session owner")); }
    if req.note.len() > 5000 { return Err(err(StatusCode::BAD_REQUEST, "Note too long (max 5000 chars)")); }
    db::update_session_note(&engine.pool, id, &req.note).await.map(Json).map_err(internal)
}

#[utoipa::path(put, path = "/api/tasks/{id}", request_body = UpdateTaskRequest, responses((status = 200, body = db::Task)), security(("bearer" = [])))]
pub async fn update_task(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<UpdateTaskRequest>) -> ApiResult<db::Task> {
    let task = db::get_task(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    if !is_owner_or_root(task.user_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    if let Some(ref t) = req.title { if t.trim().is_empty() { return Err(err(StatusCode::BAD_REQUEST, "Title cannot be empty")); } if t.len() > 500 { return Err(err(StatusCode::BAD_REQUEST, "Title too long (max 500)")); } }
    if let Some(ref d) = req.description { if d.as_ref().map_or(false, |d| d.len() > 10000) { return Err(err(StatusCode::BAD_REQUEST, "Description too long")); } }
    if let Some(ref s) = req.status { validate_task_status(s)?; }
    if let Some(p) = req.priority { if p < 1 || p > 5 { return Err(err(StatusCode::BAD_REQUEST, "Priority must be 1-5")); } }
    if let Some(e) = req.estimated { if e < 0 { return Err(err(StatusCode::BAD_REQUEST, "Estimated cannot be negative")); } }
    if let Some(h) = req.estimated_hours { if h < 0.0 { return Err(err(StatusCode::BAD_REQUEST, "Estimated hours cannot be negative")); } }
    if let Some(ref dd) = req.due_date { if let Some(ref d) = dd { if !valid_date(d) { return Err(err(StatusCode::BAD_REQUEST, "due_date must be YYYY-MM-DD")); } } }
    // V7: Prevent circular parent_id references
    if let Some(Some(new_parent)) = req.parent_id {
        if new_parent == id { return Err(err(StatusCode::BAD_REQUEST, "Task cannot be its own parent")); }
        // Walk up the ancestor chain to detect cycles
        let mut ancestor = Some(new_parent);
        let mut depth = 0;
        while let Some(aid) = ancestor {
            depth += 1;
            if depth > 50 { break; }
            if aid == id { return Err(err(StatusCode::BAD_REQUEST, "Circular parent reference detected")); }
            ancestor = db::get_task(&engine.pool, aid).await.ok().and_then(|t| t.parent_id);
        }
    }
    if let Some(ref expected) = req.expected_updated_at {
        if *expected != task.updated_at {
            return Err(err(StatusCode::CONFLICT, "Task was modified by another user. Please refresh and try again."));
        }
    }
    if let Some(Some(wdm)) = req.work_duration_minutes { if wdm < 1 || wdm > 480 { return Err(err(StatusCode::BAD_REQUEST, "work_duration_minutes must be 1-480")); } }
    let t = db::update_task(&engine.pool, id, req.title.as_deref(),
        req.description.as_ref().map(|o| o.as_deref()),
        req.project.as_ref().map(|o| o.as_deref()),
        req.tags.as_ref().map(|o| o.as_deref()),
        req.priority, req.estimated, req.estimated_hours, req.remaining_points,
        req.due_date.as_ref().map(|o| o.as_deref()),
        req.status.as_deref(), req.sort_order, req.parent_id,
        req.work_duration_minutes.as_ref().map(|o| *o))
        .await.map_err(internal)?;
    if let Err(e) = db::audit(&engine.pool, claims.user_id, "update", "task", Some(id), None).await { tracing::warn!("Audit log failed: {}", e); }
    // BL3: Auto-unblock dependents when task is completed
    if req.status.as_deref() == Some("completed") {
        if let Ok(dependents) = db::get_dependents(&engine.pool, id).await {
            for dep_id in dependents {
                if let Ok(dep_task) = db::get_task(&engine.pool, dep_id).await {
                    if dep_task.status == "blocked" {
                        // Check if ALL dependencies are now completed
                        if let Ok(deps) = db::get_dependencies(&engine.pool, dep_id).await {
                            let mut all_done = true;
                            for d in &deps {
                                if let Ok(dt) = db::get_task(&engine.pool, *d).await {
                                    // BL3: Soft-deleted or completed deps count as resolved
                                    if dt.deleted_at.is_none() && dt.status != "completed" { all_done = false; break; }
                                }
                            }
                            if all_done {
                                db::update_task(&engine.pool, dep_id, None, None, None, None, None, None, None, None, None, Some("backlog"), None, None, None).await.ok();
                            }
                        }
                    }
                }
            }
        }
    }
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

#[utoipa::path(post, path = "/api/tasks/{id}/restore", responses((status = 204)), security(("bearer" = [])))]
pub async fn restore_task(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    let task: (i64,) = sqlx::query_as("SELECT user_id FROM tasks WHERE id = ?")
        .bind(id).fetch_one(&engine.pool).await.map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    if !is_owner_or_root(task.0, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    db::restore_task(&engine.pool, id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Tasks);
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/api/tasks/{id}/permanent", responses((status = 204)), security(("bearer" = [])))]
pub async fn purge_task(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    let task: (i64, Option<String>) = sqlx::query_as("SELECT user_id, deleted_at FROM tasks WHERE id = ?")
        .bind(id).fetch_one(&engine.pool).await.map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    if !is_owner_or_root(task.0, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    if task.1.is_none() { return Err(err(StatusCode::BAD_REQUEST, "Task must be in trash first")); }
    sqlx::query("DELETE FROM tasks WHERE id = ? AND deleted_at IS NOT NULL").bind(id).execute(&engine.pool).await.map_err(internal)?;
    engine.notify(ChangeEvent::Tasks);
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct BulkStatusRequest { pub task_ids: Vec<i64>, pub status: String }

#[utoipa::path(put, path = "/api/tasks/bulk-status", request_body = BulkStatusRequest, responses((status = 204)), security(("bearer" = [])))]
pub async fn bulk_update_status(State(engine): State<AppState>, claims: Claims, Json(req): Json<BulkStatusRequest>) -> Result<StatusCode, ApiError> {
    validate_task_status(&req.status)?;
    if req.task_ids.is_empty() { return Ok(StatusCode::NO_CONTENT); }
    if req.task_ids.len() > 500 { return Err(err(StatusCode::BAD_REQUEST, "Too many task IDs (max 500)")); }
    // Batch ownership check
    if claims.role != "root" {
        let ph = req.task_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let q = format!("SELECT COUNT(*) FROM tasks WHERE id IN ({}) AND user_id != ?", ph);
        let mut query = sqlx::query_as::<_, (i64,)>(&q);
        for id in &req.task_ids { query = query.bind(id); }
        query = query.bind(claims.user_id);
        let (foreign,): (i64,) = query.fetch_one(&engine.pool).await.map_err(internal)?;
        if foreign > 0 { return Err(err(StatusCode::FORBIDDEN, "Cannot update other users' tasks")); }
    }
    // Batch update
    let ph = req.task_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!("UPDATE tasks SET status = ?, updated_at = ? WHERE id IN ({}) AND deleted_at IS NULL", ph);
    let mut q = sqlx::query(&sql).bind(&req.status).bind(crate::db::now_str());
    for id in &req.task_ids { q = q.bind(id); }
    q.execute(&engine.pool).await.map_err(internal)?;
    // B7: Auto-unblock dependents when bulk-completing tasks
    if req.status == "completed" {
        for tid in &req.task_ids {
            if let Ok(dependents) = db::get_dependents(&engine.pool, *tid).await {
                for dep_id in dependents {
                    if let Ok(dep_task) = db::get_task(&engine.pool, dep_id).await {
                        if dep_task.status == "blocked" {
                            if let Ok(deps) = db::get_dependencies(&engine.pool, dep_id).await {
                                let mut all_done = true;
                                for d in &deps {
                                    if let Ok(dt) = db::get_task(&engine.pool, *d).await {
                                        if dt.deleted_at.is_none() && dt.status != "completed" { all_done = false; break; }
                                    }
                                }
                                if all_done {
                                    db::update_task(&engine.pool, dep_id, None, None, None, None, None, None, None, None, None, Some("backlog"), None, None, None).await.ok();
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // B4: Fire webhooks for bulk status updates
    crate::webhook::dispatch(engine.pool.clone(), "task.updated", serde_json::json!({"ids": &req.task_ids, "status": &req.status, "bulk": true}));
    engine.notify(ChangeEvent::Tasks);
    Ok(StatusCode::NO_CONTENT)
}


#[derive(Deserialize, utoipa::ToSchema)]
pub struct ReorderRequest { pub orders: Vec<(i64, i64)> }

#[utoipa::path(post, path = "/api/tasks/reorder", responses((status = 204)), security(("bearer" = [])))]
pub async fn reorder_tasks(State(engine): State<AppState>, claims: Claims, Json(req): Json<ReorderRequest>) -> Result<StatusCode, ApiError> {
    if req.orders.len() > 500 { return Err(err(StatusCode::BAD_REQUEST, "Too many items (max 500)")); }
    // Verify user owns all tasks (root can reorder any)
    if claims.role != "root" && !req.orders.is_empty() {
        let ids: Vec<i64> = req.orders.iter().map(|&(id, _)| id).collect();
        let ph = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let q = format!("SELECT COUNT(*) as c FROM tasks WHERE id IN ({}) AND user_id != ?", ph);
        let mut query = sqlx::query_as::<_, (i64,)>(&q);
        for id in &ids { query = query.bind(id); }
        query = query.bind(claims.user_id);
        let (foreign,): (i64,) = query.fetch_one(&engine.pool).await.map_err(internal)?;
        if foreign > 0 { return Err(err(StatusCode::FORBIDDEN, "Cannot reorder other users' tasks")); }
    }
    db::reorder_tasks(&engine.pool, &req.orders).await.map_err(internal)?;
    engine.notify(ChangeEvent::Tasks);
    Ok(StatusCode::NO_CONTENT)
}
