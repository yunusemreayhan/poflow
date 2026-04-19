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

/// Resolve the pomodoro data directory.
/// `POMODORO_DATA_DIR` env var overrides the default `~/.local/share/pomodoro`.
pub fn data_dir() -> PathBuf {
    let dir = match std::env::var("POMODORO_DATA_DIR") {
        Ok(d) if !d.is_empty() => PathBuf::from(d),
        _ => dirs::data_dir().unwrap_or_else(|| PathBuf::from("~/.local/share")).join("pomodoro"),
    };
    std::fs::create_dir_all(&dir).ok();
    dir
}

pub(crate) fn db_path() -> PathBuf {
    data_dir().join("pomodoro.db")
}

pub async fn connect() -> Result<Pool> {
    let path = db_path();
    let opts = SqliteConnectOptions::from_str(&format!("sqlite:{}?mode=rwc", path.display()))?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_secs(10))
        .pragma("foreign_keys", "ON")
        .pragma("synchronous", "NORMAL")  // faster writes, still safe with WAL
        .pragma("cache_size", "-8000")    // 8MB page cache
        .pragma("temp_store", "MEMORY");  // temp tables in memory
    let pool = SqlitePoolOptions::new()
        .max_connections(8) // WAL allows concurrent reads
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

// INF5: Log migration errors that aren't "duplicate column" (the expected idempotent case)
fn log_migration_err(sql: &str, e: sqlx::Error) {
    let msg = e.to_string();
    if !msg.contains("duplicate column") && !msg.contains("already exists") {
        tracing::warn!("Migration warning: {} — {}", sql.chars().take(60).collect::<String>(), msg);
    }
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
    // P4: Additional indexes for common query patterns
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_notifications_user_read ON notifications(user_id, read)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_task_watchers_user ON task_watchers(user_id)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_burn_log_sprint ON burn_log(sprint_id, cancelled)").execute(pool).await.ok();
    // Performance indexes for common query patterns
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_tasks_deleted_at ON tasks(deleted_at)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_tasks_due_date ON tasks(due_date) WHERE due_date IS NOT NULL").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_sessions_task_id ON sessions(task_id)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_task_assignees_task ON task_assignees(task_id)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_task_labels_task ON task_labels(task_id)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_task_custom_values_task ON task_custom_values(task_id)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_checklist_items_task ON checklist_items(task_id)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_tasks_sort ON tasks(sort_order, id) WHERE deleted_at IS NULL").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_log_created_at ON audit_log(created_at)").execute(pool).await.ok();
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_task_links_task ON task_links(task_id)").execute(pool).await.ok();

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
        if let Err(e) = sqlx::query("ALTER TABLE sprints ADD COLUMN retro_notes TEXT").execute(pool).await { log_migration_err("ALTER TABLE sprints ADD COLUMN retro_notes TEXT", e); }
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (1, ?)").bind(now_str()).execute(pool).await.ok();
    }
    // Migration 2: Soft delete support
    if !applied_set.contains(&2) {
        if let Err(e) = sqlx::query("ALTER TABLE tasks ADD COLUMN deleted_at TEXT").execute(pool).await { log_migration_err("ALTER TABLE tasks ADD COLUMN deleted_at TEXT", e); }
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (2, ?)").bind(now_str()).execute(pool).await.ok();
    }
    // Migration 3: Notification preferences per event type
    if !applied_set.contains(&3) {
        sqlx::query("CREATE TABLE IF NOT EXISTS notification_prefs (
            user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            event_type TEXT NOT NULL,
            enabled    INTEGER NOT NULL DEFAULT 1,
            PRIMARY KEY (user_id, event_type)
        )").execute(pool).await.ok();
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (3, ?)").bind(now_str()).execute(pool).await.ok();
    }

    // Migration 4: Sprint capacity hours
    if !applied_set.contains(&4) {
        if let Err(e) = sqlx::query("ALTER TABLE sprints ADD COLUMN capacity_hours REAL").execute(pool).await { log_migration_err("ALTER TABLE sprints ADD COLUMN capacity_hours REAL", e); }
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (4, ?)").bind(now_str()).execute(pool).await.ok();
    }
    // Migration 5: Per-task work duration override
    if !applied_set.contains(&5) {
        if let Err(e) = sqlx::query("ALTER TABLE tasks ADD COLUMN work_duration_minutes INTEGER").execute(pool).await { log_migration_err("ALTER TABLE tasks ADD COLUMN work_duration_minutes INTEGER", e); }
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (5, ?)").bind(now_str()).execute(pool).await.ok();
    }
    // Migration 6: Task watchers
    if !applied_set.contains(&6) {
        sqlx::query("CREATE TABLE IF NOT EXISTS task_watchers (
            task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            created_at TEXT NOT NULL,
            PRIMARY KEY (task_id, user_id)
        )").execute(pool).await.ok();
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (6, ?)").bind(now_str()).execute(pool).await.ok();
    }
    // Migration 7: Task dependencies (table already exists from initial schema, this is a no-op)
    if !applied_set.contains(&7) {
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (7, ?)").bind(now_str()).execute(pool).await.ok();
    }
    // Migration 8: FTS5 full-text search index on tasks
    if !applied_set.contains(&8) {
        // FTS5 may not be available in all SQLite builds — skip gracefully
        let fts_ok = sqlx::query("CREATE VIRTUAL TABLE IF NOT EXISTS tasks_fts USING fts5(title, description, tags, project)").execute(pool).await.is_ok();
        if fts_ok {
            sqlx::query("INSERT OR IGNORE INTO tasks_fts(rowid, title, description, tags, project) SELECT id, COALESCE(title,''), COALESCE(description,''), COALESCE(tags,''), COALESCE(project,'') FROM tasks WHERE deleted_at IS NULL").execute(pool).await.ok();
            sqlx::query("CREATE TRIGGER IF NOT EXISTS tasks_fts_insert AFTER INSERT ON tasks BEGIN INSERT INTO tasks_fts(rowid, title, description, tags, project) VALUES (new.id, COALESCE(new.title,''), COALESCE(new.description,''), COALESCE(new.tags,''), COALESCE(new.project,'')); END").execute(pool).await.ok();
            sqlx::query("CREATE TRIGGER IF NOT EXISTS tasks_fts_update AFTER UPDATE ON tasks BEGIN DELETE FROM tasks_fts WHERE rowid=old.id; INSERT INTO tasks_fts(rowid, title, description, tags, project) SELECT new.id, COALESCE(new.title,''), COALESCE(new.description,''), COALESCE(new.tags,''), COALESCE(new.project,'') WHERE new.deleted_at IS NULL; END").execute(pool).await.ok();
            sqlx::query("CREATE TRIGGER IF NOT EXISTS tasks_fts_delete AFTER DELETE ON tasks BEGIN DELETE FROM tasks_fts WHERE rowid=old.id; END").execute(pool).await.ok();
            tasks::set_fts5_available(true);
        } else {
            tracing::warn!("FTS5 not available — search will use LIKE fallback");
            tasks::set_fts5_available(false);
        }
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (8, ?)").bind(now_str()).execute(pool).await.ok();
    }
    // Migration 9: Fix FTS5 — recreate as standalone table with proper triggers
    if !applied_set.contains(&9) {
        sqlx::query("DROP TRIGGER IF EXISTS tasks_fts_insert").execute(pool).await.ok();
        sqlx::query("DROP TRIGGER IF EXISTS tasks_fts_update").execute(pool).await.ok();
        sqlx::query("DROP TRIGGER IF EXISTS tasks_fts_delete").execute(pool).await.ok();
        sqlx::query("DROP TABLE IF EXISTS tasks_fts").execute(pool).await.ok();
        if sqlx::query("CREATE VIRTUAL TABLE IF NOT EXISTS tasks_fts USING fts5(title, description, tags, project)").execute(pool).await.is_ok() {
            sqlx::query("INSERT INTO tasks_fts(rowid, title, description, tags, project) SELECT id, COALESCE(title,''), COALESCE(description,''), COALESCE(tags,''), COALESCE(project,'') FROM tasks WHERE deleted_at IS NULL").execute(pool).await.ok();
            sqlx::query("CREATE TRIGGER IF NOT EXISTS tasks_fts_insert AFTER INSERT ON tasks BEGIN INSERT INTO tasks_fts(rowid, title, description, tags, project) VALUES (new.id, COALESCE(new.title,''), COALESCE(new.description,''), COALESCE(new.tags,''), COALESCE(new.project,'')); END").execute(pool).await.ok();
            sqlx::query("CREATE TRIGGER IF NOT EXISTS tasks_fts_update AFTER UPDATE ON tasks BEGIN DELETE FROM tasks_fts WHERE rowid=old.id; INSERT INTO tasks_fts(rowid, title, description, tags, project) SELECT new.id, COALESCE(new.title,''), COALESCE(new.description,''), COALESCE(new.tags,''), COALESCE(new.project,'') WHERE new.deleted_at IS NULL; END").execute(pool).await.ok();
            sqlx::query("CREATE TRIGGER IF NOT EXISTS tasks_fts_delete AFTER DELETE ON tasks BEGIN DELETE FROM tasks_fts WHERE rowid=old.id; END").execute(pool).await.ok();
            tasks::set_fts5_available(true);
        }
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (9, ?)").bind(now_str()).execute(pool).await.ok();
    }

    // Migration 10: Add password_changed_at to users for token invalidation after password reset
    if !applied_set.contains(&10) {
        if let Err(e) = sqlx::query("ALTER TABLE users ADD COLUMN password_changed_at TEXT").execute(pool).await { log_migration_err("ALTER TABLE users ADD COLUMN password_changed_at TEXT", e); }
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (10, ?)").bind(now_str()).execute(pool).await.ok();
    }

    // Migration 11: Achievements table
    if !applied_set.contains(&11) {
        sqlx::query("CREATE TABLE IF NOT EXISTS achievements (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            achievement_type TEXT NOT NULL,
            unlocked_at     TEXT NOT NULL,
            UNIQUE(user_id, achievement_type)
        )").execute(pool).await.ok();
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (11, ?)").bind(now_str()).execute(pool).await.ok();
    }

    // Migration 12: Task links (GitHub/GitLab commits, PRs, external URLs)
    if !applied_set.contains(&12) {
        sqlx::query("CREATE TABLE IF NOT EXISTS task_links (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id     INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            link_type   TEXT NOT NULL,
            url         TEXT NOT NULL,
            title       TEXT NOT NULL,
            created_at  TEXT NOT NULL
        )").execute(pool).await.ok();
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (12, ?)").bind(now_str()).execute(pool).await.ok();
    }

    // Migration 13: Threaded comments (parent_id for replies)
    if !applied_set.contains(&13) {
        if let Err(e) = sqlx::query("ALTER TABLE comments ADD COLUMN parent_id INTEGER REFERENCES comments(id) ON DELETE CASCADE").execute(pool).await { log_migration_err("ALTER TABLE comments ADD COLUMN parent_id", e); }
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (13, ?)").bind(now_str()).execute(pool).await.ok();
    }

    // Migration 14: PERT estimates (optimistic/pessimistic hours)
    if !applied_set.contains(&14) {
        if let Err(e) = sqlx::query("ALTER TABLE tasks ADD COLUMN estimate_optimistic REAL").execute(pool).await { log_migration_err("ALTER TABLE tasks ADD COLUMN estimate_optimistic", e); }
        if let Err(e) = sqlx::query("ALTER TABLE tasks ADD COLUMN estimate_pessimistic REAL").execute(pool).await { log_migration_err("ALTER TABLE tasks ADD COLUMN estimate_pessimistic", e); }
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (14, ?)").bind(now_str()).execute(pool).await.ok();
    }

    // Migration 15: Automation rules
    if !applied_set.contains(&15) {
        sqlx::query("CREATE TABLE IF NOT EXISTS automation_rules (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            name            TEXT NOT NULL,
            trigger_event   TEXT NOT NULL,
            condition_json  TEXT NOT NULL DEFAULT '{}',
            action_json     TEXT NOT NULL DEFAULT '{}',
            enabled         INTEGER NOT NULL DEFAULT 1,
            created_at      TEXT NOT NULL
        )").execute(pool).await.ok();
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (15, ?)").bind(now_str()).execute(pool).await.ok();
    }

    // Migration 16: Shared timer sessions (pair/mob programming)
    if !applied_set.contains(&16) {
        sqlx::query("CREATE TABLE IF NOT EXISTS session_participants (
            session_id  INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            joined_at   TEXT NOT NULL,
            PRIMARY KEY (session_id, user_id)
        )").execute(pool).await.ok();
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (16, ?)").bind(now_str()).execute(pool).await.ok();
    }

    // Migration 17: Add missing columns to user_configs (theme, notify_desktop, notify_sound)
    if !applied_set.contains(&17) {
        if let Err(e) = sqlx::query("ALTER TABLE user_configs ADD COLUMN theme TEXT").execute(pool).await { log_migration_err("ALTER TABLE user_configs ADD COLUMN theme", e); }
        if let Err(e) = sqlx::query("ALTER TABLE user_configs ADD COLUMN notify_desktop INTEGER DEFAULT 1").execute(pool).await { log_migration_err("ALTER TABLE user_configs ADD COLUMN notify_desktop", e); }
        if let Err(e) = sqlx::query("ALTER TABLE user_configs ADD COLUMN notify_sound INTEGER DEFAULT 1").execute(pool).await { log_migration_err("ALTER TABLE user_configs ADD COLUMN notify_sound", e); }
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (17, ?)").bind(now_str()).execute(pool).await.ok();
    }

    // Migration 18: Custom task statuses (Jira-like workflows)
    if !applied_set.contains(&18) {
        sqlx::query("CREATE TABLE IF NOT EXISTS custom_statuses (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL,
            color       TEXT NOT NULL DEFAULT '#6366f1',
            category    TEXT NOT NULL DEFAULT 'todo',
            sort_order  INTEGER NOT NULL DEFAULT 0,
            created_by  INTEGER NOT NULL REFERENCES users(id),
            created_at  TEXT NOT NULL
        )").execute(pool).await.ok();
        sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_custom_statuses_name ON custom_statuses(name)").execute(pool).await.ok();
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (18, ?)").bind(now_str()).execute(pool).await.ok();
    }

    // Migration 19: Custom fields on tasks (Jira-like custom fields)
    if !applied_set.contains(&19) {
        sqlx::query("CREATE TABLE IF NOT EXISTS custom_fields (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL,
            field_type  TEXT NOT NULL DEFAULT 'text',
            options     TEXT,
            required    INTEGER NOT NULL DEFAULT 0,
            sort_order  INTEGER NOT NULL DEFAULT 0,
            created_by  INTEGER NOT NULL REFERENCES users(id),
            created_at  TEXT NOT NULL
        )").execute(pool).await.ok();
        sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_custom_fields_name ON custom_fields(name)").execute(pool).await.ok();
        sqlx::query("CREATE TABLE IF NOT EXISTS task_custom_values (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id     INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            field_id    INTEGER NOT NULL REFERENCES custom_fields(id) ON DELETE CASCADE,
            value       TEXT,
            UNIQUE(task_id, field_id)
        )").execute(pool).await.ok();
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (19, ?)").bind(now_str()).execute(pool).await.ok();
    }

    // Migration 20: Task checklists (lightweight sub-items)
    if !applied_set.contains(&20) {
        sqlx::query("CREATE TABLE IF NOT EXISTS checklist_items (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id     INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            title       TEXT NOT NULL,
            checked     INTEGER NOT NULL DEFAULT 0,
            sort_order  INTEGER NOT NULL DEFAULT 0,
            created_at  TEXT NOT NULL
        )").execute(pool).await.ok();
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (20, ?)").bind(now_str()).execute(pool).await.ok();
    }

    // Migration 21: Add email column to users (for SMTP notifications)
    if !applied_set.contains(&21) {
        if let Err(e) = sqlx::query("ALTER TABLE users ADD COLUMN email TEXT").execute(pool).await { log_migration_err("ALTER TABLE users ADD COLUMN email", e); }
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (21, ?)").bind(now_str()).execute(pool).await.ok();
    }

    // Migration 22: task_attachments (previously unversioned)
    if !applied_set.contains(&22) {
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
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (22, ?)").bind(now_str()).execute(pool).await.ok();
    }

    // B12: Detect FTS5 availability for existing DBs
    if applied_set.contains(&8) {
        let fts_exists = sqlx::query("SELECT 1 FROM tasks_fts LIMIT 0").execute(pool).await.is_ok();
        tasks::set_fts5_available(fts_exists);
    }

    // Migration 23: notifications (previously unversioned)
    if !applied_set.contains(&23) {
        sqlx::query("CREATE TABLE IF NOT EXISTS notifications (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            kind        TEXT NOT NULL,
            message     TEXT NOT NULL,
            entity_type TEXT,
            entity_id   INTEGER,
            read        INTEGER NOT NULL DEFAULT 0,
            created_at  TEXT NOT NULL
        )").execute(pool).await?;
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (23, ?)").bind(now_str()).execute(pool).await.ok();
    }

    // Migration 24: Add updated_by to tasks, timezone to users
    if !applied_set.contains(&24) {
        if let Err(e) = sqlx::query("ALTER TABLE tasks ADD COLUMN updated_by INTEGER REFERENCES users(id)").execute(pool).await { log_migration_err("ALTER TABLE tasks ADD COLUMN updated_by", e); }
        if let Err(e) = sqlx::query("ALTER TABLE users ADD COLUMN timezone TEXT").execute(pool).await { log_migration_err("ALTER TABLE users ADD COLUMN timezone", e); }
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (24, ?)").bind(now_str()).execute(pool).await.ok();
    }

    // Migration 25: saved_views + webhook_deliveries
    if !applied_set.contains(&25) {
        sqlx::query("CREATE TABLE IF NOT EXISTS saved_views (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            name        TEXT NOT NULL,
            filters     TEXT NOT NULL,
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        )").execute(pool).await.ok();
        sqlx::query("CREATE TABLE IF NOT EXISTS webhook_deliveries (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            webhook_id  INTEGER NOT NULL REFERENCES webhooks(id) ON DELETE CASCADE,
            event       TEXT NOT NULL,
            status_code INTEGER,
            success     INTEGER NOT NULL DEFAULT 0,
            attempts    INTEGER NOT NULL DEFAULT 0,
            error       TEXT,
            created_at  TEXT NOT NULL
        )").execute(pool).await.ok();
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (25, ?)").bind(now_str()).execute(pool).await.ok();
    }

    // Migration 26: Projects table + project_id FK on tasks/sprints/rooms
    if !applied_set.contains(&26) {
        sqlx::query("CREATE TABLE IF NOT EXISTS projects (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL,
            description TEXT,
            key         TEXT NOT NULL,
            lead_user_id INTEGER REFERENCES users(id),
            status      TEXT NOT NULL DEFAULT 'active',
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        )").execute(pool).await.ok();
        sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_projects_key ON projects(key)").execute(pool).await.ok();
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_projects_status ON projects(status)").execute(pool).await.ok();
        // Add project_id FK columns
        if let Err(e) = sqlx::query("ALTER TABLE tasks ADD COLUMN project_id INTEGER REFERENCES projects(id)").execute(pool).await { log_migration_err("ALTER TABLE tasks ADD COLUMN project_id", e); }
        if let Err(e) = sqlx::query("ALTER TABLE sprints ADD COLUMN project_id INTEGER REFERENCES projects(id)").execute(pool).await { log_migration_err("ALTER TABLE sprints ADD COLUMN project_id", e); }
        if let Err(e) = sqlx::query("ALTER TABLE rooms ADD COLUMN project_id INTEGER REFERENCES projects(id)").execute(pool).await { log_migration_err("ALTER TABLE rooms ADD COLUMN project_id", e); }
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_tasks_project_id ON tasks(project_id)").execute(pool).await.ok();
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_sprints_project_id ON sprints(project_id)").execute(pool).await.ok();
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_rooms_project_id ON rooms(project_id)").execute(pool).await.ok();
        // Migrate existing project text values to projects table
        let now = now_str();
        let distinct_projects: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT project FROM (SELECT project FROM tasks WHERE project IS NOT NULL AND project != '' UNION SELECT project FROM sprints WHERE project IS NOT NULL AND project != '' UNION SELECT project FROM rooms WHERE project IS NOT NULL AND project != '')"
        ).fetch_all(pool).await.unwrap_or_default();
        for (name,) in &distinct_projects {
            let key = name.trim().to_lowercase().replace(|c: char| !c.is_alphanumeric(), "-");
            let key = key.trim_matches('-').to_string();
            let key = if key.is_empty() { format!("project-{}", name.len()) } else { key };
            sqlx::query("INSERT OR IGNORE INTO projects (name, description, key, status, created_at, updated_at) VALUES (?, '', ?, 'active', ?, ?)")
                .bind(name.trim()).bind(&key).bind(&now).bind(&now).execute(pool).await.ok();
        }
        // Link existing records to their projects
        sqlx::query("UPDATE tasks SET project_id = (SELECT p.id FROM projects p WHERE p.name = tasks.project) WHERE project IS NOT NULL AND project != ''").execute(pool).await.ok();
        sqlx::query("UPDATE sprints SET project_id = (SELECT p.id FROM projects p WHERE p.name = sprints.project) WHERE project IS NOT NULL AND project != ''").execute(pool).await.ok();
        sqlx::query("UPDATE rooms SET project_id = (SELECT p.id FROM projects p WHERE p.name = rooms.project) WHERE project IS NOT NULL AND project != ''").execute(pool).await.ok();
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (26, ?)").bind(now_str()).execute(pool).await.ok();
    }

    // Migration 27: Workflow transition rules
    if !applied_set.contains(&27) {
        sqlx::query("CREATE TABLE IF NOT EXISTS status_transitions (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            from_status TEXT NOT NULL,
            to_status   TEXT NOT NULL,
            project_id  INTEGER REFERENCES projects(id) ON DELETE CASCADE,
            created_at  TEXT NOT NULL,
            UNIQUE(from_status, to_status, project_id)
        )").execute(pool).await.ok();
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_status_transitions_from ON status_transitions(from_status, project_id)").execute(pool).await.ok();
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (27, ?)").bind(now_str()).execute(pool).await.ok();
    }

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
mod watchers;
pub use watchers::*;
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
pub mod webhooks;
pub use webhooks::*;
mod templates;
pub use templates::*;
mod attachments;
pub use attachments::*;
mod notifications;
pub use notifications::*;
mod custom_statuses;
pub use custom_statuses::*;
mod custom_fields;
pub use custom_fields::*;
mod checklists;
pub use checklists::*;
mod saved_views;
pub use saved_views::*;
mod webhook_deliveries;
pub use webhook_deliveries::*;
mod projects;
pub use projects::*;
mod status_transitions;
pub use status_transitions::*;