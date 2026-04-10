use crate::auth::{self, Claims};
use crate::db;
use crate::engine::{Engine, TimerPhase, ChangeEvent};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub type AppState = Arc<Engine>;

#[derive(Deserialize, utoipa::ToSchema)]
pub struct RegisterRequest { pub username: String, pub password: String }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct LoginRequest { pub username: String, pub password: String }
#[derive(Serialize, utoipa::ToSchema)]
pub struct AuthResponse { pub token: String, pub user_id: i64, pub username: String, pub role: String }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateTaskRequest { pub title: String, pub parent_id: Option<i64>, pub description: Option<String>, pub project: Option<String>, pub tags: Option<String>, pub priority: Option<i64>, pub estimated: Option<i64>, pub estimated_hours: Option<f64>, pub remaining_points: Option<f64>, pub due_date: Option<String> }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateTaskRequest { pub title: Option<String>, #[serde(default, deserialize_with = "deserialize_optional_nullable")] pub description: Option<Option<String>>, #[serde(default, deserialize_with = "deserialize_optional_nullable")] pub project: Option<Option<String>>, #[serde(default, deserialize_with = "deserialize_optional_nullable")] pub tags: Option<Option<String>>, pub priority: Option<i64>, pub estimated: Option<i64>, pub estimated_hours: Option<f64>, pub remaining_points: Option<f64>, #[serde(default, deserialize_with = "deserialize_optional_nullable")] pub due_date: Option<Option<String>>, pub status: Option<String>, pub sort_order: Option<i64>, pub parent_id: Option<Option<i64>>, pub expected_updated_at: Option<String> }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct StartRequest { pub task_id: Option<i64>, pub phase: Option<String> }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct AddCommentRequest { pub content: String, pub session_id: Option<i64> }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct HistoryQuery { pub from: Option<String>, pub to: Option<String> }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct StatsQuery { pub days: Option<i64> }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateRoleRequest { pub role: String }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateProfileRequest { pub username: Option<String>, pub password: Option<String> }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct AddTimeReportRequest { pub hours: f64, pub points: Option<f64>, pub description: Option<String> }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct AssignRequest { pub username: String }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateRoomRequest { pub name: String, pub room_type: Option<String>, pub estimation_unit: Option<String>, pub project: Option<String> }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct RoomRoleRequest { pub username: String, pub role: String }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct StartVotingRequest { pub task_id: i64 }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CastVoteRequest { pub value: f64 }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct AcceptEstimateRequest { pub value: f64 }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateSprintRequest { pub name: String, pub project: Option<String>, pub goal: Option<String>, pub start_date: Option<String>, pub end_date: Option<String> }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateSprintRequest { pub name: Option<String>, #[serde(default, deserialize_with = "deserialize_optional_nullable")] pub project: Option<Option<String>>, #[serde(default, deserialize_with = "deserialize_optional_nullable")] pub goal: Option<Option<String>>, pub status: Option<String>, #[serde(default, deserialize_with = "deserialize_optional_nullable")] pub start_date: Option<Option<String>>, #[serde(default, deserialize_with = "deserialize_optional_nullable")] pub end_date: Option<Option<String>>, pub expected_updated_at: Option<String> }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct AddSprintTasksRequest { pub task_ids: Vec<i64> }
#[derive(Deserialize)]
pub struct SprintQuery { pub status: Option<String>, pub project: Option<String> }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct LogBurnRequest { pub task_id: i64, pub points: Option<f64>, pub hours: Option<f64>, pub note: Option<String> }

type ApiResult<T> = Result<Json<T>, (StatusCode, String)>;
fn err(status: StatusCode, msg: impl ToString) -> (StatusCode, String) { (status, msg.to_string()) }
fn internal(e: impl ToString) -> (StatusCode, String) { err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()) }

fn deserialize_optional_nullable<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where D: serde::Deserializer<'de> {
    Ok(Some(Option::deserialize(deserializer)?))
}

fn is_owner_or_root(task_user_id: i64, claims: &Claims) -> bool {
    claims.user_id == task_user_id || claims.role == "root"
}

// --- Auth ---

#[utoipa::path(post, path = "/api/auth/register", request_body = RegisterRequest, responses((status = 200, body = AuthResponse)))]
pub async fn register(State(engine): State<AppState>, Json(req): Json<RegisterRequest>) -> ApiResult<AuthResponse> {
    if req.password.len() < 6 { return Err(err(StatusCode::BAD_REQUEST, "Password must be at least 6 characters")); }
    let hash = bcrypt::hash(&req.password, 12).map_err(internal)?;
    let user = db::create_user(&engine.pool, &req.username, &hash, "user").await
        .map_err(|_| err(StatusCode::CONFLICT, "Username already taken"))?;
    let token = auth::create_token(user.id, &user.username, &user.role).map_err(internal)?;
    Ok(Json(AuthResponse { token, user_id: user.id, username: user.username, role: user.role }))
}

#[utoipa::path(post, path = "/api/auth/login", request_body = LoginRequest, responses((status = 200, body = AuthResponse)))]
pub async fn login(State(engine): State<AppState>, Json(req): Json<LoginRequest>) -> ApiResult<AuthResponse> {
    let user = db::get_user_by_username(&engine.pool, &req.username).await
        .map_err(|_| err(StatusCode::UNAUTHORIZED, "Invalid credentials"))?;
    if !bcrypt::verify(&req.password, &user.password_hash).unwrap_or(false) {
        return Err(err(StatusCode::UNAUTHORIZED, "Invalid credentials"));
    }
    let token = auth::create_token(user.id, &user.username, &user.role).map_err(internal)?;
    Ok(Json(AuthResponse { token, user_id: user.id, username: user.username, role: user.role }))
}

// --- Timer ---

#[utoipa::path(get, path = "/api/timer", responses((status = 200, body = crate::engine::EngineState)), security(("bearer" = [])))]
pub async fn get_state(State(engine): State<AppState>, _claims: Claims) -> ApiResult<crate::engine::EngineState> {
    Ok(Json(engine.get_state().await))
}

