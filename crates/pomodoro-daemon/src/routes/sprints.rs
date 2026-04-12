use super::*;

// Q5: Helper to fetch sprint and verify ownership
async fn get_owned_sprint(pool: &db::Pool, id: i64, claims: &Claims) -> Result<db::Sprint, ApiError> {
    let sprint = db::get_sprint(pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Sprint not found"))?;
    if !is_owner_or_root(sprint.created_by_id, claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    Ok(sprint)
}

#[utoipa::path(get, path = "/api/sprints", responses((status = 200, body = Vec<db::Sprint>)), security(("bearer" = [])))]
pub async fn list_sprints(State(engine): State<AppState>, _claims: Claims, Query(q): Query<SprintQuery>) -> ApiResult<Vec<db::Sprint>> {
    db::list_sprints(&engine.pool, q.status.as_deref(), q.project.as_deref()).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/sprints", request_body = CreateSprintRequest, responses((status = 201, body = db::Sprint)), security(("bearer" = [])))]
pub async fn create_sprint(State(engine): State<AppState>, claims: Claims, Json(req): Json<CreateSprintRequest>) -> Result<(StatusCode, Json<db::Sprint>), ApiError> {
    if req.name.trim().is_empty() { return Err(err(StatusCode::BAD_REQUEST, "Sprint name cannot be empty")); }
    if req.name.len() > 200 { return Err(err(StatusCode::BAD_REQUEST, "Sprint name too long (max 200 chars)")); }
    if req.goal.as_ref().map_or(false, |g| g.len() > 1000) { return Err(err(StatusCode::BAD_REQUEST, "Goal too long (max 1000 chars)")); }
    if let Some(ref d) = req.start_date { if chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_err() { return Err(err(StatusCode::BAD_REQUEST, "start_date must be YYYY-MM-DD")); } }
    if let Some(ref d) = req.end_date { if chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_err() { return Err(err(StatusCode::BAD_REQUEST, "end_date must be YYYY-MM-DD")); } }
    // V4: Validate end_date >= start_date
    if let (Some(ref s), Some(ref e)) = (&req.start_date, &req.end_date) { if e < s { return Err(err(StatusCode::BAD_REQUEST, "end_date must be on or after start_date")); } }
    if let Some(ch) = req.capacity_hours { if ch < 0.0 || ch > 10000.0 { return Err(err(StatusCode::BAD_REQUEST, "capacity_hours must be 0-10000")); } }
    let s = db::create_sprint(&engine.pool, claims.user_id, &req.name, req.project.as_deref(), req.goal.as_deref(), req.start_date.as_deref(), req.end_date.as_deref(), req.capacity_hours)
        .await.map_err(internal)?;
    db::audit(&engine.pool, claims.user_id, "create", "sprint", Some(s.id), Some(&s.name)).await.ok();
    crate::webhook::dispatch(engine.pool.clone(), "sprint.created", serde_json::json!({"id": s.id, "name": &s.name}));
    engine.notify(ChangeEvent::Sprints);
    Ok((StatusCode::CREATED, Json(s)))
}

#[utoipa::path(get, path = "/api/sprints/{id}", responses((status = 200, body = db::SprintDetail)), security(("bearer" = [])))]
pub async fn get_sprint_detail(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<db::SprintDetail> {
    db::get_sprint_detail(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(put, path = "/api/sprints/{id}", request_body = UpdateSprintRequest, responses((status = 200, body = db::Sprint)), security(("bearer" = [])))]
pub async fn update_sprint(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<UpdateSprintRequest>) -> ApiResult<db::Sprint> {
    let sprint = get_owned_sprint(&engine.pool, id, &claims).await?;
    if req.goal.as_ref().and_then(|o| o.as_ref()).map_or(false, |g| g.len() > 1000) { return Err(err(StatusCode::BAD_REQUEST, "Goal too long (max 1000)")); }
    if req.retro_notes.as_ref().and_then(|o| o.as_ref()).map_or(false, |r| r.len() > 10000) { return Err(err(StatusCode::BAD_REQUEST, "Retro notes too long (max 10000)")); }
    if req.status.is_some() { return Err(err(StatusCode::BAD_REQUEST, "Use /start or /complete endpoints to change sprint status")); }
    // V1: Validate date ordering on update (resolve effective dates from request or existing sprint)
    {
        let eff_start = req.start_date.as_ref().map(|o| o.as_deref()).unwrap_or(sprint.start_date.as_deref());
        let eff_end = req.end_date.as_ref().map(|o| o.as_deref()).unwrap_or(sprint.end_date.as_deref());
        if let (Some(s), Some(e)) = (eff_start, eff_end) { if e < s { return Err(err(StatusCode::BAD_REQUEST, "end_date must be on or after start_date")); } }
    }
    if let Some(ref expected) = req.expected_updated_at {
        if *expected != sprint.updated_at {
            return Err(err(StatusCode::CONFLICT, "Sprint was modified by another user. Please refresh and try again."));
        }
    }
    let s = db::update_sprint(&engine.pool, id, req.name.as_deref(),
        req.project.as_ref().map(|o| o.as_deref()),
        req.goal.as_ref().map(|o| o.as_deref()),
        None, // B4: Never pass status through update — use /start or /complete
        req.start_date.as_ref().map(|o| o.as_deref()),
        req.end_date.as_ref().map(|o| o.as_deref()),
        req.retro_notes.as_ref().map(|o| o.as_deref()),
        req.capacity_hours.as_ref().map(|o| *o))
        .await.map_err(internal)?;
    engine.notify(ChangeEvent::Sprints);
    Ok(Json(s))
}

#[utoipa::path(delete, path = "/api/sprints/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_sprint(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    get_owned_sprint(&engine.pool, id, &claims).await?;
    db::delete_sprint(&engine.pool, id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Sprints);
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/sprints/{id}/start", responses((status = 200, body = db::Sprint)), security(("bearer" = [])))]
pub async fn start_sprint(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> ApiResult<db::Sprint> {
    let sprint = get_owned_sprint(&engine.pool, id, &claims).await?;
    if sprint.status != "planning" { return Err(err(StatusCode::BAD_REQUEST, format!("Cannot start sprint in '{}' status", sprint.status))); }
    let s = db::update_sprint(&engine.pool, id, None, None, None, Some("active"), None, None, None, None).await.map_err(internal)?;
    db::audit(&engine.pool, claims.user_id, "start", "sprint", Some(id), None).await.ok();
    crate::webhook::dispatch(engine.pool.clone(), "sprint.started", serde_json::json!({"id": id}));
    // BL22: Notify all sprint task assignees
    if let Ok(tasks) = db::get_sprint_tasks(&engine.pool, id).await {
        let mut notified = std::collections::HashSet::new();
        for t in &tasks {
            if notified.insert(t.user_id) && t.user_id != claims.user_id {
                db::create_notification(&engine.pool, t.user_id, "sprint_started", &format!("Sprint '{}' has started", sprint.name), Some("sprint"), Some(id)).await.ok();
            }
        }
    }
    if let Err(e) = db::snapshot_sprint(&engine.pool, id).await { tracing::warn!("Snapshot failed: {}", e); }
    engine.notify(ChangeEvent::Sprints);
    Ok(Json(s))
}

#[utoipa::path(post, path = "/api/sprints/{id}/complete", responses((status = 200, body = db::Sprint)), security(("bearer" = [])))]
pub async fn complete_sprint(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> ApiResult<db::Sprint> {
    let sprint = get_owned_sprint(&engine.pool, id, &claims).await?;
    if sprint.status != "active" { return Err(err(StatusCode::BAD_REQUEST, format!("Cannot complete sprint in '{}' status", sprint.status))); }
    if let Err(e) = db::snapshot_sprint(&engine.pool, id).await { tracing::warn!("Snapshot failed: {}", e); }
    let s = db::update_sprint(&engine.pool, id, None, None, None, Some("completed"), None, None, None, None).await.map_err(internal)?;
    db::audit(&engine.pool, claims.user_id, "complete", "sprint", Some(id), None).await.ok();
    crate::webhook::dispatch(engine.pool.clone(), "sprint.completed", serde_json::json!({"id": id}));
    // BL22: Notify all sprint task assignees
    if let Ok(tasks) = db::get_sprint_tasks(&engine.pool, id).await {
        let mut notified = std::collections::HashSet::new();
        for t in &tasks {
            if notified.insert(t.user_id) && t.user_id != claims.user_id {
                db::create_notification(&engine.pool, t.user_id, "sprint_completed", &format!("Sprint '{}' completed", sprint.name), Some("sprint"), Some(id)).await.ok();
            }
        }
    }
    engine.notify(ChangeEvent::Sprints);
    Ok(Json(s))
}

// F5: Sprint carry-over — move incomplete tasks to a new sprint
#[utoipa::path(post, path = "/api/sprints/{id}/carryover", responses((status = 200, body = db::Sprint)), security(("bearer" = [])))]
pub async fn carryover_sprint(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> ApiResult<db::Sprint> {
    let sprint = get_owned_sprint(&engine.pool, id, &claims).await?;
    if sprint.status != "completed" { return Err(err(StatusCode::BAD_REQUEST, "Sprint must be completed first")); }
    let tasks = db::get_sprint_tasks(&engine.pool, id).await.map_err(internal)?;
    let incomplete: Vec<i64> = tasks.iter().filter(|t| t.status != "completed" && t.status != "archived").map(|t| t.id).collect();
    if incomplete.is_empty() { return Err(err(StatusCode::BAD_REQUEST, "No incomplete tasks to carry over")); }
    let new_name = format!("{} (carry-over)", sprint.name);
    let new_sprint = db::create_sprint(&engine.pool, claims.user_id, &new_name, sprint.project.as_deref(), None, None, None, sprint.capacity_hours).await.map_err(internal)?;
    db::add_sprint_tasks(&engine.pool, new_sprint.id, &incomplete, claims.user_id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Sprints);
    Ok(Json(new_sprint))
}

#[utoipa::path(get, path = "/api/sprints/{id}/tasks", responses((status = 200, body = Vec<db::Task>)), security(("bearer" = [])))]
pub async fn get_sprint_tasks(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<db::Task>> {
    db::get_sprint_tasks(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/sprints/{id}/tasks", request_body = AddSprintTasksRequest, responses((status = 200, body = Vec<db::SprintTask>)), security(("bearer" = [])))]
pub async fn add_sprint_tasks(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<AddSprintTasksRequest>) -> ApiResult<Vec<db::SprintTask>> {
    get_owned_sprint(&engine.pool, id, &claims).await?;
    if req.task_ids.len() > 500 { return Err(err(StatusCode::BAD_REQUEST, "Too many task IDs (max 500)")); }
    if req.task_ids.is_empty() { return Err(err(StatusCode::BAD_REQUEST, "task_ids cannot be empty")); }
    // V2: Deduplicate task IDs
    let task_ids: Vec<i64> = req.task_ids.iter().copied().collect::<std::collections::HashSet<_>>().into_iter().collect();
    // Batch validate all tasks exist and are not soft-deleted
    let ph = task_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let q = format!("SELECT COUNT(*) FROM tasks WHERE id IN ({}) AND deleted_at IS NULL", ph);
    let mut query = sqlx::query_as::<_, (i64,)>(&q);
    for id in &task_ids { query = query.bind(id); }
    let (found,): (i64,) = query.fetch_one(&engine.pool).await.map_err(internal)?;
    if found != task_ids.len() as i64 { return Err(err(StatusCode::NOT_FOUND, "One or more tasks not found")); }
    let result = db::add_sprint_tasks(&engine.pool, id, &task_ids, claims.user_id).await.map_err(internal)?;
    // BL8: Audit sprint scope changes
    db::audit(&engine.pool, claims.user_id, "add_tasks", "sprint", Some(id), Some(&format!("{} tasks added", task_ids.len()))).await.ok();
    // BL7: Notify task owners about sprint scope change
    let sprint_name = db::get_sprint(&engine.pool, id).await.map(|s| s.name.clone()).unwrap_or_default();
    for tid in &task_ids {
        if let Ok(task) = db::get_task(&engine.pool, *tid).await {
            if task.user_id != claims.user_id {
                db::create_notification(&engine.pool, task.user_id, "sprint_started", &format!("Your task '{}' was added to sprint '{}'", task.title, sprint_name), Some("sprint"), Some(id)).await.ok();
            }
        }
    }
    if db::get_sprint(&engine.pool, id).await.map(|s| s.status == "active").unwrap_or(false) {
        if let Err(e) = db::snapshot_sprint(&engine.pool, id).await { tracing::warn!("Snapshot failed: {}", e); }
    }
    engine.notify(ChangeEvent::Sprints);
    Ok(Json(result))
}

#[utoipa::path(delete, path = "/api/sprints/{id}/tasks/{task_id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn remove_sprint_task(State(engine): State<AppState>, claims: Claims, Path((id, task_id)): Path<(i64, i64)>) -> Result<StatusCode, ApiError> {
    get_owned_sprint(&engine.pool, id, &claims).await?;
    db::remove_sprint_task(&engine.pool, id, task_id).await.map_err(internal)?;
    // BL8: Audit sprint scope changes
    db::audit(&engine.pool, claims.user_id, "remove_task", "sprint", Some(id), Some(&format!("task {} removed", task_id))).await.ok();
    if db::get_sprint(&engine.pool, id).await.map(|s| s.status == "active").unwrap_or(false) {
        if let Err(e) = db::snapshot_sprint(&engine.pool, id).await { tracing::warn!("Snapshot failed: {}", e); }
    }
    engine.notify(ChangeEvent::Sprints);
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/sprints/{id}/burndown", responses((status = 200, body = Vec<db::SprintDailyStat>)), security(("bearer" = [])))]
pub async fn get_sprint_burndown(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<db::SprintDailyStat>> {
    db::get_sprint_burndown(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(get, path = "/api/sprints/burndown", responses((status = 200, body = Vec<db::SprintDailyStat>)), security(("bearer" = [])))]
pub async fn get_global_burndown(State(engine): State<AppState>, _claims: Claims) -> ApiResult<Vec<db::SprintDailyStat>> {
    db::get_global_burndown(&engine.pool).await.map(Json).map_err(internal)
}

// --- Epic Groups ---

#[derive(Deserialize)]
pub struct VelocityQuery { pub sprints: Option<i64> }

// F3: Sprint comparison
#[derive(Deserialize)]
pub struct CompareQuery { pub a: i64, pub b: i64 }

#[utoipa::path(get, path = "/api/sprints/compare", responses((status = 200)), security(("bearer" = [])))]
pub async fn compare_sprints(State(engine): State<AppState>, _claims: Claims, Query(q): Query<CompareQuery>) -> ApiResult<serde_json::Value> {
    let (a, b) = tokio::join!(
        db::get_sprint_detail(&engine.pool, q.a),
        db::get_sprint_detail(&engine.pool, q.b)
    );
    let a = a.map_err(|_| err(StatusCode::NOT_FOUND, "Sprint A not found"))?;
    let b = b.map_err(|_| err(StatusCode::NOT_FOUND, "Sprint B not found"))?;
    let done_a = a.tasks.iter().filter(|t| t.status == "completed").count();
    let done_b = b.tasks.iter().filter(|t| t.status == "completed").count();
    Ok(Json(serde_json::json!({
        "a": {"id": a.sprint.id, "name": a.sprint.name, "total_tasks": a.tasks.len(), "done_tasks": done_a, "capacity_hours": a.sprint.capacity_hours},
        "b": {"id": b.sprint.id, "name": b.sprint.name, "total_tasks": b.tasks.len(), "done_tasks": done_b, "capacity_hours": b.sprint.capacity_hours},
    })))
}

#[utoipa::path(get, path = "/api/sprints/velocity", responses((status = 200)), security(("bearer" = [])))]
pub async fn get_velocity(State(engine): State<AppState>, _claims: Claims, Query(q): Query<VelocityQuery>) -> ApiResult<Vec<serde_json::Value>> {
    let n = q.sprints.unwrap_or(10).min(50);
    let rows = db::get_velocity(&engine.pool, n).await.map_err(internal)?;
    let result: Vec<serde_json::Value> = rows.into_iter().map(|(name, points, hours, tasks)| {
        serde_json::json!({ "sprint": name, "points": points, "hours": hours, "tasks_done": tasks })
    }).collect();
    Ok(Json(result))
}
