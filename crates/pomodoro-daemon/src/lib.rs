pub mod auth;
pub mod automation;
pub mod config;
pub mod db;
#[cfg(feature = "email")]
pub mod email;
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

pub async fn build_router(engine: Arc<engine::Engine>) -> Router {
    use axum::routing::{delete, get, post, put};

    // CORS: env var POMODORO_CORS_ORIGINS overrides config, defaults to localhost
    let origins_str = std::env::var("POMODORO_CORS_ORIGINS").ok();
    let extra: Vec<HeaderValue> = if let Some(s) = origins_str.as_deref() {
        s.split(',').filter_map(|o| o.trim().parse().ok()).collect()
    } else {
        let c = engine.config.lock().await;
        c.cors_origins.iter().filter_map(|o| o.parse().ok()).collect()
    };
    let mut all_origins: Vec<HeaderValue> = vec![
        "http://localhost:1420".parse().unwrap(),
        "http://127.0.0.1:1420".parse().unwrap(),
        "http://localhost:9090".parse().unwrap(),
        "http://127.0.0.1:9090".parse().unwrap(),
        "tauri://localhost".parse().unwrap(),
    ];
    all_origins.extend(extra);

    let allow_all_cors = std::env::var("POMODORO_CORS_ALLOW_ALL").is_ok();
    let cors = CorsLayer::new()
        .allow_origin(if allow_all_cors { AllowOrigin::any() } else { AllowOrigin::list(all_origins) })
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::IF_NONE_MATCH,
            axum::http::HeaderName::from_static("x-requested-with")])
        .expose_headers([header::ETAG,
            axum::http::HeaderName::from_static("x-total-count"),
            axum::http::HeaderName::from_static("x-page"),
            axum::http::HeaderName::from_static("x-per-page")]);

    let mut router = Router::new()
        .route("/api/health", get(routes::health))
        .route("/api/auth/register", post(routes::register))
        .route("/api/auth/login", post(routes::login))
        .route("/api/auth/logout", post(routes::logout))
        .route("/api/auth/password", put(routes::change_password))
        .route("/api/auth/refresh", post(routes::refresh_token))
        .route("/api/timer", get(routes::get_state))
        .route("/api/timer/active", get(routes::get_active_timers))
        .route("/api/timer/start", post(routes::start))
        .route("/api/timer/pause", post(routes::pause))
        .route("/api/timer/resume", post(routes::resume))
        .route("/api/timer/stop", post(routes::stop))
        .route("/api/timer/skip", post(routes::skip))
        .route("/api/timer/join/{session_id}", post(routes::join_session))
        .route("/api/timer/participants/{session_id}", get(routes::session_participants))
        .route("/api/tasks", get(routes::list_tasks).post(routes::create_task))
        .route("/api/tasks/trash", get(routes::list_deleted_tasks))
        .route("/api/tasks/archived", get(routes::list_archived_tasks))
        .route("/api/tasks/search", get(routes::search_tasks))
        .route("/api/tasks/search/advanced", post(routes::advanced_search))
        .route("/api/search", get(routes::global_search))
        .route("/api/tasks/{id}", get(routes::get_task_detail).put(routes::update_task).delete(routes::delete_task))
        .route("/api/tasks/{id}/restore", post(routes::restore_task))
        .route("/api/tasks/{id}/unarchive", post(routes::unarchive_task))
        .route("/api/tasks/{id}/permanent", delete(routes::purge_task))
        .route("/api/tasks/{id}/duplicate", post(routes::duplicate_task))
        .route("/api/tasks/bulk-status", put(routes::bulk_update_status))
        .route("/api/tasks/bulk-assign", post(routes::bulk_assign))
        .route("/api/tasks/bulk-sprint", post(routes::bulk_sprint_move))
        .route("/api/tasks/{id}/comments", get(routes::list_comments).post(routes::add_comment))
        .route("/api/comments/{id}", delete(routes::delete_comment).put(routes::edit_comment))
        .route("/api/tasks/{id}/time", get(routes::list_time_reports).post(routes::add_time_report))
        .route("/api/tasks/{id}/time-summary", get(routes::get_task_time_summary))
        .route("/api/tasks/{id}/assignees", get(routes::list_assignees).post(routes::add_assignee))
        .route("/api/tasks/{id}/assignees/{username}", delete(routes::remove_assignee))
        .route("/api/tasks/{id}/watchers", get(routes::get_task_watchers))
        .route("/api/tasks/{id}/watch", post(routes::watch_task).delete(routes::unwatch_task))
        .route("/api/watched", get(routes::get_watched_tasks))
        .route("/api/tasks/{id}/votes", get(routes::get_task_votes))
        .route("/api/tasks/{id}/links", get(routes::get_task_links).post(routes::add_task_link))
        .route("/api/integrations/github", post(routes::github_webhook))
        .route("/api/automations", get(routes::list_automations).post(routes::create_automation))
        .route("/api/automations/{id}", delete(routes::delete_automation))
        .route("/api/automations/{id}/toggle", put(routes::toggle_automation))
        .route("/api/users/presence", get(routes::user_presence))
        .route("/api/integrations/slack", post(routes::create_slack_integration))
        .route("/api/tasks/{id}/sessions", get(routes::get_task_sessions))
        .route("/api/sessions/{id}/note", put(routes::update_session_note))
        .route("/api/tasks/{id}/burn-total", get(routes::get_task_burn_total))
        .route("/api/tasks/{id}/burn-users", get(routes::get_task_burn_users))
        .route("/api/history", get(routes::get_history))
        .route("/api/reports/user-hours", get(routes::user_hours_report))
        .route("/api/reports/time-tracking", get(routes::time_tracking_report))
        .route("/api/reports/sla", get(routes::sla_report))
        .route("/api/reports/standup", get(routes::standup_report))
        .route("/api/stats", get(routes::get_stats))
        .route("/api/analytics/estimation-accuracy", get(routes::estimation_accuracy))
        .route("/api/analytics/focus-score", get(routes::focus_score))
        .route("/api/achievements", get(routes::list_achievements))
        .route("/api/achievements/check", post(routes::check_achievements))
        .route("/api/leaderboard", get(routes::leaderboard))
        .route("/api/suggestions/priorities", get(routes::priority_suggestions))
        .route("/api/feed", get(routes::activity_feed))
        .route("/api/suggestions/schedule", get(routes::schedule_suggestions))
        .route("/api/reports/weekly-digest", get(routes::weekly_digest))
        .route("/api/config", get(routes::get_config).put(routes::update_config))
        .route("/api/profile", put(routes::update_profile))
        .route("/api/profile/notifications", get(routes::get_notif_prefs).put(routes::update_notif_prefs))
        .route("/api/admin/users", get(routes::list_users))
        .route("/api/admin/users/{id}/role", put(routes::update_user_role))
        .route("/api/admin/users/{id}/password", put(routes::admin_reset_password))
        .route("/api/admin/users/{id}", delete(routes::delete_user))
        .route("/api/admin/backup", post(routes::create_backup))
        .route("/api/admin/backups", get(routes::list_backups))
        .route("/api/admin/restore", post(routes::restore_backup))
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
        .route("/api/sprints/compare", get(routes::compare_sprints))
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
        .route("/api/sprints/{id}/retro-report", get(routes::sprint_retro_report))
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
        .route("/api/export/ical", get(routes::export_ical))
        .route("/api/export/project", get(routes::export_project))
        .route("/api/import/project", post(routes::import_project))
        .route("/api/import/tasks", post(routes::import_tasks_csv))
        .route("/api/import/tasks/json", post(routes::import_tasks_json))
        .route("/api/audit", get(routes::list_audit))
        .route("/api/labels", get(routes::list_labels).post(routes::create_label))
        .route("/api/labels/{id}", put(routes::update_label).delete(routes::delete_label))
        .route("/api/tasks/{id}/labels/{label_id}", axum::routing::put(routes::add_task_label).delete(routes::remove_task_label))
        .route("/api/tasks/{id}/labels", get(routes::get_task_labels))
        .route("/api/tasks/{id}/recurrence", get(routes::get_recurrence).put(routes::set_recurrence).delete(routes::remove_recurrence))
        .route("/api/tasks/{id}/dependencies", get(routes::get_dependencies).post(routes::add_dependency))
        .route("/api/tasks/{id}/dependencies/{dep_id}", delete(routes::remove_dependency))
        .route("/api/dependencies", get(routes::get_all_dependencies))
        .route("/api/webhooks", get(routes::list_webhooks).post(routes::create_webhook))
        .route("/api/webhooks/{id}", put(routes::update_webhook).delete(routes::delete_webhook))
        .route("/api/templates", get(routes::list_templates).post(routes::create_template))
        .route("/api/templates/{id}", put(routes::update_template).delete(routes::delete_template))
        .route("/api/templates/{id}/instantiate", post(routes::instantiate_template))
        .route("/api/tasks/{id}/save-as-template", post(routes::save_task_as_template))
        // Custom statuses (Jira-like workflows)
        .route("/api/statuses", get(routes::list_custom_statuses).post(routes::create_custom_status))
        .route("/api/statuses/{id}", put(routes::update_custom_status).delete(routes::delete_custom_status))
        // Custom fields on tasks
        .route("/api/fields", get(routes::list_custom_fields).post(routes::create_custom_field))
        .route("/api/fields/{id}", put(routes::update_custom_field).delete(routes::delete_custom_field))
        .route("/api/tasks/{id}/fields", get(routes::get_task_fields))
        .route("/api/tasks/{task_id}/fields/{field_id}", put(routes::set_task_field_value).delete(routes::delete_task_field_value))
        // Task checklists
        .route("/api/tasks/{id}/checklist", get(routes::list_checklist).post(routes::add_checklist_item))
        .route("/api/checklist/{id}", put(routes::update_checklist_item).delete(routes::delete_checklist_item))
        .route("/api/tasks/{id}/attachments", get(routes::list_attachments)
            .post(routes::upload_attachment.layer(axum::extract::DefaultBodyLimit::max(10 * 1024 * 1024))))
        .route("/api/attachments/{id}/download", get(routes::download_attachment))
        .route("/api/attachments/{id}", delete(routes::delete_attachment))
        // BL21-23: Notifications
        .route("/api/notifications", get(routes::list_notifications))
        .route("/api/notifications/unread", get(routes::unread_count))
        .route("/api/notifications/read", post(routes::mark_notifications_read))
        .route("/api/timer/sse", get(routes::sse_timer))
        .route("/api/timer/ticket", axum::routing::post(routes::create_sse_ticket))
        .layer(axum::extract::DefaultBodyLimit::max(2 * 1024 * 1024)) // 2MB max request body
        .layer(cors)
        .layer(axum::middleware::from_fn(security_headers))
        .layer(axum::middleware::from_fn(request_id_logger))
        .layer(axum::middleware::from_fn(api_rate_limit))
        .with_state(engine);

    // Web GUI: serve static files from gui/dist/ as fallback (SPA)
    if let Some(gui_dir) = resolve_gui_dir() {
        tracing::info!("Serving web GUI from {}", gui_dir.display());
        let serve = tower_http::services::ServeDir::new(&gui_dir)
            .fallback(tower_http::services::ServeFile::new(gui_dir.join("index.html")));
        router = router.fallback_service(serve);
    } else {
        tracing::info!("No gui/dist found — web GUI disabled (API-only mode)");
    }

    router
}

