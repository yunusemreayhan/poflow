use anyhow::Result;
use chrono::{NaiveDateTime, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::FromRow;
use std::path::PathBuf;
use std::str::FromStr;

mod types;
pub use types::*;

pub fn now_str() -> String {
    Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S%.3f").to_string()
}

pub(crate) fn db_path() -> PathBuf {
    let dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("~/.local/share")).join("pomodoro");
    std::fs::create_dir_all(&dir).ok();
    dir.join("pomodoro.db")
}

pub async fn connect() -> Result<Pool> {
    let path = db_path();
    let opts = SqliteConnectOptions::from_str(&format!("sqlite:{}?mode=rwc", path.display()))?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(4) // WAL allows concurrent reads
        .min_connections(1)
        .connect_with(opts).await?;
    migrate(&pool).await?;
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).ok();
    }
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
    // F1: Soft delete support
    sqlx::query("ALTER TABLE tasks ADD COLUMN deleted_at TEXT").execute(pool).await.ok();

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
        retro_notes   TEXT,
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
        daily_goal          INTEGER,
        theme               TEXT,
        notify_desktop      INTEGER DEFAULT 1,
        notify_sound        INTEGER DEFAULT 1
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
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_burn_log_user_id ON burn_log(user_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_sprint_tasks_sprint_id ON sprint_tasks(sprint_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_sprint_tasks_task_id ON sprint_tasks(task_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_tasks_user_id ON tasks(user_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_comments_task_id ON comments(task_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_rooms_creator_id ON rooms(creator_id)").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS audit_log (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        user_id     INTEGER NOT NULL REFERENCES users(id),
        action      TEXT NOT NULL,
        entity_type TEXT NOT NULL,
        entity_id   INTEGER,
        detail      TEXT,
        created_at  TEXT NOT NULL
    )").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_log_user_id ON audit_log(user_id)").execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_log_entity ON audit_log(entity_type, entity_id)").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS labels (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        name        TEXT NOT NULL UNIQUE,
        color       TEXT NOT NULL DEFAULT '#6366f1',
        created_at  TEXT NOT NULL
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS task_labels (
        task_id     INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
        label_id    INTEGER NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
        PRIMARY KEY (task_id, label_id)
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS task_recurrence (
        task_id     INTEGER PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
        pattern     TEXT NOT NULL,
        next_due    TEXT NOT NULL,
        last_created TEXT
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS task_dependencies (
        task_id     INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
        depends_on  INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
        PRIMARY KEY (task_id, depends_on)
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS webhooks (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        url         TEXT NOT NULL,
        events      TEXT NOT NULL DEFAULT '*',
        secret      TEXT,
        active      INTEGER NOT NULL DEFAULT 1,
        created_at  TEXT NOT NULL
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS token_blocklist (
        token_hash  TEXT PRIMARY KEY,
        expires_at  TEXT NOT NULL
    )").execute(pool).await?;

    sqlx::query("CREATE TABLE IF NOT EXISTS task_templates (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        name        TEXT NOT NULL,
        data        TEXT NOT NULL,
        created_at  TEXT NOT NULL
    )").execute(pool).await?;

    // Migration versioning
    sqlx::query("CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL)").execute(pool).await?;
    let applied: Vec<(i64,)> = sqlx::query_as("SELECT version FROM schema_migrations").fetch_all(pool).await.unwrap_or_default();
    let applied_set: std::collections::HashSet<i64> = applied.into_iter().map(|r| r.0).collect();

    // Migration 1: Add retro_notes to sprints
    if !applied_set.contains(&1) {
        sqlx::query("ALTER TABLE sprints ADD COLUMN retro_notes TEXT").execute(pool).await.ok();
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (1, ?)").bind(&now_str()).execute(pool).await.ok();
    }

    sqlx::query("CREATE TABLE IF NOT EXISTS task_attachments (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        task_id     INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
        user_id     INTEGER NOT NULL REFERENCES users(id),
        filename    TEXT NOT NULL,
        mime_type   TEXT NOT NULL DEFAULT 'application/octet-stream',
        size_bytes  INTEGER NOT NULL,
        storage_key TEXT NOT NULL,
        created_at  TEXT NOT NULL
    )").execute(pool).await?;

    Ok(())
}

// --- User CRUD ---

mod users;
pub use users::*;
mod tasks;
pub use tasks::*;
mod sessions;
pub use sessions::*;
mod comments;
pub use comments::*;
mod assignees;
pub use assignees::*;
mod rooms;
pub use rooms::*;
mod sprints;
pub use sprints::*;
mod burns;
pub use burns::*;
mod epics;
pub use epics::*;
mod teams;
pub use teams::*;
mod audit;
pub use audit::*;
mod labels;
pub use labels::*;
mod recurrence;
pub use recurrence::*;
mod dependencies;
pub use dependencies::*;
mod webhooks;
pub use webhooks::*;
mod templates;
pub use templates::*;
mod attachments;
pub use attachments::*;
