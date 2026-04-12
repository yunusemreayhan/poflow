use pomodoro_daemon::{auth, config, db, engine, notify, routes, build_router};

use anyhow::Result;
use chrono::Datelike;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(
        routes::register, routes::login,
        routes::get_state, routes::start, routes::pause, routes::resume, routes::stop, routes::skip,
        routes::list_tasks, routes::create_task, routes::get_task_detail, routes::update_task, routes::delete_task,
        routes::list_comments, routes::add_comment, routes::delete_comment,
        routes::get_history, routes::get_stats,
        routes::get_config, routes::update_config,
        routes::update_profile,
        routes::add_time_report, routes::list_time_reports, routes::get_task_burn_total, routes::get_task_burn_users,
        routes::list_assignees, routes::add_assignee, routes::remove_assignee,
        routes::list_users, routes::update_user_role, routes::delete_user,
        routes::list_rooms, routes::create_room, routes::get_room_state, routes::delete_room,
        routes::join_room, routes::leave_room, routes::kick_member, routes::set_room_role,
        routes::start_voting, routes::cast_vote, routes::reveal_votes, routes::accept_estimate, routes::close_room,
        routes::get_task_votes,
        routes::list_sprints, routes::create_sprint, routes::get_sprint_detail, routes::update_sprint, routes::delete_sprint,
        routes::start_sprint, routes::complete_sprint,
        routes::get_sprint_tasks, routes::add_sprint_tasks, routes::remove_sprint_task,
        routes::get_sprint_burndown, routes::snapshot_sprint, routes::get_sprint_board,
        routes::get_task_sprints,
        routes::list_usernames,
        routes::log_burn, routes::list_burns, routes::cancel_burn, routes::get_burn_summary,
        // v0.2 endpoints
        routes::get_global_burndown, routes::get_velocity,
        routes::list_epic_groups, routes::create_epic_group, routes::get_epic_group, routes::delete_epic_group,
        routes::add_epic_group_tasks, routes::remove_epic_group_task, routes::snapshot_epic_group,
        routes::list_teams, routes::create_team, routes::get_team, routes::delete_team,
        routes::add_team_member, routes::remove_team_member, routes::add_team_root_tasks, routes::remove_team_root_task,
        routes::get_team_scope, routes::get_my_teams,
        routes::get_sprint_root_tasks, routes::add_sprint_root_tasks, routes::remove_sprint_root_task, routes::get_sprint_scope,
        routes::get_all_burn_totals, routes::get_all_assignees, routes::get_tasks_full,
        routes::reorder_tasks, routes::export_tasks,
        routes::list_audit,
        routes::list_labels, routes::create_label, routes::delete_label,
        routes::add_task_label, routes::remove_task_label, routes::get_task_labels,
        routes::get_recurrence, routes::set_recurrence, routes::remove_recurrence,
        routes::get_dependencies, routes::add_dependency, routes::remove_dependency, routes::get_all_dependencies,
        routes::list_webhooks, routes::create_webhook, routes::delete_webhook,
        routes::create_sse_ticket,
        routes::logout,
        routes::export_sessions,
        routes::list_attachments, routes::upload_attachment, routes::download_attachment, routes::delete_attachment,
        routes::list_templates, routes::create_template, routes::delete_template,
        routes::refresh_token,
        routes::update_session_note,
        routes::carryover_sprint,
        routes::export_room_history,
        routes::import_tasks_json,
    ),
    components(schemas(
        db::Task, db::Session, db::Comment, db::User, db::TaskDetail, db::SessionWithPath, db::DayStat, db::TaskAssignee,
        db::Room, db::RoomMember, db::RoomVote, db::RoomState, db::RoomVoteView, db::VoteResult,
        db::Sprint, db::SprintTask, db::SprintDailyStat, db::SprintDetail, db::SprintBoard, db::TaskSprintInfo,
        db::BurnEntry, db::BurnSummaryEntry, db::BurnTotal,
        db::Team, db::TeamMember, db::TeamDetail,
        db::EpicGroup, db::EpicSnapshot, db::EpicGroupDetail,
        engine::EngineState, engine::TimerPhase, engine::TimerStatus,
        config::Config,
        routes::RegisterRequest, routes::LoginRequest, routes::AuthResponse,
        routes::CreateTaskRequest, routes::UpdateTaskRequest, routes::StartRequest,
        routes::AddCommentRequest, routes::HistoryQuery, routes::StatsQuery, routes::UpdateRoleRequest,
        routes::UpdateProfileRequest, routes::AddTimeReportRequest, routes::AssignRequest,
        routes::CreateRoomRequest, routes::RoomRoleRequest, routes::StartVotingRequest, routes::CastVoteRequest, routes::AcceptEstimateRequest,
        routes::CreateSprintRequest, routes::UpdateSprintRequest, routes::AddSprintTasksRequest,
        routes::LogBurnRequest, routes::ApiErrorBody,
        db::Attachment, db::TaskTemplate,
    )),
    modifiers(&SecurityAddon),
    info(title = "Pomodoro API", version = "1.0.0", description = "Multi-user Pomodoro timer with hierarchical task management")
)]
struct ApiDoc;

