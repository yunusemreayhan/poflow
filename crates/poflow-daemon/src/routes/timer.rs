use super::*;

#[utoipa::path(get, path = "/api/timer", responses((status = 200, body = crate::engine::EngineState)), security(("bearer" = [])))]
pub async fn get_state(
    State(engine): State<AppState>,
    claims: Claims,
) -> ApiResult<crate::engine::EngineState> {
    Ok(Json(engine.get_state(claims.user_id).await))
}

// F12: List active timers for all users (team visibility)
// B1: Collect state snapshot under lock, then query DB without holding mutex
#[utoipa::path(get, path = "/api/timer/active", responses((status = 200)), security(("bearer" = [])))]
pub async fn get_active_timers(
    State(engine): State<AppState>,
    _claims: Claims,
) -> ApiResult<Vec<serde_json::Value>> {
    let snapshot: Vec<_> = {
        let states = engine.states.lock().await;
        states
            .iter()
            .filter(|(_, s)| s.status != crate::engine::TimerStatus::Idle)
            .map(|(uid, s)| {
                (
                    *uid,
                    s.phase,
                    s.status,
                    s.current_task_id,
                    s.elapsed_s,
                    s.duration_s,
                )
            })
            .collect()
    }; // lock dropped here
    let mut active = Vec::new();
    if snapshot.is_empty() {
        return Ok(Json(active));
    }
    // Batch lookup users and tasks
    let user_ids: Vec<i64> = snapshot.iter().map(|(uid, ..)| *uid).collect();
    let task_ids: Vec<i64> = snapshot
        .iter()
        .filter_map(|(_, _, _, tid, ..)| *tid)
        .collect();
    let uph = user_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let usql = format!("SELECT id, username FROM users WHERE id IN ({})", uph);
    let mut uq = sqlx::query_as::<_, (i64, String)>(&usql);
    for id in &user_ids {
        uq = uq.bind(id);
    }
    let user_map: std::collections::HashMap<i64, String> = uq
        .fetch_all(&engine.pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();
    let task_map: std::collections::HashMap<i64, String> = if !task_ids.is_empty() {
        let tph = task_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let tsql = format!("SELECT id, title FROM tasks WHERE id IN ({})", tph);
        let mut tq = sqlx::query_as::<_, (i64, String)>(&tsql);
        for id in &task_ids {
            tq = tq.bind(id);
        }
        tq.fetch_all(&engine.pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect()
    } else {
        std::collections::HashMap::new()
    };
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
pub async fn start(
    State(engine): State<AppState>,
    claims: Claims,
    Json(req): Json<StartRequest>,
) -> ApiResult<crate::engine::EngineState> {
    let phase = req.phase.as_deref().map(|s| match s {
        "short_break" => TimerPhase::ShortBreak,
        "long_break" => TimerPhase::LongBreak,
        _ => TimerPhase::Work,
    });
    engine
        .start(claims.user_id, req.task_id, phase)
        .await
        .map(Json)
        .map_err(internal)
}

#[utoipa::path(post, path = "/api/timer/pause", responses((status = 200, body = crate::engine::EngineState)), security(("bearer" = [])))]
pub async fn pause(
    State(engine): State<AppState>,
    claims: Claims,
) -> ApiResult<crate::engine::EngineState> {
    engine
        .pause(claims.user_id)
        .await
        .map(Json)
        .map_err(internal)
}
#[utoipa::path(post, path = "/api/timer/resume", responses((status = 200, body = crate::engine::EngineState)), security(("bearer" = [])))]
pub async fn resume(
    State(engine): State<AppState>,
    claims: Claims,
) -> ApiResult<crate::engine::EngineState> {
    engine
        .resume(claims.user_id)
        .await
        .map(Json)
        .map_err(internal)
}
#[utoipa::path(post, path = "/api/timer/stop", responses((status = 200, body = crate::engine::EngineState)), security(("bearer" = [])))]
pub async fn stop(
    State(engine): State<AppState>,
    claims: Claims,
) -> ApiResult<crate::engine::EngineState> {
    engine
        .stop(claims.user_id)
        .await
        .map(Json)
        .map_err(internal)
}
#[utoipa::path(post, path = "/api/timer/skip", responses((status = 200, body = crate::engine::EngineState)), security(("bearer" = [])))]
pub async fn skip(
    State(engine): State<AppState>,
    claims: Claims,
) -> ApiResult<crate::engine::EngineState> {
    engine
        .skip(claims.user_id)
        .await
        .map(Json)
        .map_err(internal)
}

// --- Tasks ---

// F11: Shared timer sessions — join another user's active session
#[utoipa::path(post, path = "/api/timer/join/{session_id}", responses((status = 200)), security(("bearer" = [])))]
pub async fn join_session(
    State(engine): State<AppState>,
    claims: Claims,
    Path(session_id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    // Verify session exists and is active
    let session: (String, i64) =
        sqlx::query_as("SELECT status, user_id FROM sessions WHERE id = ?")
            .bind(session_id)
            .fetch_one(&engine.pool)
            .await
            .map_err(|_| err(StatusCode::NOT_FOUND, "Session not found"))?;
    if session.0 != "running" {
        return Err(err(StatusCode::BAD_REQUEST, "Session is not active"));
    }
    // PF15: Prevent joining own session
    if session.1 == claims.user_id {
        return Err(err(StatusCode::BAD_REQUEST, "Cannot join your own session"));
    }
    sqlx::query("INSERT OR IGNORE INTO session_participants (session_id, user_id, joined_at) VALUES (?, ?, ?)")
        .bind(session_id).bind(claims.user_id).bind(db::now_str())
        .execute(&engine.pool).await.map_err(internal)?;
    Ok(StatusCode::OK)
}

#[utoipa::path(get, path = "/api/timer/participants/{session_id}", responses((status = 200)), security(("bearer" = [])))]
pub async fn session_participants(
    State(engine): State<AppState>,
    _claims: Claims,
    Path(session_id): Path<i64>,
) -> ApiResult<Vec<serde_json::Value>> {
    // V29-5: Verify session exists
    sqlx::query("SELECT 1 FROM sessions WHERE id = ?")
        .bind(session_id)
        .fetch_one(&engine.pool)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Session not found"))?;
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT u.username, sp.joined_at FROM session_participants sp JOIN users u ON sp.user_id = u.id WHERE sp.session_id = ?")
        .bind(session_id).fetch_all(&engine.pool).await.map_err(internal)?;
    Ok(Json(
        rows.into_iter()
            .map(|(u, j)| serde_json::json!({"username": u, "joined_at": j}))
            .collect(),
    ))
}
