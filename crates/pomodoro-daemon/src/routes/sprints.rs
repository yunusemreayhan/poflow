use super::*;

// Q5: Helper to fetch sprint and verify ownership
async fn get_owned_sprint(pool: &db::Pool, id: i64, claims: &Claims) -> Result<db::Sprint, ApiError> {
    let sprint = db::get_sprint(pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Sprint not found"))?;
    if !is_owner_or_root(sprint.created_by_id, claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    Ok(sprint)
}

#[utoipa::path(get, path = "/api/sprints", responses((status = 200, body = Vec<db::Sprint>)), security(("bearer" = [])))]
pub async fn list_sprints(State(engine): State<AppState>, _claims: Claims, Query(q): Query<SprintQuery>) -> ApiResult<Vec<db::Sprint>> {
    if let Some(ref s) = q.status { validate_sprint_status(s)?; }
    db::list_sprints(&engine.pool, q.status.as_deref(), q.project.as_deref()).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/sprints", request_body = CreateSprintRequest, responses((status = 201, body = db::Sprint)), security(("bearer" = [])))]
pub async fn create_sprint(State(engine): State<AppState>, claims: Claims, Json(req): Json<CreateSprintRequest>) -> Result<(StatusCode, Json<db::Sprint>), ApiError> {
    if req.name.trim().is_empty() { return Err(err(StatusCode::BAD_REQUEST, "Sprint name cannot be empty")); }
    if req.name.len() > 200 { return Err(err(StatusCode::BAD_REQUEST, "Sprint name too long (max 200 chars)")); }
    if req.goal.as_ref().is_some_and(|g| g.len() > 1000) { return Err(err(StatusCode::BAD_REQUEST, "Goal too long (max 1000 chars)")); }
    if req.project.as_ref().is_some_and(|p| p.len() > 200) { return Err(err(StatusCode::BAD_REQUEST, "Project name too long (max 200 chars)")); }
    if let Some(ref d) = req.start_date { if chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_err() { return Err(err(StatusCode::BAD_REQUEST, "start_date must be YYYY-MM-DD")); } }
    if let Some(ref d) = req.end_date { if chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_err() { return Err(err(StatusCode::BAD_REQUEST, "end_date must be YYYY-MM-DD")); } }
    // V4: Validate end_date >= start_date
    if let (Some(ref s), Some(ref e)) = (&req.start_date, &req.end_date) { if e < s { return Err(err(StatusCode::BAD_REQUEST, "end_date must be on or after start_date")); } }
    if let Some(ch) = req.capacity_hours { if !(0.0..=10000.0).contains(&ch) { return Err(err(StatusCode::BAD_REQUEST, "capacity_hours must be 0-10000")); } }
    let s = db::create_sprint(&engine.pool, claims.user_id, &req.name, req.project.as_deref(), req.goal.as_deref(), req.start_date.as_deref(), req.end_date.as_deref(), req.capacity_hours)
        .await.map_err(internal)?;
    db::audit(&engine.pool, claims.user_id, "create", "sprint", Some(s.id), Some(&s.name)).await.ok();
    crate::webhook::dispatch(engine.pool.clone(), "sprint.created", serde_json::json!({"id": s.id, "name": &s.name}));
    engine.notify(ChangeEvent::Sprints);
    Ok((StatusCode::CREATED, Json(s)))
}

#[utoipa::path(get, path = "/api/sprints/{id}", responses((status = 200, body = db::SprintDetail)), security(("bearer" = [])))]
pub async fn get_sprint_detail(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<db::SprintDetail> {
    db::get_sprint(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Sprint not found"))?;
    db::get_sprint_detail(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(put, path = "/api/sprints/{id}", request_body = UpdateSprintRequest, responses((status = 200, body = db::Sprint)), security(("bearer" = [])))]
pub async fn update_sprint(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<UpdateSprintRequest>) -> ApiResult<db::Sprint> {
    let sprint = get_owned_sprint(&engine.pool, id, &claims).await?;
    if req.goal.as_ref().and_then(|o| o.as_ref()).is_some_and(|g| g.len() > 1000) { return Err(err(StatusCode::BAD_REQUEST, "Goal too long (max 1000)")); }
    if let Some(ref name) = req.name { if name.trim().is_empty() { return Err(err(StatusCode::BAD_REQUEST, "Sprint name cannot be empty")); } else if name.len() > 200 { return Err(err(StatusCode::BAD_REQUEST, "Sprint name too long (max 200)")); } }
    if req.project.as_ref().and_then(|o| o.as_ref()).is_some_and(|p| p.len() > 200) { return Err(err(StatusCode::BAD_REQUEST, "Project too long (max 200)")); }
    if req.retro_notes.as_ref().and_then(|o| o.as_ref()).is_some_and(|r| r.len() > 10000) { return Err(err(StatusCode::BAD_REQUEST, "Retro notes too long (max 10000)")); }
    if req.status.is_some() { return Err(err(StatusCode::BAD_REQUEST, "Use /start or /complete endpoints to change sprint status")); }
    if let Some(Some(cap)) = req.capacity_hours { if !(0.0..=10000.0).contains(&cap) { return Err(err(StatusCode::BAD_REQUEST, "capacity_hours must be 0-10000")); } }
    // V1-v23: Validate date format on update (create_sprint validates, update_sprint didn't)
    if let Some(Some(ref d)) = req.start_date { if chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_err() { return Err(err(StatusCode::BAD_REQUEST, "start_date must be YYYY-MM-DD")); } }
    if let Some(Some(ref d)) = req.end_date { if chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_err() { return Err(err(StatusCode::BAD_REQUEST, "end_date must be YYYY-MM-DD")); } }
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
    if let Err(e) = db::audit(&engine.pool, claims.user_id, "delete_sprint", "sprint", Some(id), None).await { tracing::warn!("Audit log failed: {}", e); }
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
    let incomplete: Vec<i64> = tasks.iter().filter(|t| t.status != "completed" && t.status != "done" && t.status != "archived").map(|t| t.id).collect();
    if incomplete.is_empty() { return Err(err(StatusCode::BAD_REQUEST, "No incomplete tasks to carry over")); }
    let base_name = sprint.name.trim_end_matches(" (carry-over)");
    let new_name = format!("{} (carry-over)", base_name);
    let new_sprint = db::create_sprint(&engine.pool, claims.user_id, &new_name, sprint.project.as_deref(), sprint.goal.as_deref(), None, None, sprint.capacity_hours).await.map_err(internal)?;
    // BL6: Filter out tasks already in an active sprint
    let ph = incomplete.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let q = format!("SELECT DISTINCT st.task_id FROM sprint_tasks st JOIN sprints s ON s.id = st.sprint_id WHERE s.status = 'active' AND st.task_id IN ({})", ph);
    let mut query = sqlx::query_as::<_, (i64,)>(&q);
    for tid in &incomplete { query = query.bind(tid); }
    let already_active: std::collections::HashSet<i64> = query.fetch_all(&engine.pool).await.map_err(internal)?.into_iter().map(|(id,)| id).collect();
    let to_add: Vec<i64> = incomplete.into_iter().filter(|id| !already_active.contains(id)).collect();
    if to_add.is_empty() { return Err(err(StatusCode::BAD_REQUEST, "All incomplete tasks are already in an active sprint")); }
    db::add_sprint_tasks(&engine.pool, new_sprint.id, &to_add, claims.user_id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Sprints);
    Ok(Json(new_sprint))
}

#[utoipa::path(get, path = "/api/sprints/{id}/tasks", responses((status = 200, body = Vec<db::Task>)), security(("bearer" = [])))]
pub async fn get_sprint_tasks(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<db::Task>> {
    db::get_sprint(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Sprint not found"))?;
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
    // BL7/BL2: Batch-notify task owners about sprint scope change (one notification per user)
    let sprint_name = db::get_sprint(&engine.pool, id).await.map(|s| s.name.clone()).unwrap_or_default();
    let mut user_tasks: std::collections::HashMap<i64, Vec<String>> = std::collections::HashMap::new();
    for tid in &task_ids {
        if let Ok(task) = db::get_task(&engine.pool, *tid).await {
            if task.user_id != claims.user_id {
                user_tasks.entry(task.user_id).or_default().push(task.title);
            }
        }
    }
    for (uid, titles) in &user_tasks {
        let msg = if titles.len() == 1 {
            format!("Your task '{}' was added to sprint '{}'", titles[0], sprint_name)
        } else {
            format!("{} of your tasks were added to sprint '{}'", titles.len(), sprint_name)
        };
        db::create_notification(&engine.pool, *uid, "task_added_to_sprint", &msg, Some("sprint"), Some(id)).await.ok();
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
    db::remove_sprint_task(&engine.pool, id, task_id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Task not in sprint"))?;
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
    db::get_sprint(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Sprint not found"))?;
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
    let pool = &engine.pool;
    let (sa, sb) = tokio::join!(db::get_sprint(pool, q.a), db::get_sprint(pool, q.b));
    let sa = sa.map_err(|_| err(StatusCode::NOT_FOUND, "Sprint A not found"))?;
    let sb = sb.map_err(|_| err(StatusCode::NOT_FOUND, "Sprint B not found"))?;
    let count_sql = "SELECT COUNT(*), COALESCE(SUM(CASE WHEN t.status IN ('completed','done') THEN 1 ELSE 0 END), 0) FROM sprint_tasks st JOIN tasks t ON st.task_id = t.id WHERE st.sprint_id = ? AND t.deleted_at IS NULL";
    let (ca, cb) = tokio::join!(
        sqlx::query_as::<_, (i64, i64)>(count_sql).bind(q.a).fetch_one(pool),
        sqlx::query_as::<_, (i64, i64)>(count_sql).bind(q.b).fetch_one(pool)
    );
    let (total_a, done_a) = ca.map_err(internal)?;
    let (total_b, done_b) = cb.map_err(internal)?;
    Ok(Json(serde_json::json!({
        "a": {"id": sa.id, "name": sa.name, "total_tasks": total_a, "done_tasks": done_a, "capacity_hours": sa.capacity_hours},
        "b": {"id": sb.id, "name": sb.name, "total_tasks": total_b, "done_tasks": done_b, "capacity_hours": sb.capacity_hours},
    })))
}

#[utoipa::path(get, path = "/api/sprints/velocity", responses((status = 200)), security(("bearer" = [])))]
pub async fn get_velocity(State(engine): State<AppState>, _claims: Claims, Query(q): Query<VelocityQuery>) -> ApiResult<Vec<serde_json::Value>> {
    let n = q.sprints.unwrap_or(10).clamp(1, 50);
    let rows = db::get_velocity(&engine.pool, n).await.map_err(internal)?;
    let result: Vec<serde_json::Value> = rows.into_iter().map(|(name, points, hours, tasks)| {
        serde_json::json!({ "sprint": name, "points": points, "hours": hours, "tasks_done": tasks })
    }).collect();
    Ok(Json(result))
}

// F7: Sprint retrospective analytics
#[utoipa::path(get, path = "/api/sprints/{id}/retro-report", responses((status = 200)), security(("bearer" = [])))]
pub async fn sprint_retro_report(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<serde_json::Value> {
    let sprint = db::get_sprint(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Sprint not found"))?;
    let tasks = db::get_sprint_tasks(&engine.pool, id).await.map_err(internal)?;
    let burns = db::list_burns(&engine.pool, id).await.map_err(internal)?;
    let stats = db::get_sprint_burndown(&engine.pool, id).await.map_err(internal)?;

    let total = tasks.len();
    let done: Vec<_> = tasks.iter().filter(|t| t.status == "completed" || t.status == "done").collect();
    let total_points: f64 = tasks.iter().map(|t| t.remaining_points).sum();
    let done_points: f64 = done.iter().map(|t| t.remaining_points).sum();
    let total_hours: f64 = tasks.iter().map(|t| t.estimated_hours).sum();
    let burned_hours: f64 = burns.iter().filter(|b| b.cancelled == 0).map(|b| b.hours).sum();

    // Per-member contribution
    let mut by_user: std::collections::HashMap<String, (f64, f64, i64)> = std::collections::HashMap::new();
    for b in &burns {
        if b.cancelled != 0 { continue; }
        let e = by_user.entry(b.username.clone()).or_default();
        e.0 += b.points; e.1 += b.hours; e.2 += 1;
    }
    let members: Vec<serde_json::Value> = by_user.into_iter()
        .map(|(u, (p, h, c))| serde_json::json!({"username": u, "points": p, "hours": (h * 100.0).round() / 100.0, "burns": c}))
        .collect();

    // Scope change: count tasks added after sprint start
    let scope_added: i64 = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM sprint_tasks WHERE sprint_id = ? AND added_at > COALESCE((SELECT updated_at FROM sprints WHERE id = ? AND status != 'planning'), '9999')")
        .bind(id).bind(id).fetch_one(&engine.pool).await.map_err(internal)?.0;

    // Carry-over count
    let carried = total - done.len();

    Ok(Json(serde_json::json!({
        "sprint": {"id": sprint.id, "name": sprint.name, "status": sprint.status},
        "tasks": {"total": total, "done": done.len(), "carried": carried},
        "points": {"total": total_points, "done": done_points},
        "hours": {"estimated": total_hours, "burned": (burned_hours * 100.0).round() / 100.0},
        "scope_changes": scope_added,
        "members": members,
        "daily_stats_count": stats.len(),
    })))
}