#[utoipa::path(post, path = "/api/timer/start", request_body = StartRequest, responses((status = 200, body = crate::engine::EngineState)), security(("bearer" = [])))]
pub async fn start(State(engine): State<AppState>, claims: Claims, Json(req): Json<StartRequest>) -> ApiResult<crate::engine::EngineState> {
    let phase = req.phase.as_deref().map(|s| match s { "short_break" => TimerPhase::ShortBreak, "long_break" => TimerPhase::LongBreak, _ => TimerPhase::Work });
    engine.start(claims.user_id, req.task_id, phase).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/timer/pause", responses((status = 200, body = crate::engine::EngineState)), security(("bearer" = [])))]
pub async fn pause(State(engine): State<AppState>, claims: Claims) -> ApiResult<crate::engine::EngineState> {
    let state = engine.get_state().await;
    if state.current_user_id != 0 && state.current_user_id != claims.user_id && claims.role != "root" {
        return Err(err(StatusCode::FORBIDDEN, "Timer owned by another user"));
    }
    engine.pause().await.map(Json).map_err(internal)
}
#[utoipa::path(post, path = "/api/timer/resume", responses((status = 200, body = crate::engine::EngineState)), security(("bearer" = [])))]
pub async fn resume(State(engine): State<AppState>, claims: Claims) -> ApiResult<crate::engine::EngineState> {
    let state = engine.get_state().await;
    if state.current_user_id != 0 && state.current_user_id != claims.user_id && claims.role != "root" {
        return Err(err(StatusCode::FORBIDDEN, "Timer owned by another user"));
    }
    engine.resume().await.map(Json).map_err(internal)
}
#[utoipa::path(post, path = "/api/timer/stop", responses((status = 200, body = crate::engine::EngineState)), security(("bearer" = [])))]
pub async fn stop(State(engine): State<AppState>, claims: Claims) -> ApiResult<crate::engine::EngineState> {
    let state = engine.get_state().await;
    if state.current_user_id != 0 && state.current_user_id != claims.user_id && claims.role != "root" {
        return Err(err(StatusCode::FORBIDDEN, "Timer owned by another user"));
    }
    engine.stop().await.map(Json).map_err(internal)
}
#[utoipa::path(post, path = "/api/timer/skip", responses((status = 200, body = crate::engine::EngineState)), security(("bearer" = [])))]
pub async fn skip(State(engine): State<AppState>, claims: Claims) -> ApiResult<crate::engine::EngineState> {
    let state = engine.get_state().await;
    if state.current_user_id != 0 && state.current_user_id != claims.user_id && claims.role != "root" {
        return Err(err(StatusCode::FORBIDDEN, "Timer owned by another user"));
    }
    engine.skip().await.map(Json).map_err(internal)
}


// --- Tasks ---

#[derive(Deserialize)]
pub struct TaskQuery { pub status: Option<String>, pub project: Option<String>, pub page: Option<i64>, pub per_page: Option<i64>, pub team_id: Option<i64> }

#[utoipa::path(get, path = "/api/tasks", responses((status = 200, body = Vec<db::Task>)), security(("bearer" = [])))]
pub async fn list_tasks(State(engine): State<AppState>, _claims: Claims, Query(q): Query<TaskQuery>) -> ApiResult<Vec<db::Task>> {
    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(5000).min(5000);
    let offset = (page - 1) * per_page;
    db::list_tasks_paged(&engine.pool, q.status.as_deref(), q.project.as_deref(), per_page, offset, q.team_id).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/tasks", request_body = CreateTaskRequest, responses((status = 201, body = db::Task)), security(("bearer" = [])))]
pub async fn create_task(State(engine): State<AppState>, claims: Claims, Json(req): Json<CreateTaskRequest>) -> Result<(StatusCode, Json<db::Task>), (StatusCode, String)> {
    let t = db::create_task(&engine.pool, claims.user_id, req.parent_id, &req.title, req.description.as_deref(), req.project.as_deref(), req.tags.as_deref(), req.priority.unwrap_or(3), req.estimated.unwrap_or(1), req.estimated_hours.unwrap_or(0.0), req.remaining_points.unwrap_or(0.0), req.due_date.as_deref())
        .await.map_err(internal)?;
    engine.notify(ChangeEvent::Tasks);
    Ok((StatusCode::CREATED, Json(t)))
}

#[utoipa::path(get, path = "/api/tasks/{id}", responses((status = 200, body = db::TaskDetail)), security(("bearer" = [])))]
pub async fn get_task_detail(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<db::TaskDetail> {
    db::get_task_detail(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(put, path = "/api/tasks/{id}", request_body = UpdateTaskRequest, responses((status = 200, body = db::Task)), security(("bearer" = [])))]
pub async fn update_task(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<UpdateTaskRequest>) -> ApiResult<db::Task> {
    let task = db::get_task(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    if !is_owner_or_root(task.user_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    if let Some(ref expected) = req.expected_updated_at {
        if *expected != task.updated_at {
            return Err(err(StatusCode::CONFLICT, "Task was modified by another user. Please refresh and try again."));
        }
    }
    let t = db::update_task(&engine.pool, id, req.title.as_deref(),
        req.description.as_ref().map(|o| o.as_deref()),
        req.project.as_ref().map(|o| o.as_deref()),
        req.tags.as_ref().map(|o| o.as_deref()),
        req.priority, req.estimated, req.estimated_hours, req.remaining_points,
        req.due_date.as_ref().map(|o| o.as_deref()),
        req.status.as_deref(), req.sort_order, req.parent_id)
        .await.map_err(internal)?;
    engine.notify(ChangeEvent::Tasks);
    Ok(Json(t))
}

#[utoipa::path(delete, path = "/api/tasks/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_task(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<impl IntoResponse, (StatusCode, String)> {
    let task = db::get_task(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    if !is_owner_or_root(task.user_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    db::delete_task(&engine.pool, id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Tasks);
    Ok(StatusCode::NO_CONTENT)
}

// --- Comments ---

#[utoipa::path(get, path = "/api/tasks/{id}/comments", responses((status = 200, body = Vec<db::Comment>)), security(("bearer" = [])))]
pub async fn list_comments(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<db::Comment>> {
    db::list_comments(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/tasks/{id}/comments", request_body = AddCommentRequest, responses((status = 201, body = db::Comment)), security(("bearer" = [])))]
pub async fn add_comment(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<AddCommentRequest>) -> Result<(StatusCode, Json<db::Comment>), (StatusCode, String)> {
    db::add_comment(&engine.pool, claims.user_id, id, req.session_id, &req.content)
        .await.map(|c| (StatusCode::CREATED, Json(c))).map_err(internal)
}

#[utoipa::path(delete, path = "/api/comments/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_comment(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, (StatusCode, String)> {
    let comment = db::get_comment(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Comment not found"))?;
    if !is_owner_or_root(comment.user_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    db::delete_comment(&engine.pool, id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

// --- Task Time/Burns ---

#[utoipa::path(get, path = "/api/tasks/{id}/time", responses((status = 200, body = Vec<db::BurnEntry>)), security(("bearer" = [])))]
pub async fn list_time_reports(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<db::BurnEntry>> {
    db::list_task_burns(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/tasks/{id}/time", request_body = AddTimeReportRequest, responses((status = 201, body = db::BurnEntry)), security(("bearer" = [])))]
pub async fn add_time_report(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<AddTimeReportRequest>) -> Result<(StatusCode, Json<db::BurnEntry>), (StatusCode, String)> {
    let sprint_id = db::find_task_active_sprint(&engine.pool, id).await.unwrap_or(None);
    let b = db::log_burn(&engine.pool, sprint_id, id, None, claims.user_id, req.points.unwrap_or(0.0), req.hours, "time_report", req.description.as_deref())
        .await.map_err(internal)?;
    engine.notify(ChangeEvent::Tasks);
    Ok((StatusCode::CREATED, Json(b)))
}

#[utoipa::path(get, path = "/api/tasks/{id}/burn-total", responses((status = 200, body = db::BurnTotal)), security(("bearer" = [])))]
pub async fn get_task_burn_total(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<db::BurnTotal> {
    db::get_task_burn_total(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(get, path = "/api/tasks/{id}/burn-users", responses((status = 200, body = Vec<String>)), security(("bearer" = [])))]
pub async fn get_task_burn_users(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<String>> {
    db::get_task_burn_users(&engine.pool, id).await.map(Json).map_err(internal)
}

// --- Assignees ---

#[utoipa::path(get, path = "/api/tasks/{id}/assignees", responses((status = 200, body = Vec<String>)), security(("bearer" = [])))]
pub async fn list_assignees(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<String>> {
    db::list_assignees(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/tasks/{id}/assignees", request_body = AssignRequest, responses((status = 200)), security(("bearer" = [])))]
pub async fn add_assignee(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>, Json(req): Json<AssignRequest>) -> Result<StatusCode, (StatusCode, String)> {
    let uid = db::get_user_id_by_username(&engine.pool, &req.username).await.map_err(|_| err(StatusCode::NOT_FOUND, "User not found"))?;
    db::add_assignee(&engine.pool, id, uid).await.map_err(internal)?;
    Ok(StatusCode::OK)
}

#[utoipa::path(delete, path = "/api/tasks/{id}/assignees/{username}", responses((status = 204)), security(("bearer" = [])))]
pub async fn remove_assignee(State(engine): State<AppState>, claims: Claims, Path((id, username)): Path<(i64, String)>) -> Result<StatusCode, (StatusCode, String)> {
    let task = db::get_task(&engine.pool, id).await.map_err(internal)?;
    if !is_owner_or_root(task.user_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    let uid = db::get_user_id_by_username(&engine.pool, &username).await.map_err(|_| err(StatusCode::NOT_FOUND, "User not found"))?;
    db::remove_assignee(&engine.pool, id, uid).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

// --- History & Stats ---

#[utoipa::path(get, path = "/api/history", responses((status = 200, body = Vec<db::SessionWithPath>)), security(("bearer" = [])))]
pub async fn get_history(State(engine): State<AppState>, _claims: Claims, Query(q): Query<HistoryQuery>) -> ApiResult<Vec<db::SessionWithPath>> {
    let from = q.from.unwrap_or_else(|| "2000-01-01T00:00:00".to_string());
    let to = q.to.unwrap_or_else(|| "2099-12-31T23:59:59".to_string());
    db::get_history(&engine.pool, &from, &to).await.map(Json).map_err(internal)
}

#[utoipa::path(get, path = "/api/stats", responses((status = 200, body = Vec<db::DayStat>)), security(("bearer" = [])))]
pub async fn get_stats(State(engine): State<AppState>, _claims: Claims, Query(q): Query<StatsQuery>) -> ApiResult<Vec<db::DayStat>> {
    db::get_day_stats(&engine.pool, q.days.unwrap_or(30)).await.map(Json).map_err(internal)
}

// --- Config ---

#[utoipa::path(get, path = "/api/config", responses((status = 200, body = crate::config::Config)), security(("bearer" = [])))]
pub async fn get_config(State(engine): State<AppState>, claims: Claims) -> ApiResult<crate::config::Config> {
    let mut cfg = engine.config.lock().await.clone();
    // Overlay per-user config if exists
    if let Ok(Some(uc)) = db::get_user_config(&engine.pool, claims.user_id).await {
        if let Some(v) = uc.work_duration_min { cfg.work_duration_min = v as u32; }
        if let Some(v) = uc.short_break_min { cfg.short_break_min = v as u32; }
        if let Some(v) = uc.long_break_min { cfg.long_break_min = v as u32; }
        if let Some(v) = uc.long_break_interval { cfg.long_break_interval = v as u32; }
        if let Some(v) = uc.auto_start_breaks { cfg.auto_start_breaks = v != 0; }
        if let Some(v) = uc.auto_start_work { cfg.auto_start_work = v != 0; }
        if let Some(v) = uc.daily_goal { cfg.daily_goal = v as u32; }
    }
    Ok(Json(cfg))
}

#[utoipa::path(put, path = "/api/config", request_body = crate::config::Config, responses((status = 200, body = crate::config::Config)), security(("bearer" = [])))]
pub async fn update_config(State(engine): State<AppState>, claims: Claims, Json(cfg): Json<crate::config::Config>) -> ApiResult<crate::config::Config> {
    // Save per-user overrides
    let uc = db::UserConfig {
        user_id: claims.user_id,
        work_duration_min: Some(cfg.work_duration_min as i64),
        short_break_min: Some(cfg.short_break_min as i64),
        long_break_min: Some(cfg.long_break_min as i64),
        long_break_interval: Some(cfg.long_break_interval as i64),
        auto_start_breaks: Some(if cfg.auto_start_breaks { 1 } else { 0 }),
        auto_start_work: Some(if cfg.auto_start_work { 1 } else { 0 }),
        daily_goal: Some(cfg.daily_goal as i64),
    };
    db::set_user_config(&engine.pool, claims.user_id, &uc).await.map_err(internal)?;
    // Root also updates global config
    if claims.role == "root" {
        cfg.save().map_err(internal)?;
        *engine.config.lock().await = cfg.clone();
    }
    Ok(Json(cfg))
}

// --- Profile ---

#[utoipa::path(put, path = "/api/profile", request_body = UpdateProfileRequest, responses((status = 200, body = AuthResponse)), security(("bearer" = [])))]
pub async fn update_profile(State(engine): State<AppState>, claims: Claims, Json(req): Json<UpdateProfileRequest>) -> ApiResult<AuthResponse> {
    if let Some(ref u) = req.username {
        db::update_username(&engine.pool, claims.user_id, u).await
            .map_err(|e| if e.to_string().contains("already taken") { err(StatusCode::CONFLICT, "Username already taken") } else { internal(e) })?;
    }
    if let Some(ref p) = req.password {
        if p.len() < 6 { return Err(err(StatusCode::BAD_REQUEST, "Password must be at least 6 characters")); }
        let hash = bcrypt::hash(p, 12).map_err(internal)?;
        db::update_user_password(&engine.pool, claims.user_id, &hash).await.map_err(internal)?;
    }
    let user = db::get_user(&engine.pool, claims.user_id).await.map_err(internal)?;
    let token = auth::create_token(user.id, &user.username, &user.role).map_err(internal)?;
    Ok(Json(AuthResponse { token, user_id: user.id, username: user.username, role: user.role }))
}

// --- Admin ---

#[utoipa::path(get, path = "/api/admin/users", responses((status = 200, body = Vec<db::User>)), security(("bearer" = [])))]
pub async fn list_users(State(engine): State<AppState>, claims: Claims) -> Result<Json<Vec<db::User>>, (StatusCode, String)> {
    if claims.role != "root" { return Err(err(StatusCode::FORBIDDEN, "Root only")); }
    db::list_users(&engine.pool).await.map(Json).map_err(internal)
}

#[utoipa::path(put, path = "/api/admin/users/{id}/role", request_body = UpdateRoleRequest, responses((status = 200, body = db::User)), security(("bearer" = [])))]
pub async fn update_user_role(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<UpdateRoleRequest>) -> ApiResult<db::User> {
    if claims.role != "root" { return Err(err(StatusCode::FORBIDDEN, "Root only")); }
    db::update_user_role(&engine.pool, id, &req.role).await.map(Json).map_err(internal)
}

#[utoipa::path(delete, path = "/api/admin/users/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_user(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, (StatusCode, String)> {
    if claims.role != "root" { return Err(err(StatusCode::FORBIDDEN, "Root only")); }
    if claims.user_id == id { return Err(err(StatusCode::BAD_REQUEST, "Cannot delete yourself")); }
    db::delete_user(&engine.pool, id).await.map_err(|e| err(StatusCode::BAD_REQUEST, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

// --- Task votes ---

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

pub async fn get_all_burn_totals(State(engine): State<AppState>, _claims: Claims) -> ApiResult<Vec<BurnTotalEntry>> {
    let totals = db::get_all_burn_totals(&engine.pool).await.map_err(internal)?;
    Ok(Json(totals.into_iter().map(|(tid, bt)| BurnTotalEntry { task_id: tid, total_points: bt.total_points, total_hours: bt.total_hours, count: bt.count }).collect()))
}

pub async fn get_all_assignees(State(engine): State<AppState>, _claims: Claims) -> ApiResult<Vec<db::TaskAssignee>> {
    db::get_all_assignees(&engine.pool).await.map(Json).map_err(internal)
}

#[derive(Serialize)]
pub struct TasksFullResponse {
    pub tasks: Vec<db::Task>,
    pub task_sprints: Vec<db::TaskSprintInfo>,
    pub burn_totals: Vec<BurnTotalEntry>,
    pub assignees: Vec<db::TaskAssignee>,
}

pub async fn get_tasks_full(State(engine): State<AppState>, _claims: Claims, headers: axum::http::HeaderMap) -> Result<axum::response::Response, (StatusCode, String)> {
    // ETag: hash of max updated_at
    let (max_updated,): (String,) = sqlx::query_as("SELECT COALESCE(MAX(updated_at), '') FROM tasks")
        .fetch_one(&engine.pool).await.map_err(internal)?;
    let (task_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tasks")
        .fetch_one(&engine.pool).await.map_err(internal)?;
    let etag = format!("\"{}:{}\"", max_updated, task_count);

    if let Some(if_none_match) = headers.get("if-none-match").and_then(|v| v.to_str().ok()) {
        if if_none_match == etag {
            return Ok(axum::response::Response::builder()
                .status(StatusCode::NOT_MODIFIED)
                .header("etag", &etag)
                .body(axum::body::Body::empty()).unwrap());
        }
    }

    let (tasks, task_sprints, burn_totals_raw, assignees) = tokio::join!(
        db::list_tasks(&engine.pool, None, None),
        db::get_all_task_sprints(&engine.pool),
        db::get_all_burn_totals(&engine.pool),
        db::get_all_assignees(&engine.pool),
    );
    let burn_totals: Vec<BurnTotalEntry> = burn_totals_raw.map_err(internal)?.into_iter()
        .map(|(tid, bt)| BurnTotalEntry { task_id: tid, total_points: bt.total_points, total_hours: bt.total_hours, count: bt.count })
        .collect();
    let resp = TasksFullResponse {
        tasks: tasks.map_err(internal)?,
        task_sprints: task_sprints.map_err(internal)?,
        burn_totals,
        assignees: assignees.map_err(internal)?,
    };
    let body = serde_json::to_vec(&resp).map_err(internal)?;
    Ok(axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .header("etag", &etag)
        .body(axum::body::Body::from(body)).unwrap())
}

#[derive(Deserialize)]
pub struct SseQuery { pub token: Option<String> }

pub async fn sse_timer(State(engine): State<AppState>, Query(q): Query<SseQuery>) -> Result<axum::response::Sse<impl futures::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>>, (StatusCode, String)> {
    if let Some(token) = &q.token {
        auth::verify_token(token).map_err(|_| err(StatusCode::UNAUTHORIZED, "Invalid token"))?;
    } else {
        return Err(err(StatusCode::UNAUTHORIZED, "Token required"));
    }
    let mut timer_rx = engine.tx.subscribe();
    let mut change_rx = engine.changes.subscribe();
    let stream = async_stream::stream! {
        // Send initial timer state
        {
            let state = timer_rx.borrow().clone();
            if let Ok(json) = serde_json::to_string(&state) {
                yield Ok(axum::response::sse::Event::default().event("timer").data(json));
            }
        }
        loop {
            tokio::select! {
                Ok(()) = timer_rx.changed() => {
                    let state = timer_rx.borrow().clone();
                    if let Ok(json) = serde_json::to_string(&state) {
                        yield Ok(axum::response::sse::Event::default().event("timer").data(json));
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

#[utoipa::path(get, path = "/api/rooms", responses((status = 200, body = Vec<db::Room>)), security(("bearer" = [])))]
pub async fn list_rooms(State(engine): State<AppState>, _claims: Claims) -> ApiResult<Vec<db::Room>> {
    db::list_rooms(&engine.pool).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/rooms", request_body = CreateRoomRequest, responses((status = 201, body = db::Room)), security(("bearer" = [])))]
pub async fn create_room(State(engine): State<AppState>, claims: Claims, Json(req): Json<CreateRoomRequest>) -> Result<(StatusCode, Json<db::Room>), (StatusCode, String)> {
    let r = db::create_room(&engine.pool, &req.name, req.room_type.as_deref().unwrap_or("estimation"), req.estimation_unit.as_deref().unwrap_or("points"), req.project.as_deref(), claims.user_id)
        .await.map_err(internal)?;
    engine.notify(ChangeEvent::Rooms);
    Ok((StatusCode::CREATED, Json(r)))
}

#[utoipa::path(get, path = "/api/rooms/{id}", responses((status = 200, body = db::RoomState)), security(("bearer" = [])))]
pub async fn get_room_state(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<db::RoomState> {
    db::get_room_state(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(delete, path = "/api/rooms/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_room(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, (StatusCode, String)> {
    let room = db::get_room(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Room not found"))?;
    if !is_owner_or_root(room.creator_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    db::delete_room(&engine.pool, id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Rooms);
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/rooms/{id}/join", responses((status = 200)), security(("bearer" = [])))]
pub async fn join_room(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, (StatusCode, String)> {
    db::join_room(&engine.pool, id, claims.user_id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Rooms);
    Ok(StatusCode::OK)
}

#[utoipa::path(post, path = "/api/rooms/{id}/leave", responses((status = 200)), security(("bearer" = [])))]
pub async fn leave_room(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, (StatusCode, String)> {
    db::leave_room(&engine.pool, id, claims.user_id).await.map_err(internal)?;
    Ok(StatusCode::OK)
}

#[utoipa::path(delete, path = "/api/rooms/{id}/members/{username}", responses((status = 204)), security(("bearer" = [])))]
pub async fn kick_member(State(engine): State<AppState>, claims: Claims, Path((id, username)): Path<(i64, String)>) -> Result<StatusCode, (StatusCode, String)> {
    if !db::is_room_admin(&engine.pool, id, claims.user_id).await.map_err(internal)? && claims.role != "root" {
        return Err(err(StatusCode::FORBIDDEN, "Admin only"));
    }
    let uid = db::get_user_id_by_username(&engine.pool, &username).await.map_err(|_| err(StatusCode::NOT_FOUND, "User not found"))?;
    db::leave_room(&engine.pool, id, uid).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(put, path = "/api/rooms/{id}/role", request_body = RoomRoleRequest, responses((status = 200)), security(("bearer" = [])))]
pub async fn set_room_role(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<RoomRoleRequest>) -> Result<StatusCode, (StatusCode, String)> {
    if !db::is_room_admin(&engine.pool, id, claims.user_id).await.map_err(internal)? && claims.role != "root" {
        return Err(err(StatusCode::FORBIDDEN, "Admin only"));
    }
    let uid = db::get_user_id_by_username(&engine.pool, &req.username).await.map_err(|_| err(StatusCode::NOT_FOUND, "User not found"))?;
    db::set_room_member_role(&engine.pool, id, uid, &req.role).await.map_err(internal)?;
    Ok(StatusCode::OK)
}

#[utoipa::path(post, path = "/api/rooms/{id}/start-voting", request_body = StartVotingRequest, responses((status = 200, body = db::Room)), security(("bearer" = [])))]
pub async fn start_voting(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<StartVotingRequest>) -> ApiResult<db::Room> {
    if !db::is_room_admin(&engine.pool, id, claims.user_id).await.map_err(internal)? && claims.role != "root" {
        return Err(err(StatusCode::FORBIDDEN, "Admin only"));
    }
    db::start_voting(&engine.pool, id, req.task_id).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/rooms/{id}/vote", request_body = CastVoteRequest, responses((status = 200)), security(("bearer" = [])))]
pub async fn cast_vote(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<CastVoteRequest>) -> Result<StatusCode, (StatusCode, String)> {
    let room = db::get_room(&engine.pool, id).await.map_err(internal)?;
    let task_id = room.current_task_id.ok_or_else(|| err(StatusCode::BAD_REQUEST, "No active vote"))?;
    db::cast_vote(&engine.pool, id, task_id, claims.user_id, req.value).await.map_err(internal)?;
    engine.notify(ChangeEvent::Rooms);
    Ok(StatusCode::OK)
}

#[utoipa::path(post, path = "/api/rooms/{id}/reveal", responses((status = 200, body = db::Room)), security(("bearer" = [])))]
pub async fn reveal_votes(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> ApiResult<db::Room> {
    if !db::is_room_admin(&engine.pool, id, claims.user_id).await.map_err(internal)? && claims.role != "root" {
        return Err(err(StatusCode::FORBIDDEN, "Admin only"));
    }
    let r = db::reveal_votes(&engine.pool, id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Rooms);
    Ok(Json(r))
}

#[utoipa::path(post, path = "/api/rooms/{id}/accept", request_body = AcceptEstimateRequest, responses((status = 200, body = db::Task)), security(("bearer" = [])))]
pub async fn accept_estimate(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<AcceptEstimateRequest>) -> ApiResult<db::Task> {
    if !db::is_room_admin(&engine.pool, id, claims.user_id).await.map_err(internal)? && claims.role != "root" {
        return Err(err(StatusCode::FORBIDDEN, "Admin only"));
    }
    let room = db::get_room(&engine.pool, id).await.map_err(internal)?;
    let task_id = room.current_task_id.ok_or_else(|| err(StatusCode::BAD_REQUEST, "No active vote"))?;
    let task = db::accept_estimate(&engine.pool, id, task_id, req.value, &room.estimation_unit).await.map_err(internal)?;
    // Auto-advance
    let state = db::get_room_state(&engine.pool, id).await.map_err(internal)?;
    let all_tasks = &state.tasks;
    let next = all_tasks.iter().filter(|t| t.status != "estimated" && t.id != task_id)
        .filter(|t| !all_tasks.iter().any(|c| c.parent_id == Some(t.id))).next();
    if let Some(next_task) = next { db::start_voting(&engine.pool, id, next_task.id).await.map_err(internal)?; }
    else { db::set_room_status(&engine.pool, id, "lobby").await.map_err(internal)?; }
    engine.notify(ChangeEvent::Rooms);
    engine.notify(ChangeEvent::Tasks);
    Ok(Json(task))
}

#[utoipa::path(post, path = "/api/rooms/{id}/close", responses((status = 200)), security(("bearer" = [])))]
pub async fn close_room(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, (StatusCode, String)> {
    if !db::is_room_admin(&engine.pool, id, claims.user_id).await.map_err(internal)? && claims.role != "root" {
        return Err(err(StatusCode::FORBIDDEN, "Admin only"));
    }
    db::set_room_status(&engine.pool, id, "closed").await.map_err(internal)?;
    engine.notify(ChangeEvent::Rooms);
    Ok(StatusCode::OK)
}


// --- Sprints ---

#[utoipa::path(get, path = "/api/sprints", responses((status = 200, body = Vec<db::Sprint>)), security(("bearer" = [])))]
pub async fn list_sprints(State(engine): State<AppState>, _claims: Claims, Query(q): Query<SprintQuery>) -> ApiResult<Vec<db::Sprint>> {
    db::list_sprints(&engine.pool, q.status.as_deref(), q.project.as_deref()).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/sprints", request_body = CreateSprintRequest, responses((status = 201, body = db::Sprint)), security(("bearer" = [])))]
pub async fn create_sprint(State(engine): State<AppState>, claims: Claims, Json(req): Json<CreateSprintRequest>) -> Result<(StatusCode, Json<db::Sprint>), (StatusCode, String)> {
    let s = db::create_sprint(&engine.pool, claims.user_id, &req.name, req.project.as_deref(), req.goal.as_deref(), req.start_date.as_deref(), req.end_date.as_deref())
        .await.map_err(internal)?;
    engine.notify(ChangeEvent::Sprints);
    Ok((StatusCode::CREATED, Json(s)))
}

#[utoipa::path(get, path = "/api/sprints/{id}", responses((status = 200, body = db::SprintDetail)), security(("bearer" = [])))]
pub async fn get_sprint_detail(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<db::SprintDetail> {
    db::get_sprint_detail(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(put, path = "/api/sprints/{id}", request_body = UpdateSprintRequest, responses((status = 200, body = db::Sprint)), security(("bearer" = [])))]
pub async fn update_sprint(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<UpdateSprintRequest>) -> ApiResult<db::Sprint> {
    let sprint = db::get_sprint(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Sprint not found"))?;
    if !is_owner_or_root(sprint.created_by_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    if let Some(ref expected) = req.expected_updated_at {
        if *expected != sprint.updated_at {
            return Err(err(StatusCode::CONFLICT, "Sprint was modified by another user. Please refresh and try again."));
        }
    }
    let s = db::update_sprint(&engine.pool, id, req.name.as_deref(),
        req.project.as_ref().map(|o| o.as_deref()),
        req.goal.as_ref().map(|o| o.as_deref()),
        req.status.as_deref(),
        req.start_date.as_ref().map(|o| o.as_deref()),
        req.end_date.as_ref().map(|o| o.as_deref()))
        .await.map_err(internal)?;
    engine.notify(ChangeEvent::Sprints);
    Ok(Json(s))
}

#[utoipa::path(delete, path = "/api/sprints/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_sprint(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, (StatusCode, String)> {
    let sprint = db::get_sprint(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Sprint not found"))?;
    if !is_owner_or_root(sprint.created_by_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    db::delete_sprint(&engine.pool, id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Sprints);
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/sprints/{id}/start", responses((status = 200, body = db::Sprint)), security(("bearer" = [])))]
pub async fn start_sprint(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> ApiResult<db::Sprint> {
    let sprint = db::get_sprint(&engine.pool, id).await.map_err(internal)?;
    if !is_owner_or_root(sprint.created_by_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    if sprint.status != "planning" { return Err(err(StatusCode::BAD_REQUEST, format!("Cannot start sprint in '{}' status", sprint.status))); }
    let s = db::update_sprint(&engine.pool, id, None, None, None, Some("active"), None, None).await.map_err(internal)?;
    let _ = db::snapshot_sprint(&engine.pool, id).await;
    engine.notify(ChangeEvent::Sprints);
    Ok(Json(s))
}

#[utoipa::path(post, path = "/api/sprints/{id}/complete", responses((status = 200, body = db::Sprint)), security(("bearer" = [])))]
pub async fn complete_sprint(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> ApiResult<db::Sprint> {
    let sprint = db::get_sprint(&engine.pool, id).await.map_err(internal)?;
    if !is_owner_or_root(sprint.created_by_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    if sprint.status != "active" { return Err(err(StatusCode::BAD_REQUEST, format!("Cannot complete sprint in '{}' status", sprint.status))); }
    let _ = db::snapshot_sprint(&engine.pool, id).await;
    let s = db::update_sprint(&engine.pool, id, None, None, None, Some("completed"), None, None).await.map_err(internal)?;
    engine.notify(ChangeEvent::Sprints);
    Ok(Json(s))
}

#[utoipa::path(get, path = "/api/sprints/{id}/tasks", responses((status = 200, body = Vec<db::Task>)), security(("bearer" = [])))]
pub async fn get_sprint_tasks(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<db::Task>> {
    db::get_sprint_tasks(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/sprints/{id}/tasks", request_body = AddSprintTasksRequest, responses((status = 200, body = Vec<db::SprintTask>)), security(("bearer" = [])))]
pub async fn add_sprint_tasks(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<AddSprintTasksRequest>) -> ApiResult<Vec<db::SprintTask>> {
    let result = db::add_sprint_tasks(&engine.pool, id, &req.task_ids, claims.user_id).await.map_err(internal)?;
    if db::get_sprint(&engine.pool, id).await.map(|s| s.status == "active").unwrap_or(false) {
        let _ = db::snapshot_sprint(&engine.pool, id).await;
    }
    engine.notify(ChangeEvent::Sprints);
    Ok(Json(result))
}

#[utoipa::path(delete, path = "/api/sprints/{id}/tasks/{task_id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn remove_sprint_task(State(engine): State<AppState>, _claims: Claims, Path((id, task_id)): Path<(i64, i64)>) -> Result<StatusCode, (StatusCode, String)> {
    db::remove_sprint_task(&engine.pool, id, task_id).await.map_err(internal)?;
    if db::get_sprint(&engine.pool, id).await.map(|s| s.status == "active").unwrap_or(false) {
        let _ = db::snapshot_sprint(&engine.pool, id).await;
    }
    engine.notify(ChangeEvent::Sprints);
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/sprints/{id}/burndown", responses((status = 200, body = Vec<db::SprintDailyStat>)), security(("bearer" = [])))]
pub async fn get_sprint_burndown(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<db::SprintDailyStat>> {
    db::get_sprint_burndown(&engine.pool, id).await.map(Json).map_err(internal)
}

pub async fn get_global_burndown(State(engine): State<AppState>, _claims: Claims) -> ApiResult<Vec<db::SprintDailyStat>> {
    db::get_global_burndown(&engine.pool).await.map(Json).map_err(internal)
}

// --- Epic Groups ---

pub async fn list_epic_groups(State(engine): State<AppState>, _claims: Claims) -> ApiResult<Vec<db::EpicGroup>> {
    db::list_epic_groups(&engine.pool).await.map(Json).map_err(internal)
}

pub async fn create_epic_group(State(engine): State<AppState>, claims: Claims, Json(req): Json<CreateEpicGroupRequest>) -> Result<(StatusCode, Json<db::EpicGroup>), (StatusCode, String)> {
    let g = db::create_epic_group(&engine.pool, &req.name, claims.user_id).await.map_err(internal)?;
    Ok((StatusCode::CREATED, Json(g)))
}

#[derive(Deserialize)]
pub struct CreateEpicGroupRequest { pub name: String }

#[derive(Deserialize)]
pub struct EpicGroupTasksRequest { pub task_ids: Vec<i64> }

pub async fn get_epic_group(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<db::EpicGroupDetail> {
    db::get_epic_group_detail(&engine.pool, id).await.map(Json).map_err(internal)
}

pub async fn delete_epic_group(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, (StatusCode, String)> {
    db::delete_epic_group(&engine.pool, id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn add_epic_group_tasks(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>, Json(req): Json<EpicGroupTasksRequest>) -> Result<StatusCode, (StatusCode, String)> {
    for tid in req.task_ids { db::add_epic_group_task(&engine.pool, id, tid).await.map_err(internal)?; }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn remove_epic_group_task(State(engine): State<AppState>, _claims: Claims, Path((id, task_id)): Path<(i64, i64)>) -> Result<StatusCode, (StatusCode, String)> {
    db::remove_epic_group_task(&engine.pool, id, task_id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn snapshot_epic_group(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, (StatusCode, String)> {
    db::snapshot_epic_group(&engine.pool, id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

// --- Sprint Root Tasks ---

pub async fn get_sprint_root_tasks(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<i64>> {
    db::get_sprint_root_tasks(&engine.pool, id).await.map(Json).map_err(internal)
}

pub async fn add_sprint_root_tasks(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>, Json(req): Json<EpicGroupTasksRequest>) -> Result<StatusCode, (StatusCode, String)> {
    for tid in req.task_ids { db::add_sprint_root_task(&engine.pool, id, tid).await.map_err(internal)?; }
    engine.notify(ChangeEvent::Sprints);
    Ok(StatusCode::NO_CONTENT)
}

pub async fn remove_sprint_root_task(State(engine): State<AppState>, _claims: Claims, Path((id, task_id)): Path<(i64, i64)>) -> Result<StatusCode, (StatusCode, String)> {
    db::remove_sprint_root_task(&engine.pool, id, task_id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Sprints);
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_sprint_scope(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<i64>> {
    let roots = db::get_sprint_root_tasks(&engine.pool, id).await.map_err(internal)?;
    if roots.is_empty() { return Ok(Json(vec![])); }
    db::get_descendant_ids(&engine.pool, &roots).await.map(Json).map_err(internal)
}

// --- Teams ---

pub async fn list_teams(State(engine): State<AppState>, _claims: Claims) -> ApiResult<Vec<db::Team>> {
    db::list_teams(&engine.pool).await.map(Json).map_err(internal)
}

#[derive(Deserialize)]
pub struct CreateTeamRequest { pub name: String }

pub async fn create_team(State(engine): State<AppState>, claims: Claims, Json(req): Json<CreateTeamRequest>) -> Result<(StatusCode, Json<db::Team>), (StatusCode, String)> {
    let team = db::create_team(&engine.pool, &req.name).await.map_err(internal)?;
    db::add_team_member(&engine.pool, team.id, claims.user_id, "admin").await.map_err(internal)?;
    Ok((StatusCode::CREATED, Json(team)))
}

pub async fn get_team(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<db::TeamDetail> {
    db::get_team_detail(&engine.pool, id).await.map(Json).map_err(internal)
}

pub async fn delete_team(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, (StatusCode, String)> {
    if claims.role != "root" { return Err(err(StatusCode::FORBIDDEN, "Only root can delete teams")); }
    db::delete_team(&engine.pool, id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct TeamMemberRequest { pub user_id: i64, #[serde(default = "default_member_role")] pub role: String }
fn default_member_role() -> String { "member".to_string() }

pub async fn add_team_member(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>, Json(req): Json<TeamMemberRequest>) -> Result<StatusCode, (StatusCode, String)> {
    db::add_team_member(&engine.pool, id, req.user_id, &req.role).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn remove_team_member(State(engine): State<AppState>, _claims: Claims, Path((id, user_id)): Path<(i64, i64)>) -> Result<StatusCode, (StatusCode, String)> {
    db::remove_team_member(&engine.pool, id, user_id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_my_teams(State(engine): State<AppState>, claims: Claims) -> ApiResult<Vec<db::Team>> {
    db::get_user_teams(&engine.pool, claims.user_id).await.map(Json).map_err(internal)
}

pub async fn add_team_root_tasks(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>, Json(req): Json<EpicGroupTasksRequest>) -> Result<StatusCode, (StatusCode, String)> {
    for tid in req.task_ids { db::add_team_root_task(&engine.pool, id, tid).await.map_err(internal)?; }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn remove_team_root_task(State(engine): State<AppState>, _claims: Claims, Path((id, task_id)): Path<(i64, i64)>) -> Result<StatusCode, (StatusCode, String)> {
    db::remove_team_root_task(&engine.pool, id, task_id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_team_scope(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<i64>> {
    let detail = db::get_team_detail(&engine.pool, id).await.map_err(internal)?;
    if detail.root_task_ids.is_empty() { return Ok(Json(vec![])); }
    db::get_descendant_ids(&engine.pool, &detail.root_task_ids).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/sprints/{id}/snapshot", responses((status = 200, body = db::SprintDailyStat)), security(("bearer" = [])))]
pub async fn snapshot_sprint(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<db::SprintDailyStat> {
    db::snapshot_sprint(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(get, path = "/api/sprints/{id}/board", responses((status = 200, body = db::SprintBoard)), security(("bearer" = [])))]
pub async fn get_sprint_board(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<db::SprintBoard> {
    db::get_sprint_board(&engine.pool, id).await.map(Json).map_err(internal)
}

// --- Burn log ---

#[utoipa::path(post, path = "/api/sprints/{id}/burn", request_body = LogBurnRequest, responses((status = 201, body = db::BurnEntry)), security(("bearer" = [])))]
pub async fn log_burn(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<LogBurnRequest>) -> Result<(StatusCode, Json<db::BurnEntry>), (StatusCode, String)> {
    let b = db::log_burn(&engine.pool, Some(id), req.task_id, None, claims.user_id, req.points.unwrap_or(0.0), req.hours.unwrap_or(0.0), "manual", req.note.as_deref())
        .await.map_err(internal)?;
    engine.notify(ChangeEvent::Sprints);
    Ok((StatusCode::CREATED, Json(b)))
}

#[utoipa::path(get, path = "/api/sprints/{id}/burns", responses((status = 200, body = Vec<db::BurnEntry>)), security(("bearer" = [])))]
pub async fn list_burns(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<db::BurnEntry>> {
    db::list_burns(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(delete, path = "/api/sprints/{id}/burns/{burn_id}", responses((status = 200, body = db::BurnEntry)), security(("bearer" = [])))]
pub async fn cancel_burn(State(engine): State<AppState>, claims: Claims, Path((_id, burn_id)): Path<(i64, i64)>) -> ApiResult<db::BurnEntry> {
    let burn = db::get_burn(&engine.pool, burn_id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Burn not found"))?;
    if burn.cancelled != 0 { return Err(err(StatusCode::BAD_REQUEST, "Burn already cancelled")); }
    if burn.user_id != claims.user_id && claims.role != "root" {
        return Err(err(StatusCode::FORBIDDEN, "Not owner"));
    }
    let b = db::cancel_burn(&engine.pool, burn_id, claims.user_id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Sprints);
    Ok(Json(b))
}

#[utoipa::path(get, path = "/api/sprints/{id}/burn-summary", responses((status = 200, body = Vec<db::BurnSummaryEntry>)), security(("bearer" = [])))]
pub async fn get_burn_summary(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<db::BurnSummaryEntry>> {
    db::get_burn_summary(&engine.pool, id).await.map(Json).map_err(internal)
}
