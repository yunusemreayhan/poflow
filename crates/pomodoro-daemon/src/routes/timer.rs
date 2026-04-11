use super::*;


#[utoipa::path(get, path = "/api/timer", responses((status = 200, body = crate::engine::EngineState)), security(("bearer" = [])))]
pub async fn get_state(State(engine): State<AppState>, claims: Claims) -> ApiResult<crate::engine::EngineState> {
    Ok(Json(engine.get_state(claims.user_id).await))
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
