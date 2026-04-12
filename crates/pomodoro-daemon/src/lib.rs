pub mod auth;
pub mod config;
pub mod db;
pub mod engine;
pub mod notify;
pub mod routes;
pub mod webhook;

use axum::Router;
use axum::handler::Handler;
use axum::response::IntoResponse;
use std::sync::Arc;
use tower_http::cors::{CorsLayer, AllowOrigin};
use axum::http::{HeaderValue, Method, header};

pub fn build_router(engine: Arc<engine::Engine>) -> Router {
    use axum::routing::{delete, get, post, put};

    // CORS: env var POMODORO_CORS_ORIGINS overrides config, defaults to localhost
    let origins_str = std::env::var("POMODORO_CORS_ORIGINS").ok();
    let extra: Vec<HeaderValue> = origins_str.as_deref()
        .map(|s| s.split(',').filter_map(|o| o.trim().parse().ok()).collect())
        .unwrap_or_else(|| engine.config.try_lock().map(|c| c.cors_origins.iter().filter_map(|o| o.parse().ok()).collect()).unwrap_or_default());
    let mut all_origins: Vec<HeaderValue> = vec![
        "http://localhost:1420".parse().unwrap(),
        "http://127.0.0.1:1420".parse().unwrap(),
        "http://localhost:9090".parse().unwrap(),
        "http://127.0.0.1:9090".parse().unwrap(),
        "tauri://localhost".parse().unwrap(),
    ];
    all_origins.extend(extra);

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(all_origins))
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::IF_NONE_MATCH,
            axum::http::HeaderName::from_static("x-requested-with")])
        .expose_headers([header::ETAG,
            axum::http::HeaderName::from_static("x-total-count"),
            axum::http::HeaderName::from_static("x-page"),
            axum::http::HeaderName::from_static("x-per-page")]);

    Router::new()
        .route("/api/health", get(routes::health))
        .route("/api/auth/register", post(routes::register))
        .route("/api/auth/login", post(routes::login))
        .route("/api/auth/logout", post(routes::logout))
        .route("/api/auth/refresh", post(routes::refresh_token))
        .route("/api/timer", get(routes::get_state))
        .route("/api/timer/start", post(routes::start))
        .route("/api/timer/pause", post(routes::pause))
        .route("/api/timer/resume", post(routes::resume))
        .route("/api/timer/stop", post(routes::stop))
        .route("/api/timer/skip", post(routes::skip))
        .route("/api/tasks", get(routes::list_tasks).post(routes::create_task))
        .route("/api/tasks/trash", get(routes::list_deleted_tasks))
        .route("/api/tasks/{id}", get(routes::get_task_detail).put(routes::update_task).delete(routes::delete_task))
        .route("/api/tasks/{id}/restore", post(routes::restore_task))
        .route("/api/tasks/{id}/duplicate", post(routes::duplicate_task))
        .route("/api/tasks/bulk-status", put(routes::bulk_update_status))
        .route("/api/tasks/{id}/comments", get(routes::list_comments).post(routes::add_comment))
        .route("/api/comments/{id}", delete(routes::delete_comment))
        .route("/api/tasks/{id}/time", get(routes::list_time_reports).post(routes::add_time_report))
        .route("/api/tasks/{id}/assignees", get(routes::list_assignees).post(routes::add_assignee))
        .route("/api/tasks/{id}/assignees/{username}", delete(routes::remove_assignee))
        .route("/api/tasks/{id}/watchers", get(routes::get_task_watchers))
        .route("/api/tasks/{id}/watch", post(routes::watch_task).delete(routes::unwatch_task))
        .route("/api/watched", get(routes::get_watched_tasks))
        .route("/api/tasks/{id}/votes", get(routes::get_task_votes))
        .route("/api/tasks/{id}/sessions", get(routes::get_task_sessions))
        .route("/api/sessions/{id}/note", put(routes::update_session_note))
        .route("/api/tasks/{id}/burn-total", get(routes::get_task_burn_total))
        .route("/api/tasks/{id}/burn-users", get(routes::get_task_burn_users))
        .route("/api/history", get(routes::get_history))
        .route("/api/stats", get(routes::get_stats))
        .route("/api/config", get(routes::get_config).put(routes::update_config))
        .route("/api/profile", put(routes::update_profile))
        .route("/api/profile/notifications", get(routes::get_notif_prefs).put(routes::update_notif_prefs))
        .route("/api/admin/users", get(routes::list_users))
        .route("/api/admin/users/{id}/role", put(routes::update_user_role))
        .route("/api/admin/users/{id}", delete(routes::delete_user))
        .route("/api/admin/backup", post(routes::create_backup))
        .route("/api/rooms", get(routes::list_rooms).post(routes::create_room))
        .route("/api/rooms/{id}", get(routes::get_room_state).delete(routes::delete_room))
        .route("/api/rooms/{id}/join", post(routes::join_room))
        .route("/api/rooms/{id}/leave", post(routes::leave_room))
        .route("/api/rooms/{id}/members/{username}", delete(routes::kick_member))
        .route("/api/rooms/{id}/role", put(routes::set_room_role))
        .route("/api/rooms/{id}/start-voting", post(routes::start_voting))
        .route("/api/rooms/{id}/vote", post(routes::cast_vote))
        .route("/api/rooms/{id}/reveal", post(routes::reveal_votes))
        .route("/api/rooms/{id}/accept", post(routes::accept_estimate))
        .route("/api/rooms/{id}/close", post(routes::close_room))
        .route("/api/rooms/{id}/ws", get(routes::room_ws))
        .route("/api/rooms/{id}/export", get(routes::export_room_history))
        .route("/api/sprints", get(routes::list_sprints).post(routes::create_sprint))
        .route("/api/sprints/{id}", get(routes::get_sprint_detail).put(routes::update_sprint).delete(routes::delete_sprint))
        .route("/api/sprints/{id}/start", post(routes::start_sprint))
        .route("/api/sprints/{id}/complete", post(routes::complete_sprint))
        .route("/api/sprints/{id}/carryover", post(routes::carryover_sprint))
        .route("/api/sprints/{id}/tasks", get(routes::get_sprint_tasks).post(routes::add_sprint_tasks))
        .route("/api/sprints/{id}/tasks/{task_id}", delete(routes::remove_sprint_task))
        .route("/api/sprints/{id}/burndown", get(routes::get_sprint_burndown))
        .route("/api/sprints/burndown", get(routes::get_global_burndown))
        .route("/api/sprints/velocity", get(routes::get_velocity))
        .route("/api/epics", get(routes::list_epic_groups).post(routes::create_epic_group))
        .route("/api/epics/{id}", get(routes::get_epic_group).delete(routes::delete_epic_group))
        .route("/api/epics/{id}/tasks", post(routes::add_epic_group_tasks))
        .route("/api/epics/{id}/tasks/{task_id}", delete(routes::remove_epic_group_task))
        .route("/api/epics/{id}/snapshot", post(routes::snapshot_epic_group))
        .route("/api/sprints/{id}/roots", get(routes::get_sprint_root_tasks).post(routes::add_sprint_root_tasks))
        .route("/api/sprints/{id}/roots/{task_id}", delete(routes::remove_sprint_root_task))
        .route("/api/sprints/{id}/scope", get(routes::get_sprint_scope))
        .route("/api/teams", get(routes::list_teams).post(routes::create_team))
        .route("/api/teams/{id}", get(routes::get_team).delete(routes::delete_team))
        .route("/api/teams/{id}/members", post(routes::add_team_member))
        .route("/api/teams/{id}/members/{user_id}", delete(routes::remove_team_member))
        .route("/api/teams/{id}/roots", post(routes::add_team_root_tasks))
        .route("/api/teams/{id}/roots/{task_id}", delete(routes::remove_team_root_task))
        .route("/api/teams/{id}/scope", get(routes::get_team_scope))
        .route("/api/me/teams", get(routes::get_my_teams))
        .route("/api/sprints/{id}/snapshot", post(routes::snapshot_sprint))
        .route("/api/sprints/{id}/board", get(routes::get_sprint_board))
        .route("/api/sprints/{id}/burn", post(routes::log_burn))
        .route("/api/sprints/{id}/burns", get(routes::list_burns))
        .route("/api/sprints/{id}/burns/{burn_id}", delete(routes::cancel_burn))
        .route("/api/sprints/{id}/burn-summary", get(routes::get_burn_summary))
        .route("/api/task-sprints", get(routes::get_task_sprints))
        .route("/api/users", get(routes::list_usernames))
        .route("/api/burn-totals", get(routes::get_all_burn_totals))
        .route("/api/assignees", get(routes::get_all_assignees))
        .route("/api/tasks/full", get(routes::get_tasks_full))
        .route("/api/tasks/reorder", axum::routing::post(routes::reorder_tasks))
        .route("/api/export/tasks", get(routes::export_tasks))
        .route("/api/export/sessions", get(routes::export_sessions))
        .route("/api/export/burns/{sprint_id}", get(routes::export_burns))
        .route("/api/import/tasks", post(routes::import_tasks_csv))
        .route("/api/import/tasks/json", post(routes::import_tasks_json))
        .route("/api/audit", get(routes::list_audit))
        .route("/api/labels", get(routes::list_labels).post(routes::create_label))
        .route("/api/labels/{id}", delete(routes::delete_label))
        .route("/api/tasks/{id}/labels/{label_id}", axum::routing::put(routes::add_task_label).delete(routes::remove_task_label))
        .route("/api/tasks/{id}/labels", get(routes::get_task_labels))
        .route("/api/tasks/{id}/recurrence", get(routes::get_recurrence).put(routes::set_recurrence).delete(routes::remove_recurrence))
        .route("/api/tasks/{id}/dependencies", get(routes::get_dependencies).post(routes::add_dependency))
        .route("/api/tasks/{id}/dependencies/{dep_id}", delete(routes::remove_dependency))
        .route("/api/dependencies", get(routes::get_all_dependencies))
        .route("/api/webhooks", get(routes::list_webhooks).post(routes::create_webhook))
        .route("/api/webhooks/{id}", delete(routes::delete_webhook))
        .route("/api/templates", get(routes::list_templates).post(routes::create_template))
        .route("/api/templates/{id}", delete(routes::delete_template))
        .route("/api/tasks/{id}/attachments", get(routes::list_attachments)
            .post(routes::upload_attachment.layer(axum::extract::DefaultBodyLimit::max(10 * 1024 * 1024))))
        .route("/api/attachments/{id}/download", get(routes::download_attachment))
        .route("/api/attachments/{id}", delete(routes::delete_attachment))
        .route("/api/timer/sse", get(routes::sse_timer))
        .route("/api/timer/ticket", axum::routing::post(routes::create_sse_ticket))
        .layer(axum::extract::DefaultBodyLimit::max(2 * 1024 * 1024)) // 2MB max request body
        .layer(cors)
        .layer(axum::middleware::from_fn(security_headers))
        .layer(axum::middleware::from_fn(request_id_logger))
        .layer(axum::middleware::from_fn(api_rate_limit))
        .with_state(engine)
}

