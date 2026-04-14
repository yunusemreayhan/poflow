use super::*;


#[utoipa::path(get, path = "/api/rooms", responses((status = 200, body = Vec<db::Room>)), security(("bearer" = [])))]
pub async fn list_rooms(State(engine): State<AppState>, claims: Claims) -> ApiResult<Vec<db::Room>> {
    // B4: Non-root users only see rooms they're a member of — use db function
    if claims.role == "root" {
        db::list_rooms(&engine.pool).await.map(Json).map_err(internal)
    } else {
        db::list_user_rooms(&engine.pool, claims.user_id).await.map(Json).map_err(internal)
    }
}

#[utoipa::path(post, path = "/api/rooms", request_body = CreateRoomRequest, responses((status = 201, body = db::Room)), security(("bearer" = [])))]
pub async fn create_room(State(engine): State<AppState>, claims: Claims, Json(req): Json<CreateRoomRequest>) -> Result<(StatusCode, Json<db::Room>), ApiError> {
    if req.name.trim().is_empty() { return Err(err(StatusCode::BAD_REQUEST, "Room name cannot be empty")); }
    if req.name.len() > 200 { return Err(err(StatusCode::BAD_REQUEST, "Room name too long (max 200 chars)")); }
    let unit = req.estimation_unit.as_deref().unwrap_or("points");
    if !["points", "hours", "mandays", "tshirt"].contains(&unit) { return Err(err(StatusCode::BAD_REQUEST, "estimation_unit must be points, hours, mandays, or tshirt")); }
    let room_type = req.room_type.as_deref().unwrap_or("estimation");
    if room_type != "estimation" { return Err(err(StatusCode::BAD_REQUEST, "room_type must be 'estimation'")); }
    if req.project.as_ref().map(|p| p.len()).unwrap_or(0) > 200 { return Err(err(StatusCode::BAD_REQUEST, "Project name too long (max 200 chars)")); }
    // V2: Limit active rooms per user
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM rooms WHERE creator_id = ? AND status != 'closed'")
        .bind(claims.user_id).fetch_one(&engine.pool).await.map_err(internal)?;
    if count >= 20 { return Err(err(StatusCode::BAD_REQUEST, "Too many active rooms (max 20)")); }
    let r = db::create_room(&engine.pool, &req.name, room_type, unit, req.project.as_deref(), claims.user_id)
        .await.map_err(internal)?;
    engine.notify(ChangeEvent::Rooms);
    Ok((StatusCode::CREATED, Json(r)))
}

