use anyhow::Result;
use chrono::{NaiveDateTime, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{FromRow, SqlitePool};
use std::path::PathBuf;
use std::str::FromStr;

pub type Pool = SqlitePool;

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Task {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub user_id: i64,
    pub user: String,
    pub title: String,
    pub description: Option<String>,
    pub project: Option<String>,
    pub tags: Option<String>,
    pub priority: i64,
    pub estimated: i64,
    pub actual: i64,
    pub estimated_hours: f64,
    pub remaining_points: f64,
    pub due_date: Option<String>,
    pub status: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Session {
    pub id: i64,
    pub task_id: Option<i64>,
    pub user_id: i64,
    pub user: String,
    pub session_type: String,
    pub status: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_s: Option<i64>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct SessionWithPath {
    #[serde(flatten)]
    pub session: Session,
    pub task_path: Vec<String>,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Comment {
    pub id: i64,
    pub task_id: i64,
    pub session_id: Option<i64>,
    pub user_id: i64,
    pub user: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct TaskAssignee {
    pub task_id: i64,
    pub user_id: i64,
    pub username: String,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct User {
    pub id: i64,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: String,
    pub created_at: String,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Room {
    pub id: i64,
    pub name: String,
    pub room_type: String,
    pub estimation_unit: String,
    pub project: Option<String>,
    pub creator_id: i64,
    pub creator: String,
    pub status: String,
    pub current_task_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct RoomMember {
    pub room_id: i64,
    pub user_id: i64,
    pub username: String,
    pub role: String,
    pub joined_at: String,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct RoomVote {
    pub id: i64,
    pub room_id: i64,
    pub task_id: i64,
    pub user_id: i64,
    pub username: String,
    pub value: Option<f64>,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct RoomState {
    pub room: Room,
    pub members: Vec<RoomMember>,
    pub current_task: Option<Task>,
    pub votes: Vec<RoomVoteView>,
    pub tasks: Vec<Task>,
    pub vote_history: Vec<VoteResult>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct RoomVoteView {
    pub username: String,
    pub voted: bool,
    pub value: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct VoteResult {
    pub task_id: i64,
    pub task_title: String,
    pub votes: Vec<RoomVote>,
    pub average: f64,
    pub consensus: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct TaskDetail {
    pub task: Task,
    pub comments: Vec<Comment>,
    pub sessions: Vec<Session>,
    #[schema(no_recursion)]
    pub children: Vec<TaskDetail>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct DayStat {
    pub date: String,
    pub completed: i64,
    pub interrupted: i64,
    pub total_focus_s: i64,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Sprint {
    pub id: i64,
    pub name: String,
    pub project: Option<String>,
    pub goal: Option<String>,
    pub status: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub created_by_id: i64,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct SprintTask {
    pub sprint_id: i64,
    pub task_id: i64,
    pub added_by_id: i64,
    pub added_by: String,
    pub added_at: String,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct SprintDailyStat {
    pub id: i64,
    pub sprint_id: i64,
    pub date: String,
    pub total_points: f64,
    pub done_points: f64,
    pub total_hours: f64,
    pub done_hours: f64,
    pub total_tasks: i64,
    pub done_tasks: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct SprintDetail {
    pub sprint: Sprint,
    pub tasks: Vec<Task>,
    pub stats: Vec<SprintDailyStat>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct Team {
    pub id: i64,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct TeamMember {
    pub team_id: i64,
    pub user_id: i64,
    pub username: String,
    pub role: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct TeamDetail {
    pub team: Team,
    pub members: Vec<TeamMember>,
    pub root_task_ids: Vec<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct EpicGroup {
    pub id: i64,
    pub name: String,
    pub created_by: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct EpicSnapshot {
    pub id: i64,
    pub group_id: i64,
    pub date: String,
    pub total_tasks: i64,
    pub done_tasks: i64,
    pub total_points: f64,
    pub done_points: f64,
    pub total_hours: f64,
    pub done_hours: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct EpicGroupDetail {
    pub group: EpicGroup,
    pub task_ids: Vec<i64>,
    pub snapshots: Vec<EpicSnapshot>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct SprintBoard {
    pub todo: Vec<Task>,
    pub in_progress: Vec<Task>,
    pub done: Vec<Task>,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct TaskSprintInfo {
    pub task_id: i64,
    pub sprint_id: i64,
    pub sprint_name: String,
    pub sprint_status: String,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct BurnEntry {
    pub id: i64,
    pub sprint_id: Option<i64>,
    pub task_id: i64,
    pub session_id: Option<i64>,
    pub user_id: i64,
    pub username: String,
    pub points: f64,
    pub hours: f64,
    pub source: String,
    pub note: Option<String>,
    pub cancelled: i64,
    pub cancelled_by_id: Option<i64>,
    pub cancelled_by: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct BurnTotal {
    pub total_points: f64,
    pub total_hours: f64,
    pub count: i64,
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct BurnSummaryEntry {
    pub date: String,
    pub username: String,
    pub points: f64,
    pub hours: f64,
    pub count: i64,
}

fn db_path() -> PathBuf {
    let dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("~/.local/share")).join("pomodoro");
    std::fs::create_dir_all(&dir).ok();
    dir.join("pomodoro.db")
}

pub async fn connect() -> Result<Pool> {
    let path = db_path();
    let opts = SqliteConnectOptions::from_str(&format!("sqlite:{}?mode=rwc", path.display()))?
        .create_if_missing(true).journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
    let pool = SqlitePoolOptions::new().max_connections(5).connect_with(opts).await?;
    migrate(&pool).await?;
    seed_root_user(&pool).await?;
    Ok(pool)
}

pub async fn connect_memory() -> Result<Pool> {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")?;
    let pool = SqlitePoolOptions::new().max_connections(1).connect_with(opts).await?;
    migrate(&pool).await?;
    seed_root_user(&pool).await?;
    Ok(pool)
}

async fn migrate(pool: &Pool) -> Result<()> {
    sqlx::query("PRAGMA foreign_keys = ON").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS users (
        id            INTEGER PRIMARY KEY AUTOINCREMENT,
        username      TEXT NOT NULL UNIQUE,
        password_hash TEXT NOT NULL,
        role          TEXT NOT NULL DEFAULT 'user',
        created_at    TEXT NOT NULL
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS tasks (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        parent_id   INTEGER REFERENCES tasks(id) ON DELETE CASCADE,
        user_id     INTEGER NOT NULL REFERENCES users(id),
        title       TEXT NOT NULL,
        description TEXT,
        project     TEXT,
        tags        TEXT,
        priority    INTEGER NOT NULL DEFAULT 3,
        estimated   INTEGER NOT NULL DEFAULT 1,
        actual      INTEGER NOT NULL DEFAULT 0,
        estimated_hours REAL NOT NULL DEFAULT 0,
        remaining_points REAL NOT NULL DEFAULT 0,
        due_date    TEXT,
        status      TEXT NOT NULL DEFAULT 'backlog',
        sort_order  INTEGER NOT NULL DEFAULT 0,
        created_at  TEXT NOT NULL,
        updated_at  TEXT NOT NULL
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS sessions (
        id           INTEGER PRIMARY KEY AUTOINCREMENT,
        task_id      INTEGER REFERENCES tasks(id),
        user_id      INTEGER NOT NULL REFERENCES users(id),
        session_type TEXT NOT NULL,
        status       TEXT NOT NULL,
        started_at   TEXT NOT NULL,
        ended_at     TEXT,
        duration_s   INTEGER,
        notes        TEXT
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS comments (
        id           INTEGER PRIMARY KEY AUTOINCREMENT,
        task_id      INTEGER NOT NULL REFERENCES tasks(id),
        session_id   INTEGER REFERENCES sessions(id),
        user_id      INTEGER NOT NULL REFERENCES users(id),
        content      TEXT NOT NULL,
        created_at   TEXT NOT NULL
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS task_assignees (
        task_id  INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
        user_id  INTEGER NOT NULL REFERENCES users(id),
        PRIMARY KEY (task_id, user_id)
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS rooms (
        id               INTEGER PRIMARY KEY AUTOINCREMENT,
        name             TEXT NOT NULL,
        room_type        TEXT NOT NULL DEFAULT 'estimation',
        estimation_unit  TEXT NOT NULL DEFAULT 'points',
        project          TEXT,
        creator_id       INTEGER NOT NULL REFERENCES users(id),
        status           TEXT NOT NULL DEFAULT 'lobby',
        current_task_id  INTEGER REFERENCES tasks(id),
        created_at       TEXT NOT NULL
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS room_members (
        room_id   INTEGER NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
        user_id   INTEGER NOT NULL REFERENCES users(id),
        role      TEXT NOT NULL DEFAULT 'voter',
        joined_at TEXT NOT NULL,
        PRIMARY KEY (room_id, user_id)
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS room_votes (
        id         INTEGER PRIMARY KEY AUTOINCREMENT,
        room_id    INTEGER NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
        task_id    INTEGER NOT NULL REFERENCES tasks(id),
        user_id    INTEGER NOT NULL REFERENCES users(id),
        value      REAL,
        created_at TEXT NOT NULL,
        UNIQUE(room_id, task_id, user_id)
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS sprints (
        id            INTEGER PRIMARY KEY AUTOINCREMENT,
        name          TEXT NOT NULL,
        project       TEXT,
        goal          TEXT,
        status        TEXT NOT NULL DEFAULT 'planning',
        start_date    TEXT,
        end_date      TEXT,
        created_by_id INTEGER NOT NULL REFERENCES users(id),
        created_at    TEXT NOT NULL,
        updated_at    TEXT NOT NULL
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS sprint_tasks (
        sprint_id   INTEGER NOT NULL REFERENCES sprints(id) ON DELETE CASCADE,
        task_id     INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
        added_by_id INTEGER NOT NULL REFERENCES users(id),
        added_at    TEXT NOT NULL,
        PRIMARY KEY (sprint_id, task_id)
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS sprint_daily_stats (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        sprint_id       INTEGER NOT NULL REFERENCES sprints(id) ON DELETE CASCADE,
        date            TEXT NOT NULL,
        total_points    REAL NOT NULL DEFAULT 0,
        done_points     REAL NOT NULL DEFAULT 0,
        total_hours     REAL NOT NULL DEFAULT 0,
        done_hours      REAL NOT NULL DEFAULT 0,
        total_tasks     INTEGER NOT NULL DEFAULT 0,
        done_tasks      INTEGER NOT NULL DEFAULT 0,
        UNIQUE(sprint_id, date)
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS burn_log (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        sprint_id       INTEGER REFERENCES sprints(id) ON DELETE CASCADE,
        task_id         INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
        session_id      INTEGER REFERENCES sessions(id),
        user_id         INTEGER NOT NULL REFERENCES users(id),
        points          REAL NOT NULL DEFAULT 0,
        hours           REAL NOT NULL DEFAULT 0,
        source          TEXT NOT NULL DEFAULT 'manual',
        note            TEXT,
        cancelled       INTEGER NOT NULL DEFAULT 0,
        cancelled_by_id INTEGER REFERENCES users(id),
        created_at      TEXT NOT NULL
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS user_configs (
        user_id             INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
        work_duration_min   INTEGER,
        short_break_min     INTEGER,
        long_break_min      INTEGER,
        long_break_interval INTEGER,
        auto_start_breaks   INTEGER,
        auto_start_work     INTEGER,
        daily_goal          INTEGER
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS sprint_root_tasks (
        sprint_id   INTEGER NOT NULL REFERENCES sprints(id) ON DELETE CASCADE,
        task_id     INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
        PRIMARY KEY (sprint_id, task_id)
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS teams (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        name        TEXT NOT NULL UNIQUE,
        created_at  TEXT NOT NULL
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS team_members (
        team_id     INTEGER NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
        user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        role        TEXT NOT NULL DEFAULT 'member',
        PRIMARY KEY (team_id, user_id)
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS team_root_tasks (
        team_id     INTEGER NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
        task_id     INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
        PRIMARY KEY (team_id, task_id)
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS epic_groups (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        name        TEXT NOT NULL,
        created_by  INTEGER NOT NULL REFERENCES users(id),
        created_at  TEXT NOT NULL,
        updated_at  TEXT NOT NULL
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS epic_group_tasks (
        group_id    INTEGER NOT NULL REFERENCES epic_groups(id) ON DELETE CASCADE,
        task_id     INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
        PRIMARY KEY (group_id, task_id)
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS epic_snapshots (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        group_id        INTEGER NOT NULL REFERENCES epic_groups(id) ON DELETE CASCADE,
        date            TEXT NOT NULL,
        total_tasks     INTEGER NOT NULL DEFAULT 0,
        done_tasks      INTEGER NOT NULL DEFAULT 0,
        total_points    REAL NOT NULL DEFAULT 0,
        done_points     REAL NOT NULL DEFAULT 0,
        total_hours     REAL NOT NULL DEFAULT 0,
        done_hours      REAL NOT NULL DEFAULT 0,
        UNIQUE(group_id, date)
    )").execute(pool).await?;

    // Indexes for frequently queried columns
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_tasks_parent_id ON tasks(parent_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_tasks_project ON tasks(project)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_sessions_started_at ON sessions(started_at)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_burn_log_task_id ON burn_log(task_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_burn_log_sprint_id ON burn_log(sprint_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_sprint_tasks_sprint_id ON sprint_tasks(sprint_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_sprint_tasks_task_id ON sprint_tasks(task_id)").execute(pool).await?;

    Ok(())
}

// --- User CRUD ---

async fn seed_root_user(pool: &Pool) -> Result<()> {
    let count = user_count(pool).await?;
    if count == 0 {
        let hash = bcrypt::hash("root", 12).map_err(|e| anyhow::anyhow!(e))?;
        create_user(pool, "root", &hash, "root").await?;
        tracing::info!("Seeded default root user (root/root)");
    }
    Ok(())
}

pub async fn create_user(pool: &Pool, username: &str, password_hash: &str, role: &str) -> Result<User> {
    let now = Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S").to_string();
    let id = sqlx::query("INSERT INTO users (username, password_hash, role, created_at) VALUES (?, ?, ?, ?)")
        .bind(username).bind(password_hash).bind(role).bind(&now)
        .execute(pool).await?.last_insert_rowid();
    Ok(sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?").bind(id).fetch_one(pool).await?)
}

pub async fn get_user_by_username(pool: &Pool, username: &str) -> Result<User> {
    Ok(sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?").bind(username).fetch_one(pool).await?)
}

pub async fn get_user(pool: &Pool, id: i64) -> Result<User> {
    Ok(sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?").bind(id).fetch_one(pool).await?)
}

pub async fn user_count(pool: &Pool) -> Result<i64> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users").fetch_one(pool).await?;
    Ok(count)
}

pub async fn list_users(pool: &Pool) -> Result<Vec<User>> {
    Ok(sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY created_at ASC").fetch_all(pool).await?)
}

pub async fn delete_user(pool: &Pool, id: i64) -> Result<()> {
    let user = get_user(pool, id).await?;
    if user.role == "root" {
        let (root_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE role = 'root'").fetch_one(pool).await?;
        if root_count <= 1 { return Err(anyhow::anyhow!("Cannot delete the last root user")); }
    }
    sqlx::query("DELETE FROM burn_log WHERE user_id = ?").bind(id).execute(pool).await?;
    sqlx::query("DELETE FROM comments WHERE user_id = ?").bind(id).execute(pool).await?;
    sqlx::query("DELETE FROM task_assignees WHERE user_id = ?").bind(id).execute(pool).await?;
    sqlx::query("DELETE FROM room_members WHERE user_id = ?").bind(id).execute(pool).await?;
    sqlx::query("DELETE FROM room_votes WHERE user_id = ?").bind(id).execute(pool).await?;
    sqlx::query("UPDATE sessions SET user_id = (SELECT id FROM users WHERE role = 'root' LIMIT 1) WHERE user_id = ?").bind(id).execute(pool).await?;
    sqlx::query("UPDATE tasks SET user_id = (SELECT id FROM users WHERE role = 'root' LIMIT 1) WHERE user_id = ?").bind(id).execute(pool).await?;
    sqlx::query("UPDATE sprint_tasks SET added_by_id = (SELECT id FROM users WHERE role = 'root' LIMIT 1) WHERE added_by_id = ?").bind(id).execute(pool).await?;
    sqlx::query("UPDATE rooms SET creator_id = (SELECT id FROM users WHERE role = 'root' LIMIT 1) WHERE creator_id = ?").bind(id).execute(pool).await?;
    sqlx::query("UPDATE sprints SET created_by_id = (SELECT id FROM users WHERE role = 'root' LIMIT 1) WHERE created_by_id = ?").bind(id).execute(pool).await?;
    sqlx::query("DELETE FROM users WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

pub async fn update_user_role(pool: &Pool, id: i64, role: &str) -> Result<User> {
    sqlx::query("UPDATE users SET role = ? WHERE id = ?").bind(role).bind(id).execute(pool).await?;
    get_user(pool, id).await
}

pub async fn update_user_password(pool: &Pool, id: i64, password_hash: &str) -> Result<()> {
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?").bind(password_hash).bind(id).execute(pool).await?;
    Ok(())
}

pub async fn update_username(pool: &Pool, id: i64, username: &str) -> Result<()> {
    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM users WHERE username = ? AND id != ?")
        .bind(username).bind(id).fetch_optional(pool).await?;
    if existing.is_some() { return Err(anyhow::anyhow!("Username already taken")); }
    sqlx::query("UPDATE users SET username = ? WHERE id = ?").bind(username).bind(id).execute(pool).await?;
    Ok(())
}

// --- Task CRUD ---

const TASK_SELECT: &str = "SELECT t.id, t.parent_id, t.user_id, u.username as user, t.title, t.description, t.project, t.tags, t.priority, t.estimated, t.actual, t.estimated_hours, t.remaining_points, t.due_date, t.status, t.sort_order, t.created_at, t.updated_at FROM tasks t JOIN users u ON t.user_id = u.id";

pub async fn create_task(pool: &Pool, user_id: i64, parent_id: Option<i64>, title: &str, description: Option<&str>, project: Option<&str>, tags: Option<&str>, priority: i64, estimated: i64, estimated_hours: f64, remaining_points: f64, due_date: Option<&str>) -> Result<Task> {
    let now = Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S").to_string();
    let max_order: (i64,) = match parent_id {
        Some(pid) => sqlx::query_as("SELECT COALESCE(MAX(sort_order), 0) FROM tasks WHERE parent_id = ?").bind(pid).fetch_one(pool).await?,
        None => sqlx::query_as("SELECT COALESCE(MAX(sort_order), 0) FROM tasks WHERE parent_id IS NULL AND user_id = ?").bind(user_id).fetch_one(pool).await?,
    };
    let id = sqlx::query("INSERT INTO tasks (parent_id, user_id, title, description, project, tags, priority, estimated, estimated_hours, remaining_points, due_date, status, sort_order, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'backlog', ?, ?, ?)")
        .bind(parent_id).bind(user_id).bind(title).bind(description).bind(project).bind(tags).bind(priority).bind(estimated).bind(estimated_hours).bind(remaining_points).bind(due_date).bind(max_order.0 + 1).bind(&now).bind(&now)
        .execute(pool).await?.last_insert_rowid();
    get_task(pool, id).await
}

pub async fn get_task(pool: &Pool, id: i64) -> Result<Task> {
    Ok(sqlx::query_as::<_, Task>(&format!("{} WHERE t.id = ?", TASK_SELECT)).bind(id).fetch_one(pool).await?)
}

pub async fn list_tasks(pool: &Pool, status: Option<&str>, project: Option<&str>) -> Result<Vec<Task>> {
    list_tasks_paged(pool, status, project, 5000, 0, None).await
}

pub async fn list_tasks_paged(pool: &Pool, status: Option<&str>, project: Option<&str>, limit: i64, offset: i64, team_id: Option<i64>) -> Result<Vec<Task>> {
    // If team filter, get all descendant IDs first
    let team_scope: Option<Vec<i64>> = if let Some(tid) = team_id {
        let roots: Vec<(i64,)> = sqlx::query_as("SELECT task_id FROM team_root_tasks WHERE team_id = ?").bind(tid).fetch_all(pool).await?;
        if roots.is_empty() { return Ok(vec![]); }
        let rids: Vec<i64> = roots.into_iter().map(|r| r.0).collect();
        Some(get_descendant_ids(pool, &rids).await?)
    } else { None };

    let mut q = format!("{} WHERE 1=1", TASK_SELECT);
    if status.is_some() { q.push_str(" AND t.status = ?"); }
    if project.is_some() { q.push_str(" AND t.project = ?"); }
    if let Some(ref ids) = team_scope {
        let ph: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        q.push_str(&format!(" AND t.id IN ({})", ph));
    }
    q.push_str(" ORDER BY t.sort_order ASC, t.id ASC LIMIT ? OFFSET ?");
    let mut query = sqlx::query_as::<_, Task>(&q);
    if let Some(s) = status { query = query.bind(s); }
    if let Some(p) = project { query = query.bind(p); }
    if let Some(ref ids) = team_scope { for id in ids { query = query.bind(id); } }
    query = query.bind(limit).bind(offset);
    Ok(query.fetch_all(pool).await?)
}

pub async fn update_task(pool: &Pool, id: i64, title: Option<&str>, description: Option<Option<&str>>, project: Option<Option<&str>>, tags: Option<Option<&str>>, priority: Option<i64>, estimated: Option<i64>, estimated_hours: Option<f64>, remaining_points: Option<f64>, due_date: Option<Option<&str>>, status: Option<&str>, sort_order: Option<i64>, parent_id: Option<Option<i64>>) -> Result<Task> {
    let now = Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S%.3f").to_string();
    let existing = get_task(pool, id).await?;
    let new_parent = match parent_id { Some(p) => p, None => existing.parent_id };
    let new_desc = match description { Some(v) => v.map(|s| s.to_string()), None => existing.description };
    let new_project = match project { Some(v) => v.map(|s| s.to_string()), None => existing.project };
    let new_tags = match tags { Some(v) => v.map(|s| s.to_string()), None => existing.tags };
    let new_due = match due_date { Some(v) => v.map(|s| s.to_string()), None => existing.due_date };
    sqlx::query("UPDATE tasks SET parent_id=?, title=?, description=?, project=?, tags=?, priority=?, estimated=?, estimated_hours=?, remaining_points=?, due_date=?, status=?, sort_order=?, updated_at=? WHERE id=?")
        .bind(new_parent).bind(title.unwrap_or(&existing.title)).bind(&new_desc)
        .bind(&new_project).bind(&new_tags)
        .bind(priority.unwrap_or(existing.priority)).bind(estimated.unwrap_or(existing.estimated))
        .bind(estimated_hours.unwrap_or(existing.estimated_hours)).bind(remaining_points.unwrap_or(existing.remaining_points))
        .bind(&new_due).bind(status.unwrap_or(&existing.status))
        .bind(sort_order.unwrap_or(existing.sort_order)).bind(&now).bind(id)
        .execute(pool).await?;
    get_task(pool, id).await
}

pub async fn delete_task(pool: &Pool, id: i64) -> Result<()> {
    sqlx::query("PRAGMA foreign_keys = ON").execute(pool).await?;
    let mut ids = vec![id];
    let mut i = 0;
    while i < ids.len() {
        let children: Vec<(i64,)> = sqlx::query_as("SELECT id FROM tasks WHERE parent_id = ?").bind(ids[i]).fetch_all(pool).await?;
        for (cid,) in children { ids.push(cid); }
        i += 1;
    }
    for tid in &ids {
        sqlx::query("UPDATE sessions SET task_id = NULL WHERE task_id = ?").bind(tid).execute(pool).await?;
        sqlx::query("DELETE FROM comments WHERE task_id = ?").bind(tid).execute(pool).await?;
        sqlx::query("DELETE FROM task_assignees WHERE task_id = ?").bind(tid).execute(pool).await?;
        sqlx::query("DELETE FROM burn_log WHERE task_id = ?").bind(tid).execute(pool).await?;
        sqlx::query("DELETE FROM sprint_tasks WHERE task_id = ?").bind(tid).execute(pool).await?;
        sqlx::query("DELETE FROM room_votes WHERE task_id = ?").bind(tid).execute(pool).await?;
    }
    sqlx::query("DELETE FROM tasks WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

pub async fn increment_task_actual(pool: &Pool, id: i64) -> Result<()> {
    sqlx::query("UPDATE tasks SET actual = actual + 1 WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

// --- Session CRUD ---

const SESSION_SELECT: &str = "SELECT s.id, s.task_id, s.user_id, u.username as user, s.session_type, s.status, s.started_at, s.ended_at, s.duration_s, s.notes FROM sessions s JOIN users u ON s.user_id = u.id";

pub async fn create_session(pool: &Pool, user_id: i64, task_id: Option<i64>, session_type: &str) -> Result<Session> {
    let now = Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S").to_string();
    let id = sqlx::query("INSERT INTO sessions (task_id, user_id, session_type, status, started_at) VALUES (?, ?, ?, 'running', ?)")
        .bind(task_id).bind(user_id).bind(session_type).bind(&now)
        .execute(pool).await?.last_insert_rowid();
    Ok(sqlx::query_as::<_, Session>(&format!("{} WHERE s.id = ?", SESSION_SELECT)).bind(id).fetch_one(pool).await?)
}

pub async fn end_session(pool: &Pool, id: i64, status: &str) -> Result<Session> {
    let now = Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S").to_string();
    let started_at: (String,) = sqlx::query_as("SELECT started_at FROM sessions WHERE id = ?").bind(id).fetch_one(pool).await?;
    let started = NaiveDateTime::parse_from_str(&started_at.0, "%Y-%m-%dT%H:%M:%S")?;
    let duration = (Utc::now().naive_utc() - started).num_seconds();
    sqlx::query("UPDATE sessions SET status=?, ended_at=?, duration_s=? WHERE id=?")
        .bind(status).bind(&now).bind(duration).bind(id).execute(pool).await?;
    Ok(sqlx::query_as::<_, Session>(&format!("{} WHERE s.id = ?", SESSION_SELECT)).bind(id).fetch_one(pool).await?)
}

pub async fn recover_interrupted(pool: &Pool) -> Result<Vec<Session>> {
    let sessions: Vec<Session> = sqlx::query_as(&format!("{} WHERE s.status = 'running'", SESSION_SELECT)).fetch_all(pool).await?;
    for s in &sessions {
        let now = Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S").to_string();
        let started = NaiveDateTime::parse_from_str(&s.started_at, "%Y-%m-%dT%H:%M:%S").unwrap_or(Utc::now().naive_utc());
        let duration = (Utc::now().naive_utc() - started).num_seconds();
        sqlx::query("UPDATE sessions SET status='interrupted', ended_at=?, duration_s=? WHERE id=?")
            .bind(&now).bind(duration).bind(s.id).execute(pool).await?;
    }
    Ok(sessions)
}

pub async fn get_history(pool: &Pool, from: &str, to: &str) -> Result<Vec<SessionWithPath>> {
    let sessions: Vec<Session> = sqlx::query_as(&format!("{} WHERE s.started_at >= ? AND s.started_at <= ? ORDER BY s.started_at DESC LIMIT 500", SESSION_SELECT))
        .bind(from).bind(to).fetch_all(pool).await?;
    // Only load tasks referenced by these sessions
    let task_ids: Vec<i64> = sessions.iter().filter_map(|s| s.task_id).collect();
    if task_ids.is_empty() {
        return Ok(sessions.into_iter().map(|s| SessionWithPath { session: s, task_path: vec![] }).collect());
    }
    let placeholders = task_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    // Use CTE to get all ancestors
    let cte_sql = format!(
        "WITH RECURSIVE ancestors AS (SELECT id, parent_id, title FROM tasks WHERE id IN ({}) UNION ALL SELECT t.id, t.parent_id, t.title FROM tasks t JOIN ancestors a ON t.id = a.parent_id) SELECT DISTINCT id, parent_id, title FROM ancestors",
        placeholders
    );
    let mut q = sqlx::query_as::<_, (i64, Option<i64>, String)>(&cte_sql);
    for tid in &task_ids { q = q.bind(tid); }
    let rows: Vec<(i64, Option<i64>, String)> = q.fetch_all(pool).await?;
    let task_map: std::collections::HashMap<i64, (Option<i64>, String)> = rows.into_iter().map(|(id, pid, title)| (id, (pid, title))).collect();
    Ok(sessions.into_iter().map(|s| {
        let mut path = Vec::new();
        let mut current = s.task_id;
        while let Some(id) = current {
            if let Some((pid, title)) = task_map.get(&id) { path.push(title.clone()); current = *pid; } else { break; }
        }
        path.reverse();
        SessionWithPath { session: s, task_path: path }
    }).collect())
}

pub async fn get_day_stats(pool: &Pool, days: i64) -> Result<Vec<DayStat>> {
    let from = (Utc::now().naive_utc() - chrono::Duration::days(days)).format("%Y-%m-%dT00:00:00").to_string();
    let rows: Vec<Session> = sqlx::query_as(&format!("{} WHERE s.session_type = 'work' AND s.started_at >= ? ORDER BY s.started_at", SESSION_SELECT))
        .bind(&from).fetch_all(pool).await?;
    let mut map: std::collections::BTreeMap<String, DayStat> = std::collections::BTreeMap::new();
    for r in rows {
        let date = r.started_at.get(..10).unwrap_or("").to_string();
        let entry = map.entry(date.clone()).or_insert(DayStat { date, completed: 0, interrupted: 0, total_focus_s: 0 });
        match r.status.as_str() { "completed" => entry.completed += 1, "interrupted" => entry.interrupted += 1, _ => {} }
        entry.total_focus_s += r.duration_s.unwrap_or(0);
    }
    Ok(map.into_values().collect())
}

pub async fn get_today_completed(pool: &Pool) -> Result<i64> {
    let today = Utc::now().naive_utc().format("%Y-%m-%dT00:00:00").to_string();
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE session_type = 'work' AND status = 'completed' AND started_at >= ?")
        .bind(&today).fetch_one(pool).await?;
    Ok(count)
}

// --- Comment CRUD ---

const COMMENT_SELECT: &str = "SELECT c.id, c.task_id, c.session_id, c.user_id, u.username as user, c.content, c.created_at FROM comments c JOIN users u ON c.user_id = u.id";

pub async fn add_comment(pool: &Pool, user_id: i64, task_id: i64, session_id: Option<i64>, content: &str) -> Result<Comment> {
    let now = Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S").to_string();
    let id = sqlx::query("INSERT INTO comments (task_id, session_id, user_id, content, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind(task_id).bind(session_id).bind(user_id).bind(content).bind(&now)
        .execute(pool).await?.last_insert_rowid();
    Ok(sqlx::query_as::<_, Comment>(&format!("{} WHERE c.id = ?", COMMENT_SELECT)).bind(id).fetch_one(pool).await?)
}

pub async fn list_comments(pool: &Pool, task_id: i64) -> Result<Vec<Comment>> {
    Ok(sqlx::query_as::<_, Comment>(&format!("{} WHERE c.task_id = ? ORDER BY c.created_at ASC", COMMENT_SELECT))
        .bind(task_id).fetch_all(pool).await?)
}

pub async fn delete_comment(pool: &Pool, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM comments WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

pub async fn get_comment(pool: &Pool, id: i64) -> Result<Comment> {
    Ok(sqlx::query_as::<_, Comment>(&format!("{} WHERE c.id = ?", COMMENT_SELECT)).bind(id).fetch_one(pool).await?)
}

// --- Task detail ---

pub async fn get_task_detail(pool: &Pool, id: i64) -> Result<TaskDetail> {
    get_task_detail_depth(pool, id, 0, 10).await
}

async fn get_task_detail_depth(pool: &Pool, id: i64, depth: u32, max_depth: u32) -> Result<TaskDetail> {
    let task = get_task(pool, id).await?;
    let comments = list_comments(pool, id).await?;
    let sessions: Vec<Session> = sqlx::query_as(&format!("{} WHERE s.task_id = ? ORDER BY s.started_at DESC", SESSION_SELECT))
        .bind(id).fetch_all(pool).await?;
    let mut children = Vec::new();
    if depth < max_depth {
        let child_tasks: Vec<Task> = sqlx::query_as(&format!("{} WHERE t.parent_id = ? ORDER BY t.sort_order ASC", TASK_SELECT))
            .bind(id).fetch_all(pool).await?;
        for ct in child_tasks { children.push(Box::pin(get_task_detail_depth(pool, ct.id, depth + 1, max_depth)).await?); }
    }
    Ok(TaskDetail { task, comments, sessions, children })
}

// --- Time Reports ---

// --- Assignees ---

pub async fn add_assignee(pool: &Pool, task_id: i64, user_id: i64) -> Result<()> {
    sqlx::query("INSERT OR IGNORE INTO task_assignees (task_id, user_id) VALUES (?, ?)")
        .bind(task_id).bind(user_id).execute(pool).await?;
    Ok(())
}

pub async fn remove_assignee(pool: &Pool, task_id: i64, user_id: i64) -> Result<()> {
    sqlx::query("DELETE FROM task_assignees WHERE task_id = ? AND user_id = ?")
        .bind(task_id).bind(user_id).execute(pool).await?;
    Ok(())
}

pub async fn list_assignees(pool: &Pool, task_id: i64) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT u.username FROM task_assignees ta JOIN users u ON ta.user_id = u.id WHERE ta.task_id = ? ORDER BY u.username")
        .bind(task_id).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(u,)| u).collect())
}

pub async fn get_user_id_by_username(pool: &Pool, username: &str) -> Result<i64> {
    let (id,): (i64,) = sqlx::query_as("SELECT id FROM users WHERE username = ?").bind(username).fetch_one(pool).await?;
    Ok(id)
}

// --- Room CRUD ---

const ROOM_SELECT: &str = "SELECT r.id, r.name, r.room_type, r.estimation_unit, r.project, r.creator_id, u.username as creator, r.status, r.current_task_id, r.created_at FROM rooms r JOIN users u ON r.creator_id = u.id";

pub async fn create_room(pool: &Pool, name: &str, room_type: &str, estimation_unit: &str, project: Option<&str>, creator_id: i64) -> Result<Room> {
    let now = Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S").to_string();
    let id = sqlx::query("INSERT INTO rooms (name, room_type, estimation_unit, project, creator_id, status, created_at) VALUES (?, ?, ?, ?, ?, 'lobby', ?)")
        .bind(name).bind(room_type).bind(estimation_unit).bind(project).bind(creator_id).bind(&now)
        .execute(pool).await?.last_insert_rowid();
    sqlx::query("INSERT INTO room_members (room_id, user_id, role, joined_at) VALUES (?, ?, 'admin', ?)")
        .bind(id).bind(creator_id).bind(&now).execute(pool).await?;
    get_room(pool, id).await
}

pub async fn list_rooms(pool: &Pool) -> Result<Vec<Room>> {
    Ok(sqlx::query_as::<_, Room>(&format!("{} ORDER BY r.created_at DESC LIMIT 200", ROOM_SELECT)).fetch_all(pool).await?)
}

pub async fn get_room(pool: &Pool, id: i64) -> Result<Room> {
    Ok(sqlx::query_as::<_, Room>(&format!("{} WHERE r.id = ?", ROOM_SELECT)).bind(id).fetch_one(pool).await?)
}

pub async fn delete_room(pool: &Pool, id: i64) -> Result<()> {
    sqlx::query("PRAGMA foreign_keys = ON").execute(pool).await?;
    sqlx::query("DELETE FROM rooms WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

const MEMBER_SELECT: &str = "SELECT rm.room_id, rm.user_id, u.username, rm.role, rm.joined_at FROM room_members rm JOIN users u ON rm.user_id = u.id";

pub async fn join_room(pool: &Pool, room_id: i64, user_id: i64) -> Result<()> {
    let now = Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S").to_string();
    sqlx::query("INSERT OR IGNORE INTO room_members (room_id, user_id, role, joined_at) VALUES (?, ?, 'voter', ?)")
        .bind(room_id).bind(user_id).bind(&now).execute(pool).await?;
    Ok(())
}

pub async fn leave_room(pool: &Pool, room_id: i64, user_id: i64) -> Result<()> {
    sqlx::query("DELETE FROM room_members WHERE room_id = ? AND user_id = ?")
        .bind(room_id).bind(user_id).execute(pool).await?;
    Ok(())
}

pub async fn set_room_member_role(pool: &Pool, room_id: i64, user_id: i64, role: &str) -> Result<()> {
    sqlx::query("UPDATE room_members SET role = ? WHERE room_id = ? AND user_id = ?")
        .bind(role).bind(room_id).bind(user_id).execute(pool).await?;
    Ok(())
}

pub async fn get_room_members(pool: &Pool, room_id: i64) -> Result<Vec<RoomMember>> {
    Ok(sqlx::query_as::<_, RoomMember>(&format!("{} WHERE rm.room_id = ? ORDER BY rm.joined_at", MEMBER_SELECT))
        .bind(room_id).fetch_all(pool).await?)
}

pub async fn is_room_admin(pool: &Pool, room_id: i64, user_id: i64) -> Result<bool> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT role FROM room_members WHERE room_id = ? AND user_id = ?")
        .bind(room_id).bind(user_id).fetch_all(pool).await?;
    Ok(rows.first().map(|(r,)| r == "admin").unwrap_or(false))
}

pub async fn start_voting(pool: &Pool, room_id: i64, task_id: i64) -> Result<Room> {
    sqlx::query("UPDATE rooms SET status = 'voting', current_task_id = ? WHERE id = ?")
        .bind(task_id).bind(room_id).execute(pool).await?;
    sqlx::query("DELETE FROM room_votes WHERE room_id = ? AND task_id = ?")
        .bind(room_id).bind(task_id).execute(pool).await?;
    get_room(pool, room_id).await
}

const VOTE_SELECT: &str = "SELECT rv.id, rv.room_id, rv.task_id, rv.user_id, u.username, rv.value, rv.created_at FROM room_votes rv JOIN users u ON rv.user_id = u.id";

pub async fn cast_vote(pool: &Pool, room_id: i64, task_id: i64, user_id: i64, value: f64) -> Result<()> {
    let now = Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S").to_string();
    sqlx::query("INSERT INTO room_votes (room_id, task_id, user_id, value, created_at) VALUES (?, ?, ?, ?, ?) ON CONFLICT(room_id, task_id, user_id) DO UPDATE SET value = ?, created_at = ?")
        .bind(room_id).bind(task_id).bind(user_id).bind(value).bind(&now).bind(value).bind(&now)
        .execute(pool).await?;
    Ok(())
}

pub async fn reveal_votes(pool: &Pool, room_id: i64) -> Result<Room> {
    sqlx::query("UPDATE rooms SET status = 'revealed' WHERE id = ?").bind(room_id).execute(pool).await?;
    get_room(pool, room_id).await
}

pub async fn get_room_votes(pool: &Pool, room_id: i64, task_id: i64) -> Result<Vec<RoomVote>> {
    Ok(sqlx::query_as::<_, RoomVote>(&format!("{} WHERE rv.room_id = ? AND rv.task_id = ?", VOTE_SELECT))
        .bind(room_id).bind(task_id).fetch_all(pool).await?)
}

pub async fn get_room_state(pool: &Pool, room_id: i64) -> Result<RoomState> {
    let room = get_room(pool, room_id).await?;
    let members = get_room_members(pool, room_id).await?;
    let current_task = match room.current_task_id { Some(tid) => get_task(pool, tid).await.ok(), None => None };

    let votes = if let Some(tid) = room.current_task_id {
        let raw_votes = get_room_votes(pool, room_id, tid).await?;
        let revealed = room.status == "revealed";
        members.iter().map(|m| {
            let v = raw_votes.iter().find(|v| v.user_id == m.user_id);
            RoomVoteView { username: m.username.clone(), voted: v.is_some(), value: if revealed { v.and_then(|v| v.value) } else { None } }
        }).collect()
    } else { vec![] };

    let tasks = match &room.project {
        Some(p) if !p.is_empty() => {
            let pt: Vec<Task> = sqlx::query_as(&format!("{} WHERE t.project = ? ORDER BY t.sort_order", TASK_SELECT)).bind(p).fetch_all(pool).await?;
            if pt.is_empty() { list_tasks(pool, None, None).await? } else { pt }
        }
        _ => list_tasks(pool, None, None).await?,
    };

    let all_voted_tasks: Vec<(i64,)> = if let Some(ctid) = room.current_task_id {
        sqlx::query_as("SELECT DISTINCT task_id FROM room_votes WHERE room_id = ? AND task_id != ?").bind(room_id).bind(ctid).fetch_all(pool).await?
    } else {
        sqlx::query_as("SELECT DISTINCT task_id FROM room_votes WHERE room_id = ?").bind(room_id).fetch_all(pool).await?
    };

    let mut vote_history = Vec::new();
    if !all_voted_tasks.is_empty() {
        // Batch: fetch all votes for this room at once
        let all_room_votes: Vec<RoomVote> = sqlx::query_as(&format!("{} WHERE rv.room_id = ?", VOTE_SELECT))
            .bind(room_id).fetch_all(pool).await?;
        let task_titles: Vec<(i64, String)> = {
            let tids: Vec<i64> = all_voted_tasks.iter().map(|(tid,)| *tid).collect();
            if tids.is_empty() { vec![] } else {
                let ph = tids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                let sql = format!("SELECT id, title FROM tasks WHERE id IN ({})", ph);
                let mut q = sqlx::query_as::<_, (i64, String)>(&sql);
                for tid in &tids { q = q.bind(tid); }
                q.fetch_all(pool).await?
            }
        };
        let title_map: std::collections::HashMap<i64, String> = task_titles.into_iter().collect();
        for (tid,) in all_voted_tasks {
            let task_votes: Vec<RoomVote> = all_room_votes.iter().filter(|v| v.task_id == tid).cloned().collect();
            if task_votes.is_empty() { continue; }
            let task_title = title_map.get(&tid).cloned().unwrap_or_default();
            let values: Vec<f64> = task_votes.iter().filter_map(|v| v.value).collect();
            let avg = if values.is_empty() { 0.0 } else { values.iter().sum::<f64>() / values.len() as f64 };
            let consensus = !values.is_empty() && values.iter().all(|v| (*v - values[0]).abs() < 0.01);
            vote_history.push(VoteResult { task_id: tid, task_title, votes: task_votes, average: avg, consensus });
        }
    }

    Ok(RoomState { room, members, current_task, votes, tasks, vote_history })
}

pub async fn set_room_status(pool: &Pool, room_id: i64, status: &str) -> Result<()> {
    sqlx::query("UPDATE rooms SET status = ?, current_task_id = CASE WHEN ? = 'lobby' THEN NULL ELSE current_task_id END WHERE id = ?")
        .bind(status).bind(status).bind(room_id).execute(pool).await?;
    Ok(())
}

pub async fn accept_estimate(pool: &Pool, room_id: i64, task_id: i64, value: f64, unit: &str) -> Result<Task> {
    match unit {
        "hours" | "mandays" => {
            let hours = if unit == "mandays" { value * 8.0 } else { value };
            update_task(pool, task_id, None, None, None, None, None, None, Some(hours), None, None, Some("estimated"), None, None).await
        }
        _ => update_task(pool, task_id, None, None, None, None, None, Some(value as i64), None, Some(value), None, Some("estimated"), None, None).await
    }
}

pub async fn get_task_votes(pool: &Pool, task_id: i64) -> Result<Vec<RoomVote>> {
    Ok(sqlx::query_as::<_, RoomVote>(&format!("{} WHERE rv.task_id = ? ORDER BY rv.created_at DESC", VOTE_SELECT))
        .bind(task_id).fetch_all(pool).await?)
}

// --- Sprint CRUD ---

const SPRINT_SELECT: &str = "SELECT sp.id, sp.name, sp.project, sp.goal, sp.status, sp.start_date, sp.end_date, sp.created_by_id, u.username as created_by, sp.created_at, sp.updated_at FROM sprints sp JOIN users u ON sp.created_by_id = u.id";

pub async fn create_sprint(pool: &Pool, user_id: i64, name: &str, project: Option<&str>, goal: Option<&str>, start_date: Option<&str>, end_date: Option<&str>) -> Result<Sprint> {
    let now = Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S").to_string();
    let id = sqlx::query("INSERT INTO sprints (name, project, goal, start_date, end_date, created_by_id, created_at, updated_at) VALUES (?,?,?,?,?,?,?,?)")
        .bind(name).bind(project).bind(goal).bind(start_date).bind(end_date).bind(user_id).bind(&now).bind(&now)
        .execute(pool).await?.last_insert_rowid();
    get_sprint(pool, id).await
}

pub async fn get_sprint(pool: &Pool, id: i64) -> Result<Sprint> {
    Ok(sqlx::query_as::<_, Sprint>(&format!("{} WHERE sp.id = ?", SPRINT_SELECT)).bind(id).fetch_one(pool).await?)
}

pub async fn list_sprints(pool: &Pool, status: Option<&str>, project: Option<&str>) -> Result<Vec<Sprint>> {
    let mut q = format!("{} WHERE 1=1", SPRINT_SELECT);
    if status.is_some() { q.push_str(" AND sp.status = ?"); }
    if project.is_some() { q.push_str(" AND sp.project = ?"); }
    q.push_str(" ORDER BY sp.created_at DESC LIMIT 200");
    let mut query = sqlx::query_as::<_, Sprint>(&q);
    if let Some(s) = status { query = query.bind(s); }
    if let Some(p) = project { query = query.bind(p); }
    Ok(query.fetch_all(pool).await?)
}

pub async fn update_sprint(pool: &Pool, id: i64, name: Option<&str>, project: Option<Option<&str>>, goal: Option<Option<&str>>, status: Option<&str>, start_date: Option<Option<&str>>, end_date: Option<Option<&str>>) -> Result<Sprint> {
    let now = Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S%.3f").to_string();
    let current = get_sprint(pool, id).await?;
    let new_project = match project { Some(v) => v.map(|s| s.to_string()), None => current.project };
    let new_goal = match goal { Some(v) => v.map(|s| s.to_string()), None => current.goal };
    let new_start = match start_date { Some(v) => v.map(|s| s.to_string()), None => current.start_date };
    let new_end = match end_date { Some(v) => v.map(|s| s.to_string()), None => current.end_date };
    sqlx::query("UPDATE sprints SET name=?, project=?, goal=?, status=?, start_date=?, end_date=?, updated_at=? WHERE id=?")
        .bind(name.unwrap_or(&current.name)).bind(&new_project)
        .bind(&new_goal).bind(status.unwrap_or(&current.status))
        .bind(&new_start).bind(&new_end)
        .bind(&now).bind(id).execute(pool).await?;
    get_sprint(pool, id).await
}

pub async fn delete_sprint(pool: &Pool, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM sprints WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

const SPRINT_TASK_SELECT: &str = "SELECT st.sprint_id, st.task_id, st.added_by_id, u.username as added_by, st.added_at FROM sprint_tasks st JOIN users u ON st.added_by_id = u.id";

pub async fn add_sprint_tasks(pool: &Pool, sprint_id: i64, task_ids: &[i64], user_id: i64) -> Result<Vec<SprintTask>> {
    let now = Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S").to_string();
    for tid in task_ids {
        sqlx::query("INSERT OR IGNORE INTO sprint_tasks (sprint_id, task_id, added_by_id, added_at) VALUES (?,?,?,?)")
            .bind(sprint_id).bind(tid).bind(user_id).bind(&now).execute(pool).await?;
    }
    get_sprint_task_entries(pool, sprint_id).await
}

pub async fn remove_sprint_task(pool: &Pool, sprint_id: i64, task_id: i64) -> Result<()> {
    sqlx::query("DELETE FROM sprint_tasks WHERE sprint_id = ? AND task_id = ?").bind(sprint_id).bind(task_id).execute(pool).await?;
    Ok(())
}

pub async fn get_sprint_task_entries(pool: &Pool, sprint_id: i64) -> Result<Vec<SprintTask>> {
    Ok(sqlx::query_as::<_, SprintTask>(&format!("{} WHERE st.sprint_id = ?", SPRINT_TASK_SELECT)).bind(sprint_id).fetch_all(pool).await?)
}

pub async fn get_sprint_tasks(pool: &Pool, sprint_id: i64) -> Result<Vec<Task>> {
    Ok(sqlx::query_as::<_, Task>(&format!("SELECT t.id, t.parent_id, t.user_id, u.username as user, t.title, t.description, t.project, t.tags, t.priority, t.estimated, t.actual, t.estimated_hours, t.remaining_points, t.due_date, t.status, t.sort_order, t.created_at, t.updated_at FROM tasks t JOIN users u ON t.user_id = u.id JOIN sprint_tasks st ON t.id = st.task_id WHERE st.sprint_id = ? ORDER BY t.sort_order"))
        .bind(sprint_id).fetch_all(pool).await?)
}

pub async fn get_sprint_board(pool: &Pool, sprint_id: i64) -> Result<SprintBoard> {
    let tasks = get_sprint_tasks(pool, sprint_id).await?;
    let (mut todo, mut in_progress, mut done) = (Vec::new(), Vec::new(), Vec::new());
    for t in tasks {
        match t.status.as_str() { "completed" | "done" => done.push(t), "in_progress" | "active" => in_progress.push(t), _ => todo.push(t) }
    }
    Ok(SprintBoard { todo, in_progress, done })
}

pub async fn get_sprint_detail(pool: &Pool, id: i64) -> Result<SprintDetail> {
    let sprint = get_sprint(pool, id).await?;
    let tasks = get_sprint_tasks(pool, id).await?;
    let stats = get_sprint_burndown(pool, id).await?;
    Ok(SprintDetail { sprint, tasks, stats })
}

pub async fn get_sprint_burndown(pool: &Pool, sprint_id: i64) -> Result<Vec<SprintDailyStat>> {
    Ok(sqlx::query_as::<_, SprintDailyStat>("SELECT * FROM sprint_daily_stats WHERE sprint_id = ? ORDER BY date")
        .bind(sprint_id).fetch_all(pool).await?)
}

pub async fn get_global_burndown(pool: &Pool) -> Result<Vec<SprintDailyStat>> {
    Ok(sqlx::query_as::<_, SprintDailyStat>(
        "SELECT 0 as id, 0 as sprint_id, date, SUM(total_points) as total_points, SUM(done_points) as done_points, \
         SUM(total_hours) as total_hours, SUM(done_hours) as done_hours, SUM(total_tasks) as total_tasks, SUM(done_tasks) as done_tasks \
         FROM sprint_daily_stats WHERE sprint_id IN (SELECT id FROM sprints WHERE status = 'active') \
         GROUP BY date ORDER BY date"
    ).fetch_all(pool).await?)
}

pub async fn snapshot_sprint(pool: &Pool, sprint_id: i64) -> Result<SprintDailyStat> {
    let tasks = get_sprint_tasks(pool, sprint_id).await?;
    let date = Utc::now().naive_utc().format("%Y-%m-%d").to_string();
    let total_points: f64 = tasks.iter().map(|t| t.remaining_points).sum();
    let done_points: f64 = tasks.iter().filter(|t| t.status == "completed" || t.status == "done").map(|t| t.remaining_points).sum();
    let total_hours: f64 = tasks.iter().map(|t| t.estimated_hours).sum();
    let done_hours: f64 = tasks.iter().filter(|t| t.status == "completed" || t.status == "done").map(|t| t.estimated_hours).sum();
    let total_tasks = tasks.len() as i64;
    let done_tasks = tasks.iter().filter(|t| t.status == "completed" || t.status == "done").count() as i64;
    // Upsert: keep latest snapshot per day
    sqlx::query("INSERT INTO sprint_daily_stats (sprint_id, date, total_points, done_points, total_hours, done_hours, total_tasks, done_tasks) VALUES (?,?,?,?,?,?,?,?) ON CONFLICT(sprint_id, date) DO UPDATE SET total_points=excluded.total_points, done_points=excluded.done_points, total_hours=excluded.total_hours, done_hours=excluded.done_hours, total_tasks=excluded.total_tasks, done_tasks=excluded.done_tasks")
        .bind(sprint_id).bind(&date).bind(total_points).bind(done_points).bind(total_hours).bind(done_hours).bind(total_tasks).bind(done_tasks)
        .execute(pool).await?;
    Ok(sqlx::query_as::<_, SprintDailyStat>("SELECT * FROM sprint_daily_stats WHERE sprint_id = ? AND date = ?")
        .bind(sprint_id).bind(&date).fetch_one(pool).await?)
}

pub async fn snapshot_active_sprints(pool: &Pool) -> Result<()> {
    let sprints = list_sprints(pool, Some("active"), None).await?;
    for s in sprints { let _ = snapshot_sprint(pool, s.id).await; }
    Ok(())
}

pub async fn get_all_task_sprints(pool: &Pool) -> Result<Vec<TaskSprintInfo>> {
    Ok(sqlx::query_as::<_, TaskSprintInfo>(
        "SELECT st.task_id, sp.id as sprint_id, sp.name as sprint_name, sp.status as sprint_status FROM sprint_tasks st JOIN sprints sp ON st.sprint_id = sp.id ORDER BY st.task_id"
    ).fetch_all(pool).await?)
}

// --- Burn log ---

const BURN_SELECT: &str = "SELECT b.id, b.sprint_id, b.task_id, b.session_id, b.user_id, u.username, b.points, b.hours, b.source, b.note, b.cancelled, b.cancelled_by_id, cu.username as cancelled_by, b.created_at FROM burn_log b JOIN users u ON b.user_id = u.id LEFT JOIN users cu ON b.cancelled_by_id = cu.id";

pub async fn log_burn(pool: &Pool, sprint_id: Option<i64>, task_id: i64, session_id: Option<i64>, user_id: i64, points: f64, hours: f64, source: &str, note: Option<&str>) -> Result<BurnEntry> {
    let now = Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S").to_string();
    let id = sqlx::query("INSERT INTO burn_log (sprint_id, task_id, session_id, user_id, points, hours, source, note, created_at) VALUES (?,?,?,?,?,?,?,?,?)")
        .bind(sprint_id).bind(task_id).bind(session_id).bind(user_id).bind(points).bind(hours).bind(source).bind(note).bind(&now)
        .execute(pool).await?.last_insert_rowid();
    // Auto-assign user to task
    sqlx::query("INSERT OR IGNORE INTO task_assignees (task_id, user_id) VALUES (?, ?)")
        .bind(task_id).bind(user_id).execute(pool).await?;
    Ok(sqlx::query_as::<_, BurnEntry>(&format!("{} WHERE b.id = ?", BURN_SELECT)).bind(id).fetch_one(pool).await?)
}

pub async fn cancel_burn(pool: &Pool, burn_id: i64, cancelled_by_id: i64) -> Result<BurnEntry> {
    sqlx::query("UPDATE burn_log SET cancelled = 1, cancelled_by_id = ? WHERE id = ?")
        .bind(cancelled_by_id).bind(burn_id).execute(pool).await?;
    Ok(sqlx::query_as::<_, BurnEntry>(&format!("{} WHERE b.id = ?", BURN_SELECT)).bind(burn_id).fetch_one(pool).await?)
}

pub async fn get_burn(pool: &Pool, id: i64) -> Result<BurnEntry> {
    Ok(sqlx::query_as::<_, BurnEntry>(&format!("{} WHERE b.id = ?", BURN_SELECT)).bind(id).fetch_one(pool).await?)
}

pub async fn list_burns(pool: &Pool, sprint_id: i64) -> Result<Vec<BurnEntry>> {
    Ok(sqlx::query_as::<_, BurnEntry>(&format!("{} WHERE b.sprint_id = ? ORDER BY b.created_at DESC", BURN_SELECT))
        .bind(sprint_id).fetch_all(pool).await?)
}

pub async fn list_task_burns(pool: &Pool, task_id: i64) -> Result<Vec<BurnEntry>> {
    Ok(sqlx::query_as::<_, BurnEntry>(&format!("{} WHERE b.task_id = ? ORDER BY b.created_at DESC", BURN_SELECT))
        .bind(task_id).fetch_all(pool).await?)
}

pub async fn get_task_burn_total(pool: &Pool, task_id: i64) -> Result<BurnTotal> {
    Ok(sqlx::query_as::<_, BurnTotal>(
        "SELECT COALESCE(SUM(points), 0) as total_points, COALESCE(SUM(hours), 0) as total_hours, COUNT(*) as count FROM burn_log WHERE task_id = ? AND cancelled = 0"
    ).bind(task_id).fetch_one(pool).await?)
}

pub async fn get_all_burn_totals(pool: &Pool) -> Result<Vec<(i64, BurnTotal)>> {
    let rows: Vec<(i64, f64, f64, i64)> = sqlx::query_as(
        "SELECT task_id, COALESCE(SUM(points), 0), COALESCE(SUM(hours), 0), COUNT(*) FROM burn_log WHERE cancelled = 0 GROUP BY task_id"
    ).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(tid, p, h, c)| (tid, BurnTotal { total_points: p, total_hours: h, count: c })).collect())
}

pub async fn get_all_assignees(pool: &Pool) -> Result<Vec<TaskAssignee>> {
    Ok(sqlx::query_as::<_, TaskAssignee>(
        "SELECT ta.task_id, ta.user_id, u.username FROM task_assignees ta JOIN users u ON ta.user_id = u.id ORDER BY ta.task_id, u.username"
    ).fetch_all(pool).await?)
}

pub async fn get_burn_summary(pool: &Pool, sprint_id: i64) -> Result<Vec<BurnSummaryEntry>> {
    Ok(sqlx::query_as::<_, BurnSummaryEntry>(
        "SELECT DATE(b.created_at) as date, u.username, SUM(b.points) as points, SUM(b.hours) as hours, COUNT(*) as count FROM burn_log b JOIN users u ON b.user_id = u.id WHERE b.sprint_id = ? AND b.cancelled = 0 GROUP BY DATE(b.created_at), u.username ORDER BY date, u.username"
    ).bind(sprint_id).fetch_all(pool).await?)
}

pub async fn list_usernames(pool: &Pool) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT username FROM users ORDER BY username").fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(u,)| u).collect())
}

pub async fn get_task_burn_users(pool: &Pool, task_id: i64) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT u.username FROM burn_log b JOIN users u ON b.user_id = u.id WHERE b.task_id = ? AND b.cancelled = 0")
        .bind(task_id).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(u,)| u).collect())
}

pub async fn find_task_active_sprint(pool: &Pool, task_id: i64) -> Result<Option<i64>> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT sp.id FROM sprint_tasks st JOIN sprints sp ON st.sprint_id = sp.id WHERE st.task_id = ? AND sp.status = 'active' LIMIT 1")
        .bind(task_id).fetch_optional(pool).await?;
    Ok(row.map(|(id,)| id))
}

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize)]
pub struct UserConfig {
    pub user_id: i64,
    pub work_duration_min: Option<i64>,
    pub short_break_min: Option<i64>,
    pub long_break_min: Option<i64>,
    pub long_break_interval: Option<i64>,
    pub auto_start_breaks: Option<i64>,
    pub auto_start_work: Option<i64>,
    pub daily_goal: Option<i64>,
}

pub async fn get_user_config(pool: &Pool, user_id: i64) -> Result<Option<UserConfig>> {
    Ok(sqlx::query_as::<_, UserConfig>("SELECT * FROM user_configs WHERE user_id = ?").bind(user_id).fetch_optional(pool).await?)
}

pub async fn set_user_config(pool: &Pool, user_id: i64, cfg: &UserConfig) -> Result<UserConfig> {
    sqlx::query("INSERT INTO user_configs (user_id, work_duration_min, short_break_min, long_break_min, long_break_interval, auto_start_breaks, auto_start_work, daily_goal) VALUES (?,?,?,?,?,?,?,?) ON CONFLICT(user_id) DO UPDATE SET work_duration_min=excluded.work_duration_min, short_break_min=excluded.short_break_min, long_break_min=excluded.long_break_min, long_break_interval=excluded.long_break_interval, auto_start_breaks=excluded.auto_start_breaks, auto_start_work=excluded.auto_start_work, daily_goal=excluded.daily_goal")
        .bind(user_id).bind(cfg.work_duration_min).bind(cfg.short_break_min).bind(cfg.long_break_min).bind(cfg.long_break_interval).bind(cfg.auto_start_breaks).bind(cfg.auto_start_work).bind(cfg.daily_goal)
        .execute(pool).await?;
    Ok(sqlx::query_as::<_, UserConfig>("SELECT * FROM user_configs WHERE user_id = ?").bind(user_id).fetch_one(pool).await?)
}

// --- Epic Groups ---

pub async fn get_sprint_root_tasks(pool: &Pool, sprint_id: i64) -> Result<Vec<i64>> {
    let rows: Vec<(i64,)> = sqlx::query_as("SELECT task_id FROM sprint_root_tasks WHERE sprint_id = ?").bind(sprint_id).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

pub async fn add_sprint_root_task(pool: &Pool, sprint_id: i64, task_id: i64) -> Result<()> {
    sqlx::query("INSERT OR IGNORE INTO sprint_root_tasks (sprint_id, task_id) VALUES (?,?)").bind(sprint_id).bind(task_id).execute(pool).await?;
    Ok(())
}

pub async fn remove_sprint_root_task(pool: &Pool, sprint_id: i64, task_id: i64) -> Result<()> {
    sqlx::query("DELETE FROM sprint_root_tasks WHERE sprint_id = ? AND task_id = ?").bind(sprint_id).bind(task_id).execute(pool).await?;
    Ok(())
}

pub async fn get_descendant_ids(pool: &Pool, root_ids: &[i64]) -> Result<Vec<i64>> {
    let mut all: Vec<i64> = Vec::new();
    let mut queue: Vec<i64> = root_ids.to_vec();
    while let Some(pid) = queue.pop() {
        all.push(pid);
        let children: Vec<(i64,)> = sqlx::query_as("SELECT id FROM tasks WHERE parent_id = ?").bind(pid).fetch_all(pool).await?;
        for (cid,) in children { queue.push(cid); }
    }
    Ok(all)
}

// --- Epic Groups ---

pub async fn create_epic_group(pool: &Pool, name: &str, user_id: i64) -> Result<EpicGroup> {
    let now = Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S").to_string();
    let result = sqlx::query("INSERT INTO epic_groups (name, created_by, created_at, updated_at) VALUES (?,?,?,?)")
        .bind(name).bind(user_id).bind(&now).bind(&now).execute(pool).await?;
    let id = result.last_insert_rowid();
    Ok(sqlx::query_as::<_, EpicGroup>("SELECT * FROM epic_groups WHERE id = ?").bind(id).fetch_one(pool).await?)
}

pub async fn list_epic_groups(pool: &Pool) -> Result<Vec<EpicGroup>> {
    Ok(sqlx::query_as::<_, EpicGroup>("SELECT * FROM epic_groups ORDER BY id").fetch_all(pool).await?)
}

pub async fn get_epic_group_detail(pool: &Pool, id: i64) -> Result<EpicGroupDetail> {
    let group = sqlx::query_as::<_, EpicGroup>("SELECT * FROM epic_groups WHERE id = ?").bind(id).fetch_one(pool).await?;
    let task_ids: Vec<(i64,)> = sqlx::query_as("SELECT task_id FROM epic_group_tasks WHERE group_id = ?").bind(id).fetch_all(pool).await?;
    let snapshots = sqlx::query_as::<_, EpicSnapshot>("SELECT * FROM epic_snapshots WHERE group_id = ? ORDER BY date").bind(id).fetch_all(pool).await?;
    Ok(EpicGroupDetail { group, task_ids: task_ids.into_iter().map(|r| r.0).collect(), snapshots })
}

pub async fn delete_epic_group(pool: &Pool, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM epic_groups WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

pub async fn add_epic_group_task(pool: &Pool, group_id: i64, task_id: i64) -> Result<()> {
    sqlx::query("INSERT OR IGNORE INTO epic_group_tasks (group_id, task_id) VALUES (?,?)").bind(group_id).bind(task_id).execute(pool).await?;
    Ok(())
}

pub async fn remove_epic_group_task(pool: &Pool, group_id: i64, task_id: i64) -> Result<()> {
    sqlx::query("DELETE FROM epic_group_tasks WHERE group_id = ? AND task_id = ?").bind(group_id).bind(task_id).execute(pool).await?;
    Ok(())
}

pub async fn snapshot_epic_group(pool: &Pool, group_id: i64) -> Result<()> {
    let today = Utc::now().naive_utc().format("%Y-%m-%d").to_string();
    // Get all descendant tasks of the root tasks in this group
    let root_ids: Vec<(i64,)> = sqlx::query_as("SELECT task_id FROM epic_group_tasks WHERE group_id = ?").bind(group_id).fetch_all(pool).await?;
    if root_ids.is_empty() { return Ok(()); }
    let rids: Vec<i64> = root_ids.into_iter().map(|r| r.0).collect();
    let all_ids = get_descendant_ids(pool, &rids).await?;

    // Aggregate stats
    let placeholders: String = all_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let q = format!("SELECT COUNT(*), SUM(CASE WHEN status='completed' THEN 1 ELSE 0 END), \
        COALESCE(CAST(SUM(estimated) AS REAL),0), COALESCE(CAST(SUM(CASE WHEN status='completed' THEN estimated ELSE 0 END) AS REAL),0), \
        COALESCE(SUM(estimated_hours),0), COALESCE(SUM(CASE WHEN status='completed' THEN estimated_hours ELSE 0 END),0) \
        FROM tasks WHERE id IN ({})", placeholders);
    let mut qb = sqlx::query_as::<_, (i64, i64, f64, f64, f64, f64)>(&q);
    for id in &all_ids { qb = qb.bind(id); }
    let (total_tasks, done_tasks, total_points, done_points, total_hours, done_hours) = qb.fetch_one(pool).await?;

    sqlx::query("INSERT INTO epic_snapshots (group_id, date, total_tasks, done_tasks, total_points, done_points, total_hours, done_hours) \
        VALUES (?,?,?,?,?,?,?,?) ON CONFLICT(group_id, date) DO UPDATE SET total_tasks=excluded.total_tasks, done_tasks=excluded.done_tasks, \
        total_points=excluded.total_points, done_points=excluded.done_points, total_hours=excluded.total_hours, done_hours=excluded.done_hours")
        .bind(group_id).bind(&today).bind(total_tasks).bind(done_tasks).bind(total_points).bind(done_points).bind(total_hours).bind(done_hours)
        .execute(pool).await?;
    Ok(())
}

pub async fn snapshot_all_epic_groups(pool: &Pool) -> Result<()> {
    let groups: Vec<(i64,)> = sqlx::query_as("SELECT id FROM epic_groups").fetch_all(pool).await?;
    for (gid,) in groups { snapshot_epic_group(pool, gid).await?; }
    Ok(())
}

// --- Teams ---

pub async fn create_team(pool: &Pool, name: &str) -> Result<Team> {
    let now = Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S").to_string();
    let r = sqlx::query("INSERT INTO teams (name, created_at) VALUES (?,?)").bind(name).bind(&now).execute(pool).await?;
    Ok(sqlx::query_as::<_, Team>("SELECT * FROM teams WHERE id = ?").bind(r.last_insert_rowid()).fetch_one(pool).await?)
}

pub async fn list_teams(pool: &Pool) -> Result<Vec<Team>> {
    Ok(sqlx::query_as::<_, Team>("SELECT * FROM teams ORDER BY name").fetch_all(pool).await?)
}

pub async fn get_team_detail(pool: &Pool, id: i64) -> Result<TeamDetail> {
    let team = sqlx::query_as::<_, Team>("SELECT * FROM teams WHERE id = ?").bind(id).fetch_one(pool).await?;
    let members = sqlx::query_as::<_, TeamMember>("SELECT tm.team_id, tm.user_id, u.username, tm.role FROM team_members tm JOIN users u ON u.id = tm.user_id WHERE tm.team_id = ?").bind(id).fetch_all(pool).await?;
    let root_ids: Vec<(i64,)> = sqlx::query_as("SELECT task_id FROM team_root_tasks WHERE team_id = ?").bind(id).fetch_all(pool).await?;
    Ok(TeamDetail { team, members, root_task_ids: root_ids.into_iter().map(|r| r.0).collect() })
}

pub async fn delete_team(pool: &Pool, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM teams WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

pub async fn add_team_member(pool: &Pool, team_id: i64, user_id: i64, role: &str) -> Result<()> {
    sqlx::query("INSERT OR REPLACE INTO team_members (team_id, user_id, role) VALUES (?,?,?)").bind(team_id).bind(user_id).bind(role).execute(pool).await?;
    Ok(())
}

pub async fn remove_team_member(pool: &Pool, team_id: i64, user_id: i64) -> Result<()> {
    sqlx::query("DELETE FROM team_members WHERE team_id = ? AND user_id = ?").bind(team_id).bind(user_id).execute(pool).await?;
    Ok(())
}

pub async fn get_user_teams(pool: &Pool, user_id: i64) -> Result<Vec<Team>> {
    Ok(sqlx::query_as::<_, Team>("SELECT t.* FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? ORDER BY t.name").bind(user_id).fetch_all(pool).await?)
}

pub async fn add_team_root_task(pool: &Pool, team_id: i64, task_id: i64) -> Result<()> {
    sqlx::query("INSERT OR IGNORE INTO team_root_tasks (team_id, task_id) VALUES (?,?)").bind(team_id).bind(task_id).execute(pool).await?;
    Ok(())
}

pub async fn remove_team_root_task(pool: &Pool, team_id: i64, task_id: i64) -> Result<()> {
    sqlx::query("DELETE FROM team_root_tasks WHERE team_id = ? AND task_id = ?").bind(team_id).bind(task_id).execute(pool).await?;
    Ok(())
}
