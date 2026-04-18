use crate::db::{self, Pool};
use serde_json::Value;

/// Run automation rules triggered by a task status change.
/// Called after update_task when status changes.
pub async fn run_status_changed(pool: &Pool, task_id: i64, old_status: &str, new_status: &str) {
    let rules: Vec<(i64, i64, String, String)> = match sqlx::query_as(
        "SELECT id, user_id, condition_json, action_json FROM automation_rules WHERE trigger_event = 'task.status_changed' AND enabled = 1"
    ).fetch_all(pool).await {
        Ok(r) => r,
        Err(_) => return,
    };

    for (rule_id, user_id, cond_json, action_json) in rules {
        if !matches_condition(&cond_json, old_status, new_status) { continue; }
        // Check task belongs to this user (or user is admin/root)
        let task = match db::get_task(pool, task_id).await {
            Ok(t) => t,
            Err(_) => continue,
        };
        let (role,): (String,) = match sqlx::query_as("SELECT role FROM users WHERE id = ?")
            .bind(user_id).fetch_one(pool).await {
            Ok(r) => r,
            Err(_) => continue,
        };
        if task.user_id != user_id && role != "root" && role != "admin" { continue; }
        execute_action(pool, task_id, &action_json, rule_id).await;
    }
}

/// Run automation rules for "all subtasks done" trigger.
/// Called when a task is completed — checks if parent's subtasks are all done.
pub async fn run_all_subtasks_done(pool: &Pool, task_id: i64) {
    let task = match db::get_task(pool, task_id).await { Ok(t) => t, Err(_) => return };
    let parent_id = match task.parent_id { Some(p) => p, None => return };

    // Check if ALL children of parent are completed/done
    let (incomplete,): (i64,) = match sqlx::query_as(
        "SELECT COUNT(*) FROM tasks WHERE parent_id = ? AND deleted_at IS NULL AND status NOT IN ('completed','done')"
    ).bind(parent_id).fetch_one(pool).await {
        Ok(r) => r,
        Err(_) => return,
    };
    if incomplete > 0 { return; }

    let rules: Vec<(i64, String)> = match sqlx::query_as(
        "SELECT id, action_json FROM automation_rules WHERE trigger_event = 'task.all_subtasks_done' AND enabled = 1"
    ).fetch_all(pool).await {
        Ok(r) => r,
        Err(_) => return,
    };

    for (rule_id, action_json) in rules {
        execute_action(pool, parent_id, &action_json, rule_id).await;
    }
}

fn matches_condition(cond_json: &str, old_status: &str, new_status: &str) -> bool {
    let cond: Value = match serde_json::from_str(cond_json) { Ok(v) => v, Err(_) => return true };
    if let Some(to) = cond.get("to_status").and_then(|v| v.as_str()) {
        if to != new_status { return false; }
    }
    if let Some(from) = cond.get("from_status").and_then(|v| v.as_str()) {
        if from != old_status { return false; }
    }
    true
}

async fn execute_action(pool: &Pool, task_id: i64, action_json: &str, _rule_id: i64) {
    let action: Value = match serde_json::from_str(action_json) { Ok(v) => v, Err(_) => return };

    if let Some(status) = action.get("set_status").and_then(|v| v.as_str()) {
        db::update_task(pool, task_id, db::UpdateTaskOpts { status: Some(status), ..Default::default() }).await.ok();
    }
    if let Some(priority) = action.get("set_priority").and_then(|v| v.as_i64()) {
        db::update_task(pool, task_id, db::UpdateTaskOpts { priority: Some(priority), ..Default::default() }).await.ok();
    }
    tracing::debug!("Automation rule executed on task {}", task_id);
}