#[utoipa::path(get, path = "/api/rooms/{id}", responses((status = 200, body = db::RoomState)), security(("bearer" = [])))]
pub async fn get_room_state(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> ApiResult<db::RoomState> {
    // S2: Verify membership (or root)
    let state = db::get_room_state(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Room not found"))?;
    if !auth::is_admin_or_root(&claims) && !state.members.iter().any(|m| m.username == claims.username) {
        return Err(err(StatusCode::FORBIDDEN, "Not a member of this room"));
    }
    Ok(Json(state))
}

#[utoipa::path(delete, path = "/api/rooms/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_room(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    let room = db::get_room(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Room not found"))?;
    if !is_owner_or_root(room.creator_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    db::delete_room(&engine.pool, id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Rooms);
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/rooms/{id}/join", responses((status = 200)), security(("bearer" = [])))]
pub async fn join_room(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    db::get_room(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Room not found"))?;
    db::join_room(&engine.pool, id, claims.user_id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Rooms);
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/rooms/{id}/leave", responses((status = 200)), security(("bearer" = [])))]
pub async fn leave_room(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    let room = db::get_room(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Room not found"))?;
    if room.creator_id == claims.user_id { return Err(err(StatusCode::BAD_REQUEST, "Room creator cannot leave — delete the room instead")); }
    db::leave_room(&engine.pool, id, claims.user_id).await.map_err(|_| err(StatusCode::BAD_REQUEST, "Not a member of this room"))?;
    engine.notify(ChangeEvent::Rooms);
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/api/rooms/{id}/members/{username}", responses((status = 204)), security(("bearer" = [])))]
pub async fn kick_member(State(engine): State<AppState>, claims: Claims, Path((id, username)): Path<(i64, String)>) -> Result<StatusCode, ApiError> {
    if !db::is_room_admin(&engine.pool, id, claims.user_id).await.map_err(internal)? && !auth::is_admin_or_root(&claims) {
        return Err(err(StatusCode::FORBIDDEN, "Admin only"));
    }
    let uid = db::get_user_id_by_username(&engine.pool, &username).await.map_err(internal)?.ok_or_else(|| err(StatusCode::NOT_FOUND, "User not found"))?;
    if uid == claims.user_id { return Err(err(StatusCode::BAD_REQUEST, "Cannot kick yourself")); }
    db::leave_room(&engine.pool, id, uid).await.map_err(|_| err(StatusCode::NOT_FOUND, "User is not a member of this room"))?;
    engine.notify(crate::engine::ChangeEvent::Rooms);
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(put, path = "/api/rooms/{id}/role", request_body = RoomRoleRequest, responses((status = 200)), security(("bearer" = [])))]
pub async fn set_room_role(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<RoomRoleRequest>) -> Result<StatusCode, ApiError> {
    if !db::is_room_admin(&engine.pool, id, claims.user_id).await.map_err(internal)? && !auth::is_admin_or_root(&claims) {
        return Err(err(StatusCode::FORBIDDEN, "Admin only"));
    }
    if !VALID_ROOM_ROLES.contains(&req.role.as_str()) { return Err(err(StatusCode::BAD_REQUEST, format!("Invalid room role '{}'. Must be one of: {}", req.role, VALID_ROOM_ROLES.join(", ")))); }
    let uid = db::get_user_id_by_username(&engine.pool, &req.username).await.map_err(internal)?.ok_or_else(|| err(StatusCode::NOT_FOUND, "User not found"))?;
    db::set_room_member_role(&engine.pool, id, uid, &req.role).await.map_err(|_| err(StatusCode::NOT_FOUND, "User is not a member of this room"))?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/rooms/{id}/start-voting", request_body = StartVotingRequest, responses((status = 200, body = db::Room)), security(("bearer" = [])))]
pub async fn start_voting(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<StartVotingRequest>) -> ApiResult<db::Room> {
    if !db::is_room_admin(&engine.pool, id, claims.user_id).await.map_err(internal)? && !auth::is_admin_or_root(&claims) {
        return Err(err(StatusCode::FORBIDDEN, "Admin only"));
    }
    // V8: Verify task exists and is not soft-deleted
    let task = db::get_task(&engine.pool, req.task_id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    if task.deleted_at.is_some() { return Err(err(StatusCode::BAD_REQUEST, "Cannot vote on a deleted task")); }
    db::start_voting(&engine.pool, id, req.task_id).await.map(|r| { engine.notify(ChangeEvent::Rooms); Json(r) }).map_err(internal)
}

#[utoipa::path(post, path = "/api/rooms/{id}/vote", request_body = CastVoteRequest, responses((status = 200)), security(("bearer" = [])))]
pub async fn cast_vote(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<CastVoteRequest>) -> Result<StatusCode, ApiError> {
    if req.value < 0.0 || req.value > 1000.0 { return Err(err(StatusCode::BAD_REQUEST, "Vote value must be 0-1000")); }
    let room = db::get_room(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Room not found"))?;
    if room.status != "voting" { return Err(err(StatusCode::BAD_REQUEST, "Room is not in voting state")); }
    // Verify user is a room member
    let members = db::get_room_members(&engine.pool, id).await.map_err(internal)?;
    if !members.iter().any(|m| m.user_id == claims.user_id) && !auth::is_admin_or_root(&claims) {
        return Err(err(StatusCode::FORBIDDEN, "Not a room member"));
    }
    // V3: Observers cannot vote
    if members.iter().any(|m| m.user_id == claims.user_id && m.role == "observer") {
        return Err(err(StatusCode::FORBIDDEN, "Observers cannot vote"));
    }
    let task_id = room.current_task_id.ok_or_else(|| err(StatusCode::BAD_REQUEST, "No active vote"))?;
    db::cast_vote(&engine.pool, id, task_id, claims.user_id, req.value).await.map_err(internal)?;
    engine.notify(ChangeEvent::Rooms);
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/rooms/{id}/reveal", responses((status = 200, body = db::Room)), security(("bearer" = [])))]
pub async fn reveal_votes(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> ApiResult<db::Room> {
    if !db::is_room_admin(&engine.pool, id, claims.user_id).await.map_err(internal)? && !auth::is_admin_or_root(&claims) {
        return Err(err(StatusCode::FORBIDDEN, "Admin only"));
    }
    let r = db::reveal_votes(&engine.pool, id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Rooms);
    Ok(Json(r))
}

#[utoipa::path(post, path = "/api/rooms/{id}/accept", request_body = AcceptEstimateRequest, responses((status = 200, body = db::Task)), security(("bearer" = [])))]
pub async fn accept_estimate(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<AcceptEstimateRequest>) -> ApiResult<db::Task> {
    if !db::is_room_admin(&engine.pool, id, claims.user_id).await.map_err(internal)? && !auth::is_admin_or_root(&claims) {
        return Err(err(StatusCode::FORBIDDEN, "Admin only"));
    }
    if req.value < 0.0 || req.value > 10000.0 { return Err(err(StatusCode::BAD_REQUEST, "Estimate value must be 0-10000")); }
    let room = db::get_room(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Room not found"))?;
    if room.status != "revealed" { return Err(err(StatusCode::BAD_REQUEST, "Votes must be revealed before accepting")); }
    let task_id = room.current_task_id.ok_or_else(|| err(StatusCode::BAD_REQUEST, "No active vote"))?;
    let task = db::accept_estimate(&engine.pool, id, task_id, req.value, &room.estimation_unit).await.map_err(internal)?;
    // Auto-advance: find next unestimated leaf task (O(n) with pre-computed set)
    let state = db::get_room_state(&engine.pool, id).await.map_err(internal)?;
    let all_tasks = &state.tasks;
    let voted_task_ids: std::collections::HashSet<i64> = state.vote_history.iter().map(|v| v.task_id).collect();
    // B9: Build has_children only from tasks within the room scope
    let room_task_ids: std::collections::HashSet<i64> = all_tasks.iter().map(|t| t.id).collect();
    let has_children: std::collections::HashSet<i64> = all_tasks.iter().filter_map(|t| t.parent_id).filter(|pid| room_task_ids.contains(pid)).collect();
    let next = all_tasks.iter()
        .filter(|t| t.id != task_id && t.status != "estimated" && !voted_task_ids.contains(&t.id))
        .filter(|t| !has_children.contains(&t.id)) // leaf only
        .min_by_key(|t| (t.sort_order, t.id)); // BL8: deterministic ordering
    if let Some(next_task) = next { db::start_voting(&engine.pool, id, next_task.id).await.map_err(internal)?; }
    else { db::set_room_status(&engine.pool, id, "lobby").await.map_err(internal)?; }
    engine.notify(ChangeEvent::Rooms);
    engine.notify(ChangeEvent::Tasks);
    Ok(Json(task))
}

#[utoipa::path(post, path = "/api/rooms/{id}/close", responses((status = 200)), security(("bearer" = [])))]
pub async fn close_room(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    if !db::is_room_admin(&engine.pool, id, claims.user_id).await.map_err(internal)? && !auth::is_admin_or_root(&claims) {
        return Err(err(StatusCode::FORBIDDEN, "Admin only"));
    }
    db::set_room_status(&engine.pool, id, "closed").await.map_err(internal)?;
    engine.notify(ChangeEvent::Rooms);
    Ok(StatusCode::NO_CONTENT)
}


// F8: Export room estimation history as JSON
#[utoipa::path(get, path = "/api/rooms/{id}/export", responses((status = 200)), security(("bearer" = [])))]
pub async fn export_room_history(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<axum::response::Response, ApiError> {
    // B4: Verify room membership
    let members = db::get_room_members(&engine.pool, id).await.map_err(internal)?;
    if !members.iter().any(|m| m.user_id == claims.user_id) && !auth::is_admin_or_root(&claims) {
        return Err(err(StatusCode::FORBIDDEN, "Not a room member"));
    }
    let state = db::get_room_state(&engine.pool, id).await.map_err(internal)?;
    let body = serde_json::to_vec(&state.vote_history).map_err(internal)?;
    Ok(axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .header("content-disposition", format!("attachment; filename=\"room_{}_history.json\"", id))
        .body(axum::body::Body::from(body)).map_err(|e| internal(e.to_string()))?)
}

// --- Sprints ---

pub async fn room_ws(
    State(engine): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<super::misc::SseQuery>,
    ws: axum::extract::WebSocketUpgrade,
) -> Result<axum::response::Response, ApiError> {
    // Auth via ticket or token
    let _user_id = if let Some(ticket) = &q.ticket {
        let mut tickets = super::misc::sse_tickets().lock().await;
        let (uid, created) = tickets.remove(ticket.as_str())
            .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "Invalid ticket"))?;
        if std::time::Instant::now().duration_since(created).as_secs() > 30 {
            return Err(err(StatusCode::UNAUTHORIZED, "Ticket expired"));
        }
        uid
    } else {
        return Err(err(StatusCode::UNAUTHORIZED, "Ticket required"));
    };

    let room_id = id;
    let user_id = _user_id;

    // S4: Verify user is a room member before allowing WebSocket connection
    let members = db::get_room_members(&engine.pool, room_id).await.map_err(internal)?;
    if !members.iter().any(|m| m.user_id == user_id) {
        return Err(err(StatusCode::FORBIDDEN, "Not a room member"));
    }

    let mut change_rx = engine.changes.subscribe();
    let pool = engine.pool.clone();

    Ok(ws.on_upgrade(move |mut socket| async move {
        use axum::extract::ws::Message;
        // Send initial state
        let mut last_json = String::new();
        if let Ok(state) = db::get_room_state(&pool, room_id).await {
            if let Ok(json) = serde_json::to_string(&state) {
                last_json = json.clone();
                let _ = socket.send(Message::Text(json.into())).await;
            }
        }
        loop {
            tokio::select! {
                Ok(evt) = change_rx.recv() => {
                    if matches!(evt, ChangeEvent::Rooms) {
                        // BL3: Re-verify membership on each state push
                        let still_member = db::get_room_members(&pool, room_id).await
                            .map(|m| m.iter().any(|m| m.user_id == user_id)).unwrap_or(false);
                        if !still_member { let _ = socket.send(Message::Close(None)).await; break; }
                        if let Ok(state) = db::get_room_state(&pool, room_id).await {
                            if let Ok(json) = serde_json::to_string(&state) {
                                // P2: Skip sending if state unchanged
                                if json != last_json {
                                    last_json = json.clone();
                                    if socket.send(Message::Text(json.into())).await.is_err() { break; }
                                }
                            }
                        }
                    }
                }
                msg = socket.recv() => {
                    match msg {
                        Some(Ok(Message::Close(_))) | None => break,
                        _ => {}
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {
                    if socket.send(Message::Ping(vec![].into())).await.is_err() { break; }
                }
            }
        }
    }))
}