async fn security_headers(req: axum::extract::Request, next: axum::middleware::Next) -> axum::response::Response {
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    h.insert("x-content-type-options", "nosniff".parse().unwrap());
    h.insert("x-frame-options", "DENY".parse().unwrap());
    h.insert("referrer-policy", "strict-origin-when-cross-origin".parse().unwrap());
    resp
}

// O2: Request ID + structured error logging
async fn request_id_logger(req: axum::extract::Request, next: axum::middleware::Next) -> axum::response::Response {
    use std::time::{SystemTime, UNIX_EPOCH};
    let rid = format!("{:08x}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().subsec_nanos());
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let mut resp = next.run(req).await;
    let status = resp.status().as_u16();
    if status >= 400 {
        tracing::warn!(request_id = %rid, method = %method, path = %path, status = status, "request error");
    }
    resp.headers_mut().insert("x-request-id", rid.parse().unwrap());
    resp
}

async fn api_rate_limit(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    let method = req.method().clone();
    if method == axum::http::Method::GET || method == axum::http::Method::HEAD || method == axum::http::Method::OPTIONS {
        return next.run(req).await.into_response();
    }
    let ip = routes::extract_ip(req.headers());
    let limiter = routes::api_limiter();
    let now = std::time::Instant::now();
    {
        let mut map = limiter.attempts.lock().unwrap();
        if map.len() > 500 {
            map.retain(|_, entries| {
                entries.retain(|t| now.duration_since(*t).as_secs() < limiter.window_secs);
                !entries.is_empty()
            });
        }
        let entries = map.entry(ip).or_default();
        entries.retain(|t| now.duration_since(*t).as_secs() < limiter.window_secs);
        if entries.len() >= limiter.max_requests {
            return (axum::http::StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response();
        }
        entries.push(now);
    }
    next.run(req).await.into_response()
}
