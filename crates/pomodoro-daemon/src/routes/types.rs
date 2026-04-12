use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

// --- Request types ---

#[derive(Deserialize, utoipa::ToSchema)]
pub struct RegisterRequest { pub username: String, pub password: String }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct LoginRequest { pub username: String, pub password: String }
#[derive(Serialize, utoipa::ToSchema)]
pub struct AuthResponse { pub token: String, pub refresh_token: String, pub user_id: i64, pub username: String, pub role: String }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateTaskRequest {
    pub title: String,
    pub parent_id: Option<i64>,
    pub description: Option<String>,
    pub project: Option<String>,
    pub tags: Option<String>,
    pub priority: Option<i64>,
    pub estimated: Option<i64>,
    pub estimated_hours: Option<f64>,
    pub remaining_points: Option<f64>,
    pub due_date: Option<String>,
}
#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub project: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub tags: Option<Option<String>>,
    pub priority: Option<i64>,
    pub estimated: Option<i64>,
    pub estimated_hours: Option<f64>,
    pub remaining_points: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub due_date: Option<Option<String>>,
    pub status: Option<String>,
    pub sort_order: Option<i64>,
    pub parent_id: Option<Option<i64>>,
    pub expected_updated_at: Option<String>,
}
#[derive(Deserialize, utoipa::ToSchema)]
pub struct StartRequest { pub task_id: Option<i64>, pub phase: Option<String> }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct AddCommentRequest { pub content: String, pub session_id: Option<i64> }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct HistoryQuery { pub from: Option<String>, pub to: Option<String>, pub user_id: Option<i64> }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct StatsQuery { pub days: Option<i64> }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateRoleRequest { pub role: String }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateProfileRequest { pub username: Option<String>, pub password: Option<String>, pub current_password: Option<String> }
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
pub struct UpdateSprintRequest {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub project: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub goal: Option<Option<String>>,
    /// Rejected with error — use /start or /complete endpoints instead
    pub status: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub start_date: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub end_date: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_nullable")]
    pub retro_notes: Option<Option<String>>,
    pub expected_updated_at: Option<String>,
}
#[derive(Deserialize, utoipa::ToSchema)]
pub struct AddSprintTasksRequest { pub task_ids: Vec<i64> }
#[derive(Deserialize)]
pub struct SprintQuery { pub status: Option<String>, pub project: Option<String> }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct LogBurnRequest { pub task_id: i64, pub points: Option<f64>, pub hours: Option<f64>, pub note: Option<String> }

// --- Error types ---

pub type ApiResult<T> = Result<Json<T>, ApiError>;

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ApiErrorBody {
    pub error: String,
    pub code: String,
}

pub struct ApiError {
    pub status: StatusCode,
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &str, msg: impl ToString) -> Self {
        Self { status, code: code.to_string(), message: msg.to_string() }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let body = ApiErrorBody { error: self.message, code: self.code };
        (self.status, Json(body)).into_response()
    }
}

pub fn err(status: StatusCode, msg: impl ToString) -> ApiError {
    let code = match status {
        StatusCode::BAD_REQUEST => "bad_request",
        StatusCode::UNAUTHORIZED => "unauthorized",
        StatusCode::FORBIDDEN => "forbidden",
        StatusCode::NOT_FOUND => "not_found",
        StatusCode::CONFLICT => "conflict",
        StatusCode::TOO_MANY_REQUESTS => "rate_limited",
        _ => "internal_error",
    };
    ApiError::new(status, code, msg)
}
pub fn internal(e: impl ToString) -> ApiError { err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()) }

pub fn deserialize_optional_nullable<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where D: serde::Deserializer<'de> {
    Ok(Some(Option::deserialize(deserializer)?))
}

pub use crate::auth::is_owner_or_root;

pub const VALID_TASK_STATUSES: &[&str] = &["backlog", "active", "in_progress", "completed", "done", "estimated", "archived"];
pub const VALID_SPRINT_STATUSES: &[&str] = &["planning", "active", "completed"];
pub const VALID_ROLES: &[&str] = &["user", "root"];
pub const VALID_ROOM_ROLES: &[&str] = &["admin", "voter"];

pub fn validate_task_status(s: &str) -> Result<(), ApiError> {
    if !VALID_TASK_STATUSES.contains(&s) { Err(err(StatusCode::BAD_REQUEST, format!("Invalid status '{}'. Must be one of: {}", s, VALID_TASK_STATUSES.join(", ")))) } else { Ok(()) }
}

pub fn validate_sprint_status(s: &str) -> Result<(), ApiError> {
    if !VALID_SPRINT_STATUSES.contains(&s) { Err(err(StatusCode::BAD_REQUEST, format!("Invalid sprint status '{}'. Must be one of: {}", s, VALID_SPRINT_STATUSES.join(", ")))) } else { Ok(()) }
}

pub fn validate_username(u: &str) -> Result<(), ApiError> {
    if u.trim().is_empty() { return Err(err(StatusCode::BAD_REQUEST, "Username cannot be empty")); }
    if u.len() > 32 { return Err(err(StatusCode::BAD_REQUEST, "Username too long (max 32 chars)")); }
    if !u.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return Err(err(StatusCode::BAD_REQUEST, "Username must be alphanumeric (underscores and hyphens allowed)"));
    }
    Ok(())
}

pub fn validate_password(p: &str) -> Result<(), ApiError> {
    if p.len() < 8 { return Err(err(StatusCode::BAD_REQUEST, "Password must be at least 8 characters")); }
    if p.len() > 128 { return Err(err(StatusCode::BAD_REQUEST, "Password too long (max 128 chars)")); }
    if !p.chars().any(|c| c.is_uppercase()) { return Err(err(StatusCode::BAD_REQUEST, "Password must contain an uppercase letter")); }
    if !p.chars().any(|c| c.is_ascii_digit()) { return Err(err(StatusCode::BAD_REQUEST, "Password must contain a digit")); }
    Ok(())
}
