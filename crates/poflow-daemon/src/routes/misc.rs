use super::*;
use std::collections::HashMap;

#[utoipa::path(get, path = "/api/health", responses((status = 200)))]
pub async fn health(State(engine): State<AppState>) -> Json<serde_json::Value> {
    let db_ok = sqlx::query("SELECT 1").execute(&engine.pool).await.is_ok();
    let (user_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users").fetch_one(&engine.pool).await.unwrap_or((0,));
    let (task_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tasks WHERE deleted_at IS NULL").fetch_one(&engine.pool).await.unwrap_or((0,));
    Json(serde_json::json!({
        "status": if db_ok { "ok" } else { "degraded" },
        "db": db_ok,
        "version": env!("CARGO_PKG_VERSION"),
        "users": user_count,
        "tasks": task_count,
    }))
}

#[utoipa::path(get, path = "/api/tasks/{id}/votes", responses((status = 200, body = Vec<db::RoomVote>)), security(("bearer" = [])))]
pub async fn get_task_votes(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<db::RoomVote>> {
    db::get_task(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    db::get_task_votes(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(get, path = "/api/task-sprints", responses((status = 200, body = Vec<db::TaskSprintInfo>)), security(("bearer" = [])))]
pub async fn get_task_sprints(State(engine): State<AppState>, _claims: Claims) -> ApiResult<Vec<db::TaskSprintInfo>> {
    db::get_all_task_sprints(&engine.pool).await.map(Json).map_err(internal)
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct UserEntry { pub id: i64, pub username: String }

#[utoipa::path(get, path = "/api/users", responses((status = 200, body = Vec<UserEntry>)), security(("bearer" = [])))]
pub async fn list_usernames(State(engine): State<AppState>, _claims: Claims) -> ApiResult<Vec<UserEntry>> {
    let rows = db::list_usernames(&engine.pool).await.map_err(internal)?;
    Ok(Json(rows.into_iter().map(|(id, username)| UserEntry { id, username }).collect()))
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

#[derive(Serialize, utoipa::ToSchema)]
pub struct TasksFullResponse {
    pub tasks: Vec<db::Task>,
    pub task_sprints: Vec<db::TaskSprintInfo>,
    pub burn_totals: Vec<BurnTotalEntry>,
    pub assignees: Vec<db::TaskAssignee>,
    pub labels: Vec<db::TaskLabel>,
}

#[derive(Deserialize)]
pub struct TasksFullQuery {
    pub project: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[utoipa::path(get, path = "/api/tasks/full", responses((status = 200, body = TasksFullResponse)), security(("bearer" = [])))]
pub async fn get_tasks_full(State(engine): State<AppState>, _claims: Claims, headers: axum::http::HeaderMap, Query(q): Query<TasksFullQuery>) -> Result<axum::response::Response, ApiError> {
    let project_filter = q.project.as_deref();
    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(5000).clamp(1, 5000);
    let offset = (page - 1) * per_page;

    // B8: ETag includes labels and attachments to avoid stale data
    let (max_updated, task_count, sprint_task_count, burn_count, assignee_count, label_count, att_count): (String, i64, i64, i64, i64, i64, i64) =
        sqlx::query_as("SELECT COALESCE((SELECT MAX(COALESCE(deleted_at, updated_at)) FROM tasks), ''), (SELECT COUNT(*) FROM tasks WHERE deleted_at IS NULL), (SELECT COUNT(*) FROM sprint_tasks), (SELECT COUNT(*) FROM burn_log WHERE cancelled = 0), (SELECT COUNT(*) FROM task_assignees), (SELECT COUNT(*) FROM task_labels), (SELECT COUNT(*) FROM task_attachments)")
        .fetch_one(&engine.pool).await.map_err(internal)?;
    let etag_base = format!("{}:{}:{}:{}:{}:{}:{}", max_updated, task_count, sprint_task_count, burn_count, assignee_count, label_count, att_count);
    let etag = format!("\"{}:{}:{}:{}\"", etag_base, project_filter.unwrap_or(""), page, per_page);

    if let Some(if_none_match) = headers.get("if-none-match").and_then(|v| v.to_str().ok()) {
        if if_none_match == etag {
            return axum::response::Response::builder()
                .status(StatusCode::NOT_MODIFIED)
                .header("etag", &etag)
                .body(axum::body::Body::empty()).map_err(|e| internal(e.to_string()));
        }
    }

    // Count total for pagination header
    let total_count: (i64,) = if let Some(proj) = project_filter {
        sqlx::query_as("SELECT COUNT(*) FROM tasks WHERE deleted_at IS NULL AND project = ?")
            .bind(proj).fetch_one(&engine.pool).await.map_err(internal)?
    } else {
        sqlx::query_as("SELECT COUNT(*) FROM tasks WHERE deleted_at IS NULL")
            .fetch_one(&engine.pool).await.map_err(internal)?
    };

    let tasks = db::list_tasks_paged(&engine.pool, db::TaskFilter {
        status: None, project: project_filter, search: None, assignee: None,
        due_before: None, due_after: None, priority: None, team_id: None,
        user_id: None, label: None,
    }, per_page, offset).await.map_err(internal)?;

    let task_ids: Vec<i64> = tasks.iter().map(|t| t.id).collect();
    let (task_sprints, burn_totals, assignees, labels) = tokio::join!(
        scoped_task_sprints(&engine.pool, &task_ids),
        scoped_burn_totals(&engine.pool, &task_ids),
        scoped_assignees(&engine.pool, &task_ids),
        scoped_labels(&engine.pool, &task_ids),
    );
    let resp = TasksFullResponse {
        tasks,
        task_sprints: task_sprints.map_err(internal)?,
        burn_totals: burn_totals.map_err(internal)?,
        assignees: assignees.map_err(internal)?,
        labels: labels.map_err(internal)?,
    };
    let body = serde_json::to_vec(&resp).map_err(internal)?;
    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .header("etag", &etag)
        .header("x-total-count", total_count.0.to_string())
        .header("x-page", page.to_string())
        .header("x-per-page", per_page.to_string())
        .body(axum::body::Body::from(body)).map_err(|e| internal(e.to_string()))
}

#[derive(Deserialize)]
pub struct SseQuery { pub ticket: Option<String> }

// Short-lived opaque tickets for SSE (avoids JWT in query string / logs)
static SSE_TICKETS: std::sync::OnceLock<tokio::sync::Mutex<HashMap<String, (i64, std::time::Instant)>>> = std::sync::OnceLock::new();
pub(crate) fn sse_tickets() -> &'static tokio::sync::Mutex<HashMap<String, (i64, std::time::Instant)>> {
    SSE_TICKETS.get_or_init(|| tokio::sync::Mutex::new(HashMap::new()))
}

#[utoipa::path(post, path = "/api/timer/ticket", responses((status = 200)), security(("bearer" = [])))]
pub async fn create_sse_ticket(claims: Claims) -> ApiResult<serde_json::Value> {
    // V36-1: Use getrandom for cryptographic randomness
    let ticket = {
        let mut buf = [0u8; 24];
        getrandom::fill(&mut buf).map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to generate ticket: {}", e)))?;
        buf.iter().map(|b| format!("{:02x}", b)).collect::<String>()
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


async fn scoped_task_sprints(pool: &db::Pool, ids: &[i64]) -> Result<Vec<db::TaskSprintInfo>, String> {
    if ids.is_empty() { return Ok(vec![]); }
    let ph = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let q = format!("SELECT st.task_id, sp.id as sprint_id, sp.name as sprint_name, sp.status as sprint_status FROM sprint_tasks st JOIN sprints sp ON st.sprint_id = sp.id WHERE st.task_id IN ({}) ORDER BY st.task_id", ph);
    let mut query = sqlx::query_as::<_, db::TaskSprintInfo>(&q);
    for id in ids { query = query.bind(*id); }
    query.fetch_all(pool).await.map_err(|e| e.to_string())
}

async fn scoped_burn_totals(pool: &db::Pool, ids: &[i64]) -> Result<Vec<BurnTotalEntry>, String> {
    if ids.is_empty() { return Ok(vec![]); }
    let ph = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let q = format!("SELECT task_id, COALESCE(SUM(points), 0), COALESCE(SUM(hours), 0), COUNT(*) FROM burn_log WHERE cancelled = 0 AND task_id IN ({}) GROUP BY task_id", ph);
    let mut query = sqlx::query_as::<_, (i64, f64, f64, i64)>(&q);
    for id in ids { query = query.bind(*id); }
    let rows = query.fetch_all(pool).await.map_err(|e| e.to_string())?;
    Ok(rows.into_iter().map(|(tid, p, h, c)| BurnTotalEntry { task_id: tid, total_points: p, total_hours: h, count: c }).collect())
}

async fn scoped_assignees(pool: &db::Pool, ids: &[i64]) -> Result<Vec<db::TaskAssignee>, String> {
    if ids.is_empty() { return Ok(vec![]); }
    let ph = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let q = format!("SELECT ta.task_id, ta.user_id, u.username FROM task_assignees ta JOIN users u ON ta.user_id = u.id WHERE ta.task_id IN ({}) ORDER BY ta.task_id, u.username", ph);
    let mut query = sqlx::query_as::<_, db::TaskAssignee>(&q);
    for id in ids { query = query.bind(*id); }
    query.fetch_all(pool).await.map_err(|e| e.to_string())
}

async fn scoped_labels(pool: &db::Pool, ids: &[i64]) -> Result<Vec<db::TaskLabel>, String> {
    if ids.is_empty() { return Ok(vec![]); }
    let ph = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let q = format!("SELECT tl.task_id, l.id, l.name, l.color FROM labels l JOIN task_labels tl ON l.id = tl.label_id WHERE tl.task_id IN ({}) ORDER BY tl.task_id, l.name", ph);
    let mut query = sqlx::query_as::<_, db::TaskLabel>(&q);
    for id in ids { query = query.bind(*id); }
    query.fetch_all(pool).await.map_err(|e| e.to_string())
}

// --- Rooms ---

// F13: Task links (GitHub/GitLab integration)
#[utoipa::path(get, path = "/api/tasks/{id}/links", responses((status = 200)), security(("bearer" = [])))]
pub async fn get_task_links(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<serde_json::Value>> {
    db::get_task(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    let rows: Vec<(i64, String, String, String, String)> = sqlx::query_as("SELECT id, link_type, url, title, created_at FROM task_links WHERE task_id = ? ORDER BY created_at DESC")
        .bind(id).fetch_all(&engine.pool).await.map_err(internal)?;
    Ok(Json(rows.into_iter().map(|(id, lt, url, title, at)| serde_json::json!({"id": id, "link_type": lt, "url": url, "title": title, "created_at": at})).collect()))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct AddTaskLinkRequest { pub link_type: String, pub url: String, pub title: String }

#[utoipa::path(post, path = "/api/tasks/{id}/links", responses((status = 201)), security(("bearer" = [])))]
pub async fn add_task_link(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<AddTaskLinkRequest>) -> Result<StatusCode, ApiError> {
    let task = db::get_task(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    if !is_owner_or_root(task.user_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    if req.url.len() > 2000 { return Err(err(StatusCode::BAD_REQUEST, "URL too long")); }
    if req.title.len() > 500 { return Err(err(StatusCode::BAD_REQUEST, "Title too long")); }
    // V32-8: Validate link_type
    const VALID_LINK_TYPES: &[&str] = &["commit", "pr", "issue", "url", "doc", "design"];
    if !VALID_LINK_TYPES.contains(&req.link_type.as_str()) {
        return Err(err(StatusCode::BAD_REQUEST, format!("Invalid link_type '{}'. Must be one of: {}", req.link_type, VALID_LINK_TYPES.join(", "))));
    }
    sqlx::query("INSERT INTO task_links (task_id, link_type, url, title, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind(id).bind(&req.link_type).bind(&req.url).bind(&req.title).bind(db::now_str())
        .execute(&engine.pool).await.map_err(internal)?;
    Ok(StatusCode::CREATED)
}

// F13: GitHub/GitLab webhook receiver — parses push events and links commits to tasks
#[derive(Deserialize, utoipa::ToSchema)]
pub struct GitHubPushEvent {
    pub commits: Option<Vec<GitHubCommit>>,
    pub repository: Option<GitHubRepo>,
}
#[derive(Deserialize, utoipa::ToSchema)]
pub struct GitHubCommit { pub id: String, pub message: String, pub url: String }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct GitHubRepo { pub full_name: Option<String> }

#[utoipa::path(post, path = "/api/integrations/github", responses((status = 200)),
    request_body(content = GitHubPushEvent, content_type = "application/json"))]
pub async fn github_webhook(State(engine): State<AppState>, headers: axum::http::HeaderMap, body: axum::body::Bytes) -> Result<StatusCode, ApiError> {
    // PF5: Verify HMAC-SHA256 signature if GITHUB_WEBHOOK_SECRET is set
    // V29-8: Warn if secret is not configured
    if let Ok(secret) = std::env::var("GITHUB_WEBHOOK_SECRET") {
        let sig_header = headers.get("x-hub-signature-256").and_then(|v| v.to_str().ok()).unwrap_or("");
        let expected_sig = sig_header.strip_prefix("sha256=").unwrap_or("");
        use hmac::{Hmac, Mac, KeyInit};
        type HmacSha256 = Hmac<sha2::Sha256>;
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "HMAC init failed"))?;
        mac.update(&body);
        let computed = hex::encode(mac.finalize().into_bytes());
        if computed != expected_sig {
            return Err(err(StatusCode::UNAUTHORIZED, "Invalid webhook signature"));
        }
    }
    let payload: GitHubPushEvent = serde_json::from_slice(&body).map_err(|e| err(StatusCode::BAD_REQUEST, format!("Invalid JSON: {}", e)))?;
    let repo = payload.repository.as_ref().and_then(|r| r.full_name.as_deref()).unwrap_or("unknown");
    let now = db::now_str();
    let mut linked = 0;
    for commit in payload.commits.unwrap_or_default() {
        // Parse task IDs from commit message: #123, task-123, TASK-123
        let re_ids: Vec<i64> = commit.message.split_whitespace()
            .filter_map(|w| {
                let w = w.trim_matches(|c: char| !c.is_alphanumeric() && c != '#' && c != '-');
                if let Some(n) = w.strip_prefix('#') { return n.parse().ok(); }
                let lower = w.to_lowercase();
                if let Some(n) = lower.strip_prefix("task-") { return n.parse().ok(); }
                None
            }).collect();
        for task_id in re_ids {
            // Verify task exists
            if db::get_task(&engine.pool, task_id).await.is_ok() {
                let title = format!("{}: {}", &commit.id[..7.min(commit.id.len())], commit.message.lines().next().unwrap_or("").chars().take(100).collect::<String>());
                sqlx::query("INSERT INTO task_links (task_id, link_type, url, title, created_at) VALUES (?, 'commit', ?, ?, ?)")
                    .bind(task_id).bind(&commit.url).bind(&title).bind(&now)
                    .execute(&engine.pool).await.ok();
                linked += 1;
            }
        }
    }
    tracing::info!("GitHub webhook from {}: linked {} commits", repo, linked);
    Ok(StatusCode::OK)
}

// F19: Automation rules
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateAutomationRuleRequest {
    pub name: String,
    pub trigger_event: String,
    pub condition_json: Option<String>,
    pub action_json: String,
}

#[derive(sqlx::FromRow, serde::Serialize, utoipa::ToSchema)]
pub struct AutomationRule {
    pub id: i64, pub user_id: i64, pub name: String, pub trigger_event: String,
    pub condition_json: String, pub action_json: String, pub enabled: i64, pub created_at: String,
}

const VALID_TRIGGERS: &[&str] = &["task.status_changed", "task.due_approaching", "task.all_subtasks_done", "task.created", "task.assigned", "task.priority_changed"];

#[utoipa::path(get, path = "/api/automations", responses((status = 200)), security(("bearer" = [])))]
pub async fn list_automations(State(engine): State<AppState>, claims: Claims) -> ApiResult<Vec<AutomationRule>> {
    let rows = sqlx::query_as::<_, AutomationRule>("SELECT * FROM automation_rules WHERE user_id = ? ORDER BY created_at DESC")
        .bind(claims.user_id).fetch_all(&engine.pool).await.map_err(internal)?;
    Ok(Json(rows))
}

#[utoipa::path(post, path = "/api/automations", responses((status = 201)), security(("bearer" = [])))]
pub async fn create_automation(State(engine): State<AppState>, claims: Claims, Json(req): Json<CreateAutomationRuleRequest>) -> Result<(StatusCode, Json<AutomationRule>), ApiError> {
    if req.name.trim().is_empty() || req.name.len() > 200 { return Err(err(StatusCode::BAD_REQUEST, "Name required (max 200 chars)")); }
    if !VALID_TRIGGERS.contains(&req.trigger_event.as_str()) { return Err(err(StatusCode::BAD_REQUEST, format!("Invalid trigger. Must be one of: {}", VALID_TRIGGERS.join(", ")))); }
    if req.action_json.len() > 4096 { return Err(err(StatusCode::BAD_REQUEST, "Action JSON too large")); }
    // PF6: Validate JSON
    if serde_json::from_str::<serde_json::Value>(&req.action_json).is_err() { return Err(err(StatusCode::BAD_REQUEST, "action_json is not valid JSON")); }
    if let Some(ref c) = req.condition_json { if serde_json::from_str::<serde_json::Value>(c).is_err() { return Err(err(StatusCode::BAD_REQUEST, "condition_json is not valid JSON")); } }
    // Limit rules per user
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM automation_rules WHERE user_id = ?")
        .bind(claims.user_id).fetch_one(&engine.pool).await.map_err(internal)?;
    if count >= 50 { return Err(err(StatusCode::BAD_REQUEST, "Too many rules (max 50)")); }
    let now = db::now_str();
    let id = sqlx::query("INSERT INTO automation_rules (user_id, name, trigger_event, condition_json, action_json, enabled, created_at) VALUES (?, ?, ?, ?, ?, 1, ?)")
        .bind(claims.user_id).bind(req.name.trim()).bind(&req.trigger_event)
        .bind(req.condition_json.as_deref().unwrap_or("{}")).bind(&req.action_json).bind(&now)
        .execute(&engine.pool).await.map_err(internal)?.last_insert_rowid();
    let rule = sqlx::query_as::<_, AutomationRule>("SELECT * FROM automation_rules WHERE id = ?")
        .bind(id).fetch_one(&engine.pool).await.map_err(internal)?;
    Ok((StatusCode::CREATED, Json(rule)))
}

#[utoipa::path(delete, path = "/api/automations/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_automation(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    let rule: (i64,) = sqlx::query_as("SELECT user_id FROM automation_rules WHERE id = ?")
        .bind(id).fetch_one(&engine.pool).await.map_err(|_| err(StatusCode::NOT_FOUND, "Rule not found"))?;
    if !is_owner_or_root(rule.0, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    sqlx::query("DELETE FROM automation_rules WHERE id = ?").bind(id).execute(&engine.pool).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(put, path = "/api/automations/{id}/toggle", responses((status = 200)), security(("bearer" = [])))]
pub async fn toggle_automation(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    let rule: (i64,) = sqlx::query_as("SELECT user_id FROM automation_rules WHERE id = ?")
        .bind(id).fetch_one(&engine.pool).await.map_err(|_| err(StatusCode::NOT_FOUND, "Rule not found"))?;
    if !is_owner_or_root(rule.0, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    sqlx::query("UPDATE automation_rules SET enabled = CASE WHEN enabled = 1 THEN 0 ELSE 1 END WHERE id = ?")
        .bind(id).execute(&engine.pool).await.map_err(internal)?;
    Ok(StatusCode::OK)
}

// F12: User presence — track last activity
#[utoipa::path(get, path = "/api/users/presence", responses((status = 200)), security(("bearer" = [])))]
pub async fn user_presence(State(engine): State<AppState>, _claims: Claims) -> ApiResult<Vec<serde_json::Value>> {
    // Get all users with their last session activity
    let rows: Vec<(i64, String, Option<String>)> = sqlx::query_as(
        "SELECT u.id, u.username, (SELECT MAX(s.started_at) FROM sessions s WHERE s.user_id = u.id) as last_active FROM users u ORDER BY last_active DESC NULLS LAST")
        .fetch_all(&engine.pool).await.map_err(internal)?;

    // Check who has an active timer
    let active_users: std::collections::HashSet<i64> = {
        let states = engine.states.lock().await;
        states.iter().filter(|(_, s)| s.status != crate::engine::TimerStatus::Idle).map(|(uid, _)| *uid).collect()
    };

    Ok(Json(rows.into_iter().map(|(id, username, last_active)| {
        let online = active_users.contains(&id);
        serde_json::json!({"user_id": id, "username": username, "online": online, "last_active": last_active})
    }).collect()))
}

// F14: Slack webhook integration — format payload for Slack incoming webhooks
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SlackIntegrationRequest { pub webhook_url: String, pub events: Option<String> }

#[utoipa::path(post, path = "/api/integrations/slack", responses((status = 201)), security(("bearer" = [])))]
pub async fn create_slack_integration(State(engine): State<AppState>, claims: Claims, Json(req): Json<SlackIntegrationRequest>) -> Result<StatusCode, ApiError> {
    if !req.webhook_url.starts_with("https://hooks.slack.com/") && !req.webhook_url.starts_with("https://discord.com/api/webhooks/") {
        return Err(err(StatusCode::BAD_REQUEST, "URL must be a Slack or Discord webhook URL"));
    }
    if req.webhook_url.len() > 500 { return Err(err(StatusCode::BAD_REQUEST, "URL too long")); }
    let events = req.events.as_deref().unwrap_or("sprint.started,sprint.completed");
    // Store as a regular webhook with a special "slack" marker in the events field
    db::create_webhook(&engine.pool, claims.user_id, &req.webhook_url, &format!("slack:{}", events), None).await.map_err(internal)?;
    Ok(StatusCode::CREATED)
}
