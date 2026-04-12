use super::*;
use std::collections::HashMap;

#[utoipa::path(get, path = "/api/health", responses((status = 200)))]
pub async fn health(State(engine): State<AppState>) -> Json<serde_json::Value> {
    let db_ok = sqlx::query("SELECT 1").execute(&engine.pool).await.is_ok();
    // O1: Report schema migration version
    let migration_version: i64 = sqlx::query_as::<_, (i64,)>("SELECT COALESCE(MAX(version), 0) FROM schema_migrations")
        .fetch_one(&engine.pool).await.map(|r| r.0).unwrap_or(0);
    let active_timers = engine.states.lock().await.values().filter(|s| s.status == crate::engine::TimerStatus::Running).count();
    // O2: Background task health
    let heartbeats = engine.heartbeats.lock().await;
    let tasks: serde_json::Map<String, serde_json::Value> = heartbeats.iter().map(|(name, last)| {
        let secs_ago = last.elapsed().as_secs();
        // O1: Use 2x expected interval as health threshold
        let max_age = match name.as_str() {
            "tick" => 10, "snapshot" => 7200, "auto_archive" => 172800,
            _ => 600,
        };
        (name.clone(), serde_json::json!({ "last_heartbeat_secs_ago": secs_ago, "healthy": secs_ago < max_age }))
    }).collect();
    drop(heartbeats);
    // O2: Database size
    let db_size = std::fs::metadata(db::db_path()).map(|m| m.len()).unwrap_or(0);
    Json(serde_json::json!({ "status": if db_ok { "ok" } else { "degraded" }, "db": db_ok, "db_size_bytes": db_size, "schema_version": migration_version, "active_timers": active_timers, "background_tasks": tasks }))
}

