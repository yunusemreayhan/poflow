use sqlx::{FromRow, SqlitePool};

pub type Pool = SqlitePool;

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Task {
    pub id: i64, pub parent_id: Option<i64>, pub user_id: i64, pub user: String,
    pub title: String, pub description: Option<String>, pub project: Option<String>,
    pub tags: Option<String>, pub priority: i64, pub estimated: i64, pub actual: i64,
    pub estimated_hours: f64, pub remaining_points: f64, pub due_date: Option<String>,
    pub status: String, pub sort_order: i64, pub created_at: String, pub updated_at: String,
    pub attachment_count: i64,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Session {
    pub id: i64, pub task_id: Option<i64>, pub user_id: i64, pub user: String,
    pub session_type: String, pub status: String, pub started_at: String,
    pub ended_at: Option<String>, pub duration_s: Option<i64>, pub notes: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct SessionWithPath { #[serde(flatten)] pub session: Session, pub task_path: Vec<String> }

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Comment {
    pub id: i64, pub task_id: i64, pub session_id: Option<i64>, pub user_id: i64,
    pub user: String, pub content: String, pub created_at: String,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct TaskAssignee { pub task_id: i64, pub user_id: i64, pub username: String }

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct User {
    pub id: i64, pub username: String, #[serde(skip_serializing)] pub password_hash: String,
    pub role: String, pub created_at: String,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Room {
    pub id: i64, pub name: String, pub room_type: String, pub estimation_unit: String,
    pub project: Option<String>, pub creator_id: i64, pub creator: String,
    pub status: String, pub current_task_id: Option<i64>, pub created_at: String,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct RoomMember {
    pub room_id: i64, pub user_id: i64, pub username: String, pub role: String, pub joined_at: String,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct RoomVote {
    pub id: i64, pub room_id: i64, pub task_id: i64, pub user_id: i64,
    pub username: String, pub value: Option<f64>, pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct RoomState {
    pub room: Room, pub members: Vec<RoomMember>, pub current_task: Option<Task>,
    pub votes: Vec<RoomVoteView>, pub tasks: Vec<Task>, pub vote_history: Vec<VoteResult>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct RoomVoteView { pub username: String, pub voted: bool, pub value: Option<f64> }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct VoteResult {
    pub task_id: i64, pub task_title: String, pub votes: Vec<RoomVote>,
    pub average: f64, pub consensus: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct TaskDetail {
    pub task: Task, pub comments: Vec<Comment>, pub sessions: Vec<Session>,
    #[schema(no_recursion)] pub children: Vec<TaskDetail>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct DayStat { pub date: String, pub completed: i64, pub interrupted: i64, pub total_focus_s: i64 }

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Sprint {
    pub id: i64, pub name: String, pub project: Option<String>, pub goal: Option<String>,
    pub status: String, pub start_date: Option<String>, pub end_date: Option<String>,
    pub retro_notes: Option<String>,
    pub created_by_id: i64, pub created_by: String, pub created_at: String, pub updated_at: String,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct SprintTask {
    pub sprint_id: i64, pub task_id: i64, pub added_by_id: i64, pub added_by: String, pub added_at: String,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct SprintDailyStat {
    pub id: i64, pub sprint_id: i64, pub date: String, pub total_points: f64,
    pub done_points: f64, pub total_hours: f64, pub done_hours: f64, pub total_tasks: i64, pub done_tasks: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct SprintDetail { pub sprint: Sprint, pub tasks: Vec<Task>, pub stats: Vec<SprintDailyStat> }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct Team { pub id: i64, pub name: String, pub created_at: String }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct TeamMember { pub team_id: i64, pub user_id: i64, pub username: String, pub role: String }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct TeamDetail { pub team: Team, pub members: Vec<TeamMember>, pub root_task_ids: Vec<i64> }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct EpicGroup { pub id: i64, pub name: String, pub created_by: i64, pub created_at: String, pub updated_at: String }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct EpicSnapshot {
    pub id: i64, pub group_id: i64, pub date: String, pub total_tasks: i64,
    pub done_tasks: i64, pub total_points: f64, pub done_points: f64, pub total_hours: f64, pub done_hours: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct EpicGroupDetail { pub group: EpicGroup, pub task_ids: Vec<i64>, pub snapshots: Vec<EpicSnapshot> }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct SprintBoard { pub todo: Vec<Task>, pub in_progress: Vec<Task>, pub done: Vec<Task> }

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct TaskSprintInfo { pub task_id: i64, pub sprint_id: i64, pub sprint_name: String, pub sprint_status: String }

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct BurnEntry {
    pub id: i64, pub sprint_id: Option<i64>, pub task_id: i64, pub session_id: Option<i64>,
    pub user_id: i64, pub username: String, pub points: f64, pub hours: f64,
    pub source: String, pub note: Option<String>, pub cancelled: i64,
    pub cancelled_by_id: Option<i64>, pub cancelled_by: Option<String>, pub created_at: String,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct BurnTotal { pub total_points: f64, pub total_hours: f64, pub count: i64 }

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct BurnSummaryEntry { pub date: String, pub username: String, pub points: f64, pub hours: f64, pub count: i64 }
