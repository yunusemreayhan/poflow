use pomodoro_daemon::{config, db, engine, notify, routes, build_router};

use anyhow::Result;
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
    ),
    components(schemas(
        db::Task, db::Session, db::Comment, db::User, db::TaskDetail, db::SessionWithPath, db::DayStat, db::TaskAssignee,
        db::Room, db::RoomMember, db::RoomVote, db::RoomState, db::RoomVoteView, db::VoteResult,
        db::Sprint, db::SprintTask, db::SprintDailyStat, db::SprintDetail, db::SprintBoard, db::TaskSprintInfo,
        db::BurnEntry, db::BurnSummaryEntry, db::BurnTotal,
        engine::EngineState, engine::TimerPhase, engine::TimerStatus,
        config::Config,
        routes::RegisterRequest, routes::LoginRequest, routes::AuthResponse,
        routes::CreateTaskRequest, routes::UpdateTaskRequest, routes::StartRequest,
        routes::AddCommentRequest, routes::HistoryQuery, routes::StatsQuery, routes::UpdateRoleRequest,
        routes::UpdateProfileRequest, routes::AddTimeReportRequest, routes::AssignRequest,
        routes::CreateRoomRequest, routes::RoomRoleRequest, routes::StartVotingRequest, routes::CastVoteRequest, routes::AcceptEstimateRequest,
        routes::CreateSprintRequest, routes::UpdateSprintRequest, routes::AddSprintTasksRequest,
        routes::LogBurnRequest,
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
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("pomodoro_daemon=info".parse()?))
        .init();

    tracing::info!("Pomodoro daemon starting...");

    let config = config::Config::load()?;
    let pool = db::connect().await?;

    let interrupted = db::recover_interrupted(&pool).await?;
    if !interrupted.is_empty() {
        tracing::warn!("Recovered {} interrupted sessions", interrupted.len());
    }

    let engine = Arc::new(engine::Engine::new(pool, config.clone()).await);

    // Tick loop
    let engine_tick = engine.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        let mut last_date = chrono::Utc::now().naive_utc().format("%Y-%m-%d").to_string();
        loop {
            interval.tick().await;
            // Midnight reset: refresh daily_completed from DB
            let today = chrono::Utc::now().naive_utc().format("%Y-%m-%d").to_string();
            if today != last_date {
                last_date = today;
                if let Ok(count) = db::get_today_completed(&engine_tick.pool).await {
                    let mut state = engine_tick.state.lock().await;
                    state.daily_completed = count;
                }
            }
            match engine_tick.tick().await {
                Ok(Some(state)) => { notify::notify_session_complete(state.phase, state.session_count); }
                Ok(None) => {}
                Err(e) => tracing::error!("Tick error: {}", e),
            }
        }
    });

    // Sprint burndown snapshot (hourly)
    let engine_snap = engine.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            if let Err(e) = db::snapshot_active_sprints(&engine_snap.pool).await {
                tracing::error!("Sprint snapshot error: {}", e);
            }
            if let Err(e) = db::snapshot_all_epic_groups(&engine_snap.pool).await {
                tracing::error!("Epic snapshot error: {}", e);
            }
        }
    });

    let app = build_router(engine.clone())
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let addr = format!("{}:{}", config.bind_address, config.bind_port);
    tracing::info!("HTTP server listening on {}", addr);
    tracing::info!("Swagger UI: http://{}/swagger-ui/", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let server = axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>());

    // Graceful shutdown on SIGTERM/SIGINT
    let engine_shutdown = engine.clone();
    let handle = server.with_graceful_shutdown(async move {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("Shutting down gracefully...");
        // Flush running sessions
        if let Err(e) = db::recover_interrupted(&engine_shutdown.pool).await {
            tracing::error!("Error flushing sessions on shutdown: {}", e);
        }
    });
    handle.await?;

    Ok(())
}