#[utoipa::path(get, path = "/api/tasks/{id}/votes", responses((status = 200, body = Vec<db::RoomVote>)), security(("bearer" = [])))]
pub async fn get_task_votes(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<db::RoomVote>> {
    db::get_task_votes(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(get, path = "/api/task-sprints", responses((status = 200, body = Vec<db::TaskSprintInfo>)), security(("bearer" = [])))]
pub async fn get_task_sprints(State(engine): State<AppState>, _claims: Claims) -> ApiResult<Vec<db::TaskSprintInfo>> {
    db::get_all_task_sprints(&engine.pool).await.map(Json).map_err(internal)
}

#[utoipa::path(get, path = "/api/users", responses((status = 200, body = Vec<String>)), security(("bearer" = [])))]
pub async fn list_usernames(State(engine): State<AppState>, _claims: Claims) -> ApiResult<Vec<String>> {
    db::list_usernames(&engine.pool).await.map(Json).map_err(internal)
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct BurnTotalEntry { pub task_id: i64, pub total_points: f64, pub total_hours: f64, pub count: i64 }

#[utoipa::path(get, path = "/api/burn-totals", responses((status = 200)), security(("bearer" = [])))]
pub async fn get_all_burn_totals(State(engine): State<AppState>, _claims: Claims) -> ApiResult<Vec<BurnTotalEntry>> {
    let totals = db::get_all_burn_totals(&engine.pool).await.map_err(internal)?;
    Ok(Json(totals.into_iter().map(|(tid, bt)| BurnTotalEntry { task_id: tid, total_points: bt.total_points, total_hours: bt.total_hours, count: bt.count }).collect()))
}

#[utoipa::path(get, path = "/api/assignees", responses((status = 200, body = Vec<db::TaskAssignee>)), security(("bearer" = [])))]
pub async fn get_all_assignees(State(engine): State<AppState>, _claims: Claims) -> ApiResult<Vec<db::TaskAssignee>> {
    db::get_all_assignees(&engine.pool).await.map(Json).map_err(internal)
}

#[derive(Serialize)]
pub struct TasksFullResponse {
    pub tasks: Vec<db::Task>,
    pub task_sprints: Vec<db::TaskSprintInfo>,
    pub burn_totals: Vec<BurnTotalEntry>,
    pub assignees: Vec<db::TaskAssignee>,
    pub labels: Vec<db::TaskLabel>,
}

#[utoipa::path(get, path = "/api/tasks/full", responses((status = 200, body = Vec<db::Task>)), security(("bearer" = [])))]
pub async fn get_tasks_full(State(engine): State<AppState>, _claims: Claims, headers: axum::http::HeaderMap) -> Result<axum::response::Response, ApiError> {
    // B8: ETag includes labels and attachments to avoid stale data
    let (max_updated, task_count, sprint_task_count, burn_count, assignee_count, label_count, att_count): (String, i64, i64, i64, i64, i64, i64) =
        sqlx::query_as("SELECT COALESCE((SELECT MAX(updated_at) FROM tasks), ''), (SELECT COUNT(*) FROM tasks), (SELECT COUNT(*) FROM sprint_tasks), (SELECT COUNT(*) FROM burn_log WHERE cancelled = 0), (SELECT COUNT(*) FROM task_assignees), (SELECT COUNT(*) FROM task_labels), (SELECT COUNT(*) FROM task_attachments)")
        .fetch_one(&engine.pool).await.map_err(internal)?;
    let etag = format!("\"{}:{}:{}:{}:{}:{}:{}\"", max_updated, task_count, sprint_task_count, burn_count, assignee_count, label_count, att_count);

    if let Some(if_none_match) = headers.get("if-none-match").and_then(|v| v.to_str().ok()) {
        if if_none_match == etag {
            return Ok(axum::response::Response::builder()
                .status(StatusCode::NOT_MODIFIED)
                .header("etag", &etag)
                .body(axum::body::Body::empty()).map_err(|e| internal(e.to_string()))?);
        }
    }

    let (tasks, task_sprints, burn_totals_raw, assignees, labels) = tokio::join!(
        db::list_tasks(&engine.pool, None, None),
        db::get_all_task_sprints(&engine.pool),
        db::get_all_burn_totals(&engine.pool),
        db::get_all_assignees(&engine.pool),
        db::get_all_task_labels(&engine.pool),
    );
    let burn_totals: Vec<BurnTotalEntry> = burn_totals_raw.map_err(internal)?.into_iter()
        .map(|(tid, bt)| BurnTotalEntry { task_id: tid, total_points: bt.total_points, total_hours: bt.total_hours, count: bt.count })
        .collect();
    let resp = TasksFullResponse {
        tasks: tasks.map_err(internal)?,
        task_sprints: task_sprints.map_err(internal)?,
        burn_totals,
        assignees: assignees.map_err(internal)?,
        labels: labels.map_err(internal)?,
    };
    let body = serde_json::to_vec(&resp).map_err(internal)?;
    Ok(axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .header("etag", &etag)
        .body(axum::body::Body::from(body)).map_err(|e| internal(e.to_string()))?)
}

#[derive(Deserialize)]
pub struct SseQuery { pub token: Option<String>, pub ticket: Option<String> }

// Short-lived opaque tickets for SSE (avoids JWT in query string / logs)
static SSE_TICKETS: std::sync::OnceLock<tokio::sync::Mutex<HashMap<String, (i64, std::time::Instant)>>> = std::sync::OnceLock::new();
pub(crate) fn sse_tickets() -> &'static tokio::sync::Mutex<HashMap<String, (i64, std::time::Instant)>> {
    SSE_TICKETS.get_or_init(|| tokio::sync::Mutex::new(HashMap::new()))
}

#[utoipa::path(post, path = "/api/timer/ticket", responses((status = 200)), security(("bearer" = [])))]
pub async fn create_sse_ticket(claims: Claims) -> ApiResult<serde_json::Value> {
    // Use /dev/urandom for cryptographic randomness, fallback to hash-based
    let ticket = {
        let mut buf = [0u8; 24];
        let got = (|| -> Result<(), std::io::Error> {
            use std::io::Read;
            let mut f = std::fs::File::open("/dev/urandom")?;
            f.read_exact(&mut buf)?;
            Ok(())
        })().is_ok();
        if got {
            buf.iter().map(|b| format!("{:02x}", b)).collect::<String>()
        } else {
            use sha2::{Sha256, Digest};
            static CTR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let seed = format!("sse{}{}{}",
                chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
                claims.user_id,
                CTR.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
            let hash = Sha256::digest(seed.as_bytes());
            hash.iter().map(|b| format!("{:02x}", b)).collect::<String>()
        }
    };
    let mut tickets = sse_tickets().lock().await;
    let now = std::time::Instant::now();
    // Only clean up when map grows large to avoid O(n) on every creation
    if tickets.len() > 50 {
        tickets.retain(|_, (_, t)| now.duration_since(*t).as_secs() < 30);
    }
    // S1: Limit active tickets per user to prevent ticket pool exhaustion
    let user_tickets = tickets.values().filter(|(uid, t)| *uid == claims.user_id && now.duration_since(*t).as_secs() < 30).count();
    if user_tickets >= 5 { return Err(err(StatusCode::TOO_MANY_REQUESTS, "Too many active tickets")); }
    tickets.insert(ticket.clone(), (claims.user_id, now));
    Ok(Json(serde_json::json!({ "ticket": ticket })))
}

pub async fn sse_timer(State(engine): State<AppState>, Query(q): Query<SseQuery>) -> Result<axum::response::Sse<impl futures::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>>, ApiError> {
    let user_id = if let Some(ticket) = &q.ticket {
        let mut tickets = sse_tickets().lock().await;
        let (uid, created) = tickets.remove(ticket.as_str())
            .ok_or_else(|| err(StatusCode::UNAUTHORIZED, "Invalid or expired ticket"))?;
        if std::time::Instant::now().duration_since(created).as_secs() > 30 {
            return Err(err(StatusCode::UNAUTHORIZED, "Ticket expired"));
        }
        uid
    } else {
        return Err(err(StatusCode::UNAUTHORIZED, "Ticket required — use POST /api/timer/ticket first"));
    };
    let mut timer_rx = engine.tx.subscribe();
    let mut change_rx = engine.changes.subscribe();
    // Send initial state for this user
    let initial = engine.get_state(user_id).await;
    let stream = async_stream::stream! {
        if let Ok(json) = serde_json::to_string(&initial) {
            yield Ok(axum::response::sse::Event::default().event("timer").data(json));
        }
        loop {
            tokio::select! {
                Ok(()) = timer_rx.changed() => {
                    let state = timer_rx.borrow().clone();
                    // B5: Only send events for this user (removed == 0 check that leaked idle states)
                    if state.current_user_id == user_id {
                        // P3: Use in-memory state directly instead of re-fetching from DB on every tick
                        if let Ok(json) = serde_json::to_string(&state) {
                            yield Ok(axum::response::sse::Event::default().event("timer").data(json));
                        }
                    }
                }
                Ok(evt) = change_rx.recv() => {
                    if let Ok(json) = serde_json::to_string(&evt) {
                        yield Ok(axum::response::sse::Event::default().event("change").data(json));
                    }
                }
                else => break,
            }
        }
    };
    Ok(axum::response::Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default()))
}


// --- Rooms ---