/// Find the gui/dist directory relative to the executable or working directory.
fn resolve_gui_dir() -> Option<std::path::PathBuf> {
    // 1. Env var override
    if let Ok(p) = std::env::var("POMODORO_GUI_DIR") {
        let pb = std::path::PathBuf::from(p);
        if pb.join("index.html").exists() { return Some(pb); }
    }
    // 2. Next to the executable (packaged .deb layout)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("gui");
            if candidate.join("index.html").exists() { return Some(candidate); }
        }
    }
    // 3. Relative to working directory (development)
    for candidate in ["gui/dist", "../gui/dist"] {
        let pb = std::path::PathBuf::from(candidate);
        if pb.join("index.html").exists() { return Some(pb); }
    }
    None
}

async fn security_headers(req: axum::extract::Request, next: axum::middleware::Next) -> axum::response::Response {
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    h.insert("x-content-type-options", "nosniff".parse().unwrap());
    h.insert("x-frame-options", "DENY".parse().unwrap());
    h.insert("referrer-policy", "strict-origin-when-cross-origin".parse().unwrap());
    h.insert("x-xss-protection", "1; mode=block".parse().unwrap());
    h.insert("permissions-policy", "camera=(), microphone=(), geolocation=()".parse().unwrap());
    h.insert("content-security-policy", "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self' ws: wss:; media-src 'self'".parse().unwrap());
    h.insert("strict-transport-security", "max-age=63072000; includeSubDomains".parse().unwrap());
    resp
}

// O2: Request ID + structured error logging
async fn request_id_logger(req: axum::extract::Request, next: axum::middleware::Next) -> axum::response::Response {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let rid = format!("{:012x}", COUNTER.fetch_add(1, Ordering::Relaxed));
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let start = std::time::Instant::now();
    let mut resp = next.run(req).await;
    let elapsed_ms = start.elapsed().as_millis();
    let status = resp.status().as_u16();
    if status >= 400 {
        tracing::warn!(request_id = %rid, method = %method, path = %path, status = status, elapsed_ms = elapsed_ms, "request error");
    } else if elapsed_ms > 500 {
        tracing::warn!(request_id = %rid, method = %method, path = %path, status = status, elapsed_ms = elapsed_ms, "slow request");
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
    if std::env::var("POMODORO_NO_RATE_LIMIT").is_err() {
        let limiter = routes::api_limiter();
        if !limiter.check_and_record(&ip) {
            return (axum::http::StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response();
        }
    }
    next.run(req).await.into_response()
}