struct SecurityAddon;
impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme("bearer", utoipa::openapi::security::SecurityScheme::Http(
            utoipa::openapi::security::Http::new(utoipa::openapi::security::HttpAuthScheme::Bearer)
        ));
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let json_logs = std::env::var("POMODORO_LOG_JSON").map_or(false, |v| v == "1" || v.to_lowercase() == "true");
    let filter = tracing_subscriber::EnvFilter::from_default_env()
        .add_directive("pomodoro_daemon=info".parse()?);
    if json_logs {
        tracing_subscriber::fmt().with_env_filter(filter).json().init();
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }

    tracing::info!("Pomodoro daemon starting...");

    let config = config::Config::load()?;
    let pool = db::connect().await?;
    auth::init_pool(pool.clone()).await;

    let interrupted = db::recover_interrupted(&pool).await?;
    if !interrupted.is_empty() {
        tracing::warn!("Recovered {} interrupted sessions", interrupted.len());
    }

    let engine = Arc::new(engine::Engine::new(pool, config.clone()).await);

    // Graceful shutdown signal (O1)
    let (shutdown_tx, _) = tokio::sync::watch::channel(false);

    // Tick loop
    let engine_tick = engine.clone();
    let mut shutdown_rx = shutdown_tx.subscribe();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        let mut last_date = chrono::Utc::now().naive_utc().format("%Y-%m-%d").to_string();
        loop {
            tokio::select! {
                _ = interval.tick() => {},
                _ = shutdown_rx.changed() => break,
            }
            // Midnight reset: clear per-user daily counts
            let today = chrono::Utc::now().naive_utc().format("%Y-%m-%d").to_string();
            if today != last_date {
                last_date = today;
                let mut states = engine_tick.states.lock().await;
                for state in states.values_mut() {
                    // Reset to 0 — will be refreshed from DB on next start()
                    state.daily_completed = 0;
                }
            }
            match engine_tick.tick().await {
                Ok(completed) => {
                    engine_tick.heartbeat("tick").await;
                    for state in completed {
                        // Check user notification preferences
                        let (should_notify, play_sound) = match db::get_user_config(&engine_tick.pool, state.current_user_id).await {
                            Ok(Some(uc)) => (uc.notify_desktop.unwrap_or(1) != 0, uc.notify_sound.unwrap_or(1) != 0),
                            _ => (true, true),
                        };
                        if should_notify {
                            notify::notify_session_complete(state.phase, state.session_count, play_sound);
                        }
                    }
                }
                Err(e) => tracing::error!("Tick error: {}", e),
            }
        }
    });

    // Sprint burndown snapshot (hourly)
    let engine_snap = engine.clone();
    let mut shutdown_rx2 = shutdown_tx.subscribe();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        interval.tick().await; // Skip immediate first tick
        loop {
            tokio::select! {
                _ = interval.tick() => {},
                _ = shutdown_rx2.changed() => break,
            }
            if let Err(e) = db::snapshot_active_sprints(&engine_snap.pool).await {
                tracing::error!("Sprint snapshot error: {}", e);
            }
            if let Err(e) = db::snapshot_all_epic_groups(&engine_snap.pool).await {
                tracing::error!("Epic snapshot error: {}", e);
            }
            engine_snap.heartbeat("snapshot").await;
        }
    });

    // Recurring task processing (every 5 minutes)
    let engine_recur = engine.clone();
    let mut shutdown_rx3 = shutdown_tx.subscribe();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            tokio::select! {
                _ = interval.tick() => {},
                _ = shutdown_rx3.changed() => break,
            }
            let today = chrono::Utc::now().naive_utc().format("%Y-%m-%d").to_string();
            let due = match db::get_due_recurrences(&engine_recur.pool, &today).await {
                Ok(d) => d,
                Err(e) => { tracing::error!("Recurrence check error: {}", e); continue; }
            };
            for rec in due {
                // Skip if already created today (idempotency)
                if rec.last_created.as_deref() == Some(&today) { continue; }
                // Clone the template task
                if let Ok(task) = db::get_task(&engine_recur.pool, rec.task_id).await {
                    let title = format!("{} ({})", task.title, today);
                    if let Ok(_new) = db::create_task(&engine_recur.pool, task.user_id, task.parent_id, &title,
                        task.description.as_deref(), task.project.as_deref(), task.tags.as_deref(),
                        task.priority, task.estimated, task.estimated_hours, task.remaining_points, task.due_date.as_deref()).await {
                        // Advance next_due
                        let next = match rec.pattern.as_str() {
                            "daily" => chrono::NaiveDate::parse_from_str(&rec.next_due, "%Y-%m-%d").ok().map(|d| d + chrono::Duration::days(1)),
                            "weekly" => chrono::NaiveDate::parse_from_str(&rec.next_due, "%Y-%m-%d").ok().map(|d| d + chrono::Duration::weeks(1)),
                            "biweekly" => chrono::NaiveDate::parse_from_str(&rec.next_due, "%Y-%m-%d").ok().map(|d| d + chrono::Duration::weeks(2)),
                            "monthly" => chrono::NaiveDate::parse_from_str(&rec.next_due, "%Y-%m-%d").ok().map(|d| {
                                let m = d.month() % 12 + 1;
                                let y = if m == 1 { d.year() + 1 } else { d.year() };
                                // Preserve original day, clamping to month's last day
                                let original_day = task.due_date.as_ref()
                                    .and_then(|dd| chrono::NaiveDate::parse_from_str(dd, "%Y-%m-%d").ok())
                                    .map(|dd| dd.day()).unwrap_or(d.day());
                                // B4: Get last day of target month correctly
                                let next_month_first = if m < 12 {
                                    chrono::NaiveDate::from_ymd_opt(y, m + 1, 1)
                                } else {
                                    chrono::NaiveDate::from_ymd_opt(y + 1, 1, 1)
                                };
                                let max_day = next_month_first
                                    .and_then(|d| d.pred_opt()).map(|d| d.day()).unwrap_or(28);
                                chrono::NaiveDate::from_ymd_opt(y, m, original_day.min(max_day)).unwrap_or(d + chrono::Duration::days(30))
                            }),
                            _ => None,
                        };
                        if let Some(next_date) = next {
                            db::advance_recurrence(&engine_recur.pool, rec.task_id, &next_date.format("%Y-%m-%d").to_string()).await.ok();
                        }
                        engine_recur.notify(engine::ChangeEvent::Tasks);
                    }
                }
            }
        }
    });

    // F1: Auto-archive completed tasks older than 90 days (daily)
    let engine_archive = engine.clone();
    let mut shutdown_rx_archive = shutdown_tx.subscribe();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(86400));
        interval.tick().await;
        loop {
            tokio::select! {
                _ = interval.tick() => {},
                _ = shutdown_rx_archive.changed() => break,
            }
            let days = engine_archive.get_config().await.auto_archive_days.max(1) as i64;
            let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).format("%Y-%m-%dT%H:%M:%S").to_string();
            if let Err(e) = sqlx::query("UPDATE tasks SET status = 'archived', updated_at = datetime('now') WHERE status = 'completed' AND updated_at < ? AND deleted_at IS NULL")
                .bind(&cutoff).execute(&engine_archive.pool).await {
                tracing::warn!("Auto-archive error: {}", e);
            }
            engine_archive.heartbeat("auto_archive").await;
        }
    });

    // O3: Orphaned attachment cleanup (daily)
    let pool_cleanup = engine.pool.clone();
    let mut shutdown_rx_cleanup = shutdown_tx.subscribe();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(86400));
        interval.tick().await; // Skip immediate first tick
        loop {
            tokio::select! {
                _ = interval.tick() => {},
                _ = shutdown_rx_cleanup.changed() => break,
            }
            match db::cleanup_orphaned_attachments(&pool_cleanup).await {
                Ok(n) if n > 0 => tracing::info!("Cleaned up {} orphaned attachment files", n),
                Err(e) => tracing::error!("Attachment cleanup error: {}", e),
                _ => {}
            }
        }
    });

    // Due date reminders (every 30 minutes)
    let engine_due = engine.clone();
    let mut shutdown_rx4 = shutdown_tx.subscribe();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1800));
        let mut notified: std::collections::HashSet<i64> = std::collections::HashSet::new();
        let mut last_date = String::new();
        loop {
            tokio::select! {
                _ = interval.tick() => {},
                _ = shutdown_rx4.changed() => break,
            }
            let today = chrono::Utc::now().naive_utc().format("%Y-%m-%d").to_string();
            // Reset notified set on new day
            if today != last_date { notified.clear(); last_date = today.clone(); }
            let tomorrow = (chrono::Utc::now().naive_utc() + chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
            let due_tasks = db::get_due_tasks(&engine_due.pool, &tomorrow).await.unwrap_or_default();
            for (id, title, due) in due_tasks {
                if notified.contains(&id) { continue; }
                let urgency = if due <= today { "overdue" } else { "due tomorrow" };
                notify::notify_due_task(&title, urgency);
                notified.insert(id);
            }
        }
    });

    let mut app = build_router(engine.clone());
    let swagger_enabled = std::env::var("POMODORO_SWAGGER").map_or(true, |v| v != "0" && v.to_lowercase() != "false");
    if swagger_enabled {
        app = app.merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()));
    }
    let app = app.layer(tower_http::trace::TraceLayer::new_for_http());

    let addr = format!("{}:{}", config.bind_address, config.bind_port);
    tracing::info!("HTTP server listening on {}", addr);
    if swagger_enabled { tracing::info!("Swagger UI: http://{}/swagger-ui/", addr); }

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let server = axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>());

    // Graceful shutdown on SIGTERM/SIGINT
    let engine_shutdown = engine.clone();
    let handle = server.with_graceful_shutdown(async move {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("Shutting down gracefully...");
        // Signal background tasks to stop
        let _ = shutdown_tx.send(true);
        // Flush running sessions
        if let Err(e) = db::recover_interrupted(&engine_shutdown.pool).await {
            tracing::error!("Error flushing sessions on shutdown: {}", e);
        }
    });
    handle.await?;

    Ok(())
}
