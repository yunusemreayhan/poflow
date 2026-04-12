use super::*;


#[utoipa::path(get, path = "/api/timer", responses((status = 200, body = crate::engine::EngineState)), security(("bearer" = [])))]
pub async fn get_state(State(engine): State<AppState>, claims: Claims) -> ApiResult<crate::engine::EngineState> {
    Ok(Json(engine.get_state(claims.user_id).await))
}

// F12: List active timers for all users (team visibility)
// B1: Collect state snapshot under lock, then query DB without holding mutex
#[utoipa::path(get, path = "/api/timer/active", responses((status = 200)), security(("bearer" = [])))]
pub async fn get_active_timers(State(engine): State<AppState>, _claims: Claims) -> ApiResult<Vec<serde_json::Value>> {
    let snapshot: Vec<_> = {
        let states = engine.states.lock().await;
        states.iter().filter(|(_, s)| s.status != crate::engine::TimerStatus::Idle)
            .map(|(uid, s)| (*uid, s.phase, s.status, s.current_task_id, s.elapsed_s, s.duration_s))
            .collect()
    }; // lock dropped here
    let mut active = Vec::new();
    if snapshot.is_empty() { return Ok(Json(active)); }
    // Batch lookup users and tasks
    let user_ids: Vec<i64> = snapshot.iter().map(|(uid, ..)| *uid).collect();
    let task_ids: Vec<i64> = snapshot.iter().filter_map(|(_, _, _, tid, ..)| *tid).collect();
    let uph = user_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let usql = format!("SELECT id, username FROM users WHERE id IN ({})", uph);
    let mut uq = sqlx::query_as::<_, (i64, String)>(&usql);
    for id in &user_ids { uq = uq.bind(id); }
    let user_map: std::collections::HashMap<i64, String> = uq.fetch_all(&engine.pool).await.unwrap_or_default().into_iter().collect();
    let task_map: std::collections::HashMap<i64, String> = if !task_ids.is_empty() {
        let tph = task_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let tsql = format!("SELECT id, title FROM tasks WHERE id IN ({})", tph);
        let mut tq = sqlx::query_as::<_, (i64, String)>(&tsql);
        for id in &task_ids { tq = tq.bind(id); }
        tq.fetch_all(&engine.pool).await.unwrap_or_default().into_iter().collect()
    } else { std::collections::HashMap::new() };
    for (uid, phase, status, task_id, elapsed_s, duration_s) in &snapshot {
        active.push(serde_json::json!({
            "username": user_map.get(uid).cloned().unwrap_or_default(),
            "phase": phase, "status": status,
            "task": task_id.and_then(|tid| task_map.get(&tid).cloned()),
            "elapsed_s": elapsed_s, "duration_s": duration_s,
        }));
    }
    Ok(Json(active))
}

#[utoipa::path(post, path = "/api/timer/start", request_body = StartRequest, responses((status = 200, body = crate::engine::EngineState)), security(("bearer" = [])))]
pub async fn start(State(engine): State<AppState>, claims: Claims, Json(req): Json<StartRequest>) -> ApiResult<crate::engine::EngineState> {
    let phase = req.phase.as_deref().map(|s| match s { "short_break" => TimerPhase::ShortBreak, "long_break" => TimerPhase::LongBreak, _ => TimerPhase::Work });
    engine.start(claims.user_id, req.task_id, phase).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/timer/pause", responses((status = 200, body = crate::engine::EngineState)), security(("bearer" = [])))]
pub async fn pause(State(engine): State<AppState>, claims: Claims) -> ApiResult<crate::engine::EngineState> {
    engine.pause(claims.user_id).await.map(Json).map_err(internal)
}
#[utoipa::path(post, path = "/api/timer/resume", responses((status = 200, body = crate::engine::EngineState)), security(("bearer" = [])))]
pub async fn resume(State(engine): State<AppState>, claims: Claims) -> ApiResult<crate::engine::EngineState> {
    engine.resume(claims.user_id).await.map(Json).map_err(internal)
}
#[utoipa::path(post, path = "/api/timer/stop", responses((status = 200, body = crate::engine::EngineState)), security(("bearer" = [])))]
pub async fn stop(State(engine): State<AppState>, claims: Claims) -> ApiResult<crate::engine::EngineState> {
    engine.stop(claims.user_id).await.map(Json).map_err(internal)
}
#[utoipa::path(post, path = "/api/timer/skip", responses((status = 200, body = crate::engine::EngineState)), security(("bearer" = [])))]
pub async fn skip(State(engine): State<AppState>, claims: Claims) -> ApiResult<crate::engine::EngineState> {
    engine.skip(claims.user_id).await.map(Json).map_err(internal)
}


// --- Tasks ---
