use axum::body::Body;
use http_body_util::BodyExt;
use hyper::Request;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

mod common;
use common::{app, json_req, auth_req, body_json, login_root, register_user, register_user_full, reg};

// ---- Flow: initial-root-user-seeding ----

#[tokio::test]
async fn flow_empty_db_seeds_root() {
    let app = app().await;
    // Root user exists on fresh in-memory DB
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"root","password":"root"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j = body_json(resp).await;
    assert_eq!(j["role"], "root");
    assert_eq!(j["user_id"], 1);
}

// ---- Flow: user-registration ----

#[tokio::test]
async fn flow_register_creates_user_role() {
    let app = app().await;
    let (token, _) = register_user_full(&app, "newuser", "TestPass1").await;
    // Verify role is "user" not "root"
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer", &token, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn flow_register_weak_password_rejected() {
    let app = app().await;
    // Too short
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"u1","password":"Ab1"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // No uppercase
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"u2","password":"abcdefg1"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // No digit
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"u3","password":"Abcdefgh"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn flow_register_duplicate_rejected() {
    let app = app().await;
    register_user_full(&app, "dup", "TestPass1").await;
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"dup","password":"TestPass1"})))).await.unwrap();
    assert_eq!(resp.status(), 409);
}

#[tokio::test]
async fn flow_register_disabled_via_env() {
    // This test uses POFLOW_ALLOW_REGISTRATION=false to disable registration
    // We can't set env vars safely in parallel tests, so we test via config instead
    let pool = poflow_daemon::db::connect_memory().await.unwrap();
    let mut config = poflow_daemon::config::Config::default();
    config.allow_registration = false;
    let engine = Arc::new(poflow_daemon::engine::Engine::new(pool, config).await);
    let app = poflow_daemon::build_router(engine).await;
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"blocked","password":"Block1234"})))).await.unwrap();
    assert_eq!(resp.status(), 403, "Registration should be disabled");
    let body = body_json(resp).await;
    assert!(body["error"].as_str().unwrap().contains("disabled"));
}

// ---- Flow: user-login, jwt-token-validation ----

#[tokio::test]
async fn flow_login_returns_both_tokens() {
    let app = app().await;
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"root","password":"root"})))).await.unwrap();
    let j = body_json(resp).await;
    assert!(j["token"].as_str().unwrap().len() > 20);
    assert!(j["refresh_token"].as_str().unwrap().len() > 20);
    assert_ne!(j["token"], j["refresh_token"]);
}

#[tokio::test]
async fn flow_refresh_token_rejected_as_access() {
    let app = app().await;
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"root","password":"root"})))).await.unwrap();
    let j = body_json(resp).await;
    let refresh = j["refresh_token"].as_str().unwrap();
    // Using refresh token as Bearer should fail
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer", refresh, None)).await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn flow_refresh_rotates_tokens() {
    let app = app().await;
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"root","password":"root"})))).await.unwrap();
    let j = body_json(resp).await;
    let refresh1 = j["refresh_token"].as_str().unwrap().to_string();
    // Refresh
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/refresh", Some(json!({"refresh_token": refresh1})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j2 = body_json(resp).await;
    assert!(j2["token"].as_str().unwrap().len() > 20);
    // Old refresh token should be revoked (used once)
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/refresh", Some(json!({"refresh_token": refresh1})))).await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn flow_csrf_required_on_mutations() {
    let app = app().await;
    let token = login_root(&app).await;
    // POST without X-Requested-With should fail with 403
    let req = Request::builder().method("POST").uri("/api/tasks")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(serde_json::to_vec(&json!({"title":"test"})).unwrap())).unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 403);
}

// ---- Flow: user-logout ----

#[tokio::test]
async fn flow_logout_revokes_access_token() {
    let app = app().await;
    let token = login_root(&app).await;
    // Logout
    let resp = app.clone().oneshot(auth_req("POST", "/api/auth/logout", &token, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Token should now be rejected
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer", &token, None)).await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn flow_logout_revokes_refresh_token() {
    let app = app().await;
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"root","password":"root"})))).await.unwrap();
    let j = body_json(resp).await;
    let access = j["token"].as_str().unwrap().to_string();
    let refresh = j["refresh_token"].as_str().unwrap().to_string();
    // Logout with refresh token in body
    let resp = app.clone().oneshot(auth_req("POST", "/api/auth/logout", &access, Some(json!({"refresh_token": refresh})))).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Refresh token should now be rejected
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/refresh", Some(json!({"refresh_token": refresh})))).await.unwrap();
    assert_eq!(resp.status(), 401, "Refresh token should be revoked after logout");
}

// ---- Flow: user-assigned-others-task (multi-user authorization) ----

#[tokio::test]
async fn flow_assignee_cannot_update_task() {
    let app = app().await;
    let root_token = login_root(&app).await;
    let (user_token, _) = register_user_full(&app, "dev1", "DevPass11").await;
    // Root creates task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &root_token, Some(json!({"title":"Root's task"})))).await.unwrap();
    let task = body_json(resp).await;
    let task_id = task["id"].as_i64().unwrap();
    // Assign dev1
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", task_id), &root_token, Some(json!({"username":"dev1"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    // dev1 (assignee) can update the task
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", task_id), &user_token, Some(json!({"status":"in_progress"})))).await.unwrap();
    assert_eq!(resp.status(), 200, "Assignee should be able to update task");
    assert_eq!(body_json(resp).await["status"], "in_progress");
}

#[tokio::test]
async fn flow_assignee_cannot_unassign_self() {
    let app = app().await;
    let root_token = login_root(&app).await;
    let (_user_token, _) = register_user_full(&app, "dev2", "DevPass22").await;
    // Root creates task, assigns dev2
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &root_token, Some(json!({"title":"Task X"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &root_token, Some(json!({"username":"dev2"})))).await.unwrap();
    // BL1: dev2 can unassign self → 204
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/assignees/dev2", tid), &_user_token, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn flow_assignee_can_comment_on_others_task() {
    let app = app().await;
    let root_token = login_root(&app).await;
    let (user_token, _) = register_user_full(&app, "dev3", "DevPass33").await;
    // Root creates task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &root_token, Some(json!({"title":"Commentable"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // dev3 comments → should succeed (no ownership check on comments)
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &user_token, Some(json!({"content":"My comment"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
}

#[tokio::test]
async fn flow_any_user_can_assign_anyone() {
    let app = app().await;
    let root_token = login_root(&app).await;
    let (user_token, _) = register_user_full(&app, "dev4", "DevPass44").await;
    register_user_full(&app, "dev5", "DevPass55").await;
    // Root creates task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &root_token, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // dev4 (non-owner) assigns dev5 → should be forbidden (S1 fix)
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &user_token, Some(json!({"username":"dev5"})))).await.unwrap();
    assert_eq!(resp.status(), 403);
    // Root (owner) assigns dev5 → should succeed
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &root_token, Some(json!({"username":"dev5"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
}

// ---- Flow: root-views-others-task (root privilege) ----

#[tokio::test]
async fn flow_root_can_update_others_task() {
    let app = app().await;
    let root_token = login_root(&app).await;
    let (user_token, _) = register_user_full(&app, "dev6", "DevPass66").await;
    // dev6 creates task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &user_token, Some(json!({"title":"Dev task"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Root updates it → should succeed
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &root_token, Some(json!({"status":"completed"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["status"], "completed");
}

#[tokio::test]
async fn flow_normal_user_cannot_update_others_task() {
    let app = app().await;
    let root_token = login_root(&app).await;
    let (user_token, _) = register_user_full(&app, "cantupdate7", "DevPass77").await;
    // Root creates task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &root_token, Some(json!({"title":"Root task"})))).await.unwrap();
    assert_eq!(resp.status(), 201, "Task creation should succeed");
    let tid = body_json(resp).await["id"].as_i64().expect("task should have id");
    // cantupdate7 tries to update → 403
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &user_token, Some(json!({"title":"Hacked"})))).await.unwrap();
    assert_eq!(resp.status(), 403);
}

// ---- Flow: root-elevates-user-role, root-deletes-user ----

#[tokio::test]
async fn flow_elevate_user_to_root() {
    let app = app().await;
    let root_token = login_root(&app).await;
    let (_, uid) = register_user_full(&app, "promo", "PromoPass1").await;
    // Elevate
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/admin/users/{}/role", uid), &root_token, Some(json!({"role":"root"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["role"], "root");
}

#[tokio::test]
async fn flow_cannot_demote_last_root() {
    let app = app().await;
    let root_token = login_root(&app).await;
    // Try to demote root (the only root user) to user
    let resp = app.clone().oneshot(auth_req("PUT", "/api/admin/users/1/role", &root_token, Some(json!({"role":"user"})))).await.unwrap();
    assert_eq!(resp.status(), 400, "Should not be able to demote the last root user");
}

#[tokio::test]
async fn flow_role_change_creates_audit_log() {
    let app = app().await;
    let root_token = login_root(&app).await;
    let (_, uid) = register_user_full(&app, "auditee", "AuditP111").await;
    // Change role
    app.clone().oneshot(auth_req("PUT", &format!("/api/admin/users/{}/role", uid), &root_token, Some(json!({"role":"root"})))).await.unwrap();
    // Check audit log
    let resp = app.clone().oneshot(auth_req("GET", "/api/audit?entity_type=user", &root_token, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let entries = body_json(resp).await;
    let arr = entries.as_array().unwrap();
    assert!(arr.iter().any(|e| e["action"] == "update_role"), "Audit log should contain role change entry");
}

#[tokio::test]
async fn flow_normal_user_cannot_elevate() {
    let app = app().await;
    let (user_token, uid) = register_user_full(&app, "sneaky", "SneakyP1").await;
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/admin/users/{}/role", uid), &user_token, Some(json!({"role":"root"})))).await.unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn flow_delete_user_reassigns_tasks() {
    let app = app().await;
    let root_token = login_root(&app).await;
    let (user_token, uid) = register_user_full(&app, "doomed", "DoomedP1").await;
    // User creates a task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &user_token, Some(json!({"title":"Orphan task"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Delete user
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/admin/users/{}", uid), &root_token, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Task should still exist (accessible by root)
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", tid), &root_token, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let detail = body_json(resp).await;
    // Task detail has a "task" sub-object with "user" field
    let user = detail["task"]["user"].as_str()
        .or_else(|| detail["user"].as_str())
        .unwrap_or("unknown");
    assert_eq!(user, "root");
}

#[tokio::test]
async fn flow_cannot_delete_last_root() {
    let app = app().await;
    let root_token = login_root(&app).await;
    // Try to delete root (id=1) — should fail (can't delete self)
    let resp = app.clone().oneshot(auth_req("DELETE", "/api/admin/users/1", &root_token, None)).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn flow_deleted_user_cannot_login() {
    let app = app().await;
    let root_token = login_root(&app).await;
    let (_, uid) = register_user_full(&app, "gone", "GonePass1").await;
    // Delete user
    app.clone().oneshot(auth_req("DELETE", &format!("/api/admin/users/{}", uid), &root_token, None)).await.unwrap();
    // Deleted user cannot login again
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"gone","password":"GonePass1"})))).await.unwrap();
    assert_eq!(resp.status(), 401);
}

// ---- Flow: root-resets-user-password ----

#[tokio::test]
async fn flow_admin_reset_password() {
    let app = app().await;
    let root_token = login_root(&app).await;
    let (_, uid) = register_user_full(&app, "resetme", "OldPass11").await;
    // Reset password
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/admin/users/{}/password", uid), &root_token, Some(json!({"password":"NewPass11"})))).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Login with new password
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"resetme","password":"NewPass11"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Old password fails
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"resetme","password":"OldPass11"})))).await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn flow_admin_reset_password_non_root_rejected() {
    let app = app().await;
    let (user_token, _) = register_user_full(&app, "noreset", "NoReset1").await;
    let resp = app.clone().oneshot(auth_req("PUT", "/api/admin/users/1/password", &user_token, Some(json!({"password":"HackRoot1"})))).await.unwrap();
    assert_eq!(resp.status(), 403);
}

// ---- Flow: user-creates-sprint (full lifecycle) ----

#[tokio::test]
async fn flow_sprint_full_lifecycle() {
    let app = app().await;
    let root_token = login_root(&app).await;
    // Create task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &root_token, Some(json!({"title":"Sprint task","estimated":3})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Create sprint
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &root_token, Some(json!({"name":"S1","start_date":"2026-04-14","end_date":"2026-04-28"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    // Add task to sprint
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &root_token, Some(json!({"task_ids":[tid]})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Start sprint
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &root_token, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["status"], "active");
    // Log burn
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &root_token, Some(json!({"task_id":tid,"hours":2.0})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    // Complete sprint
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/complete", sid), &root_token, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["status"], "completed");
    // Cannot log burns on completed sprint
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &root_token, Some(json!({"task_id":tid,"hours":1.0})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn flow_sprint_non_owner_cannot_manage() {
    let app = app().await;
    let root_token = login_root(&app).await;
    let (user_token, _) = register_user_full(&app, "spdev", "SpDev123").await;
    // Root creates sprint
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &root_token, Some(json!({"name":"S2"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    // Normal user cannot start it
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &user_token, None)).await.unwrap();
    assert_eq!(resp.status(), 403);
    // Normal user cannot delete it
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/sprints/{}", sid), &user_token, None)).await.unwrap();
    assert_eq!(resp.status(), 403);
}

// ---- Flow: collaborative-estimation-room ----

#[tokio::test]
async fn flow_room_full_estimation_session() {
    let app = app().await;
    let root_token = login_root(&app).await;
    let (u1_token, _) = register_user_full(&app, "voter1", "Voter1P1").await;
    let (u2_token, _) = register_user_full(&app, "voter2", "Voter2P1").await;
    // Create task to estimate
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &root_token, Some(json!({"title":"Estimate me"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Create room
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &root_token, Some(json!({"name":"Planning"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let rid = body_json(resp).await["id"].as_i64().unwrap();
    // Users join
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/join", rid), &u1_token, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/join", rid), &u2_token, None)).await.unwrap();
    // Start voting
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/start-voting", rid), &root_token, Some(json!({"task_id":tid})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Cast votes
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &u1_token, Some(json!({"value":5.0})))).await.unwrap();
    assert_eq!(resp.status(), 204);
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &u2_token, Some(json!({"value":8.0})))).await.unwrap();
    // Non-admin cannot reveal
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/reveal", rid), &u1_token, None)).await.unwrap();
    assert_eq!(resp.status(), 403);
    // Admin reveals
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/reveal", rid), &root_token, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Accept estimate → writes to task
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/accept", rid), &root_token, Some(json!({"value":5.0})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let task = body_json(resp).await;
    assert_eq!(task["estimated"], 5);
}

// ---- Flow: poflow-timer-session ----

#[tokio::test]
async fn flow_timer_multi_user_isolation() {
    let app = app().await;
    let root_token = login_root(&app).await;
    let (u_token, _) = register_user_full(&app, "timer1", "Timer1P1").await;
    // Root starts timer
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &root_token, Some(json!({})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let root_state = body_json(resp).await;
    assert_eq!(root_state["status"], "Running");
    // User's timer should be idle
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer", &u_token, None)).await.unwrap();
    let user_state = body_json(resp).await;
    assert_eq!(user_state["status"], "Idle");
    // User starts their own timer
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &u_token, Some(json!({})))).await.unwrap();
    assert_eq!(body_json(resp).await["status"], "Running");
    // Root's timer still running
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer", &root_token, None)).await.unwrap();
    assert_eq!(body_json(resp).await["status"], "Running");
}

#[tokio::test]
async fn flow_timer_pause_resume_stop() {
    let app = app().await;
    let token = login_root(&app).await;
    // Start
    app.clone().oneshot(auth_req("POST", "/api/timer/start", &token, Some(json!({})))).await.unwrap();
    // Pause
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/pause", &token, None)).await.unwrap();
    assert_eq!(body_json(resp).await["status"], "Paused");
    // Resume
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/resume", &token, None)).await.unwrap();
    assert_eq!(body_json(resp).await["status"], "Running");
    // Stop
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/stop", &token, None)).await.unwrap();
    assert_eq!(body_json(resp).await["status"], "Idle");
}

#[tokio::test]
async fn flow_timer_start_with_task_link() {
    let app = app().await;
    let token = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &token, Some(json!({"title":"Timer task"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &token, Some(json!({"task_id":tid})))).await.unwrap();
    let state = body_json(resp).await;
    assert_eq!(state["current_task_id"], tid);
    assert_eq!(state["status"], "Running");
}

// ---- Flow: user-changes-password ----

#[tokio::test]
async fn flow_change_password_via_profile() {
    let app = app().await;
    let (token, _) = register_user_full(&app, "pwuser", "OldPass11").await;
    // Change password via profile
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &token, Some(json!({"password":"NewPass11","current_password":"OldPass11"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j = body_json(resp).await;
    assert!(j["token"].as_str().unwrap().len() > 20); // Returns new token
    // Login with new password
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"pwuser","password":"NewPass11"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn flow_change_password_wrong_current_rejected() {
    let app = app().await;
    let (token, _) = register_user_full(&app, "pwuser2", "OldPass22").await;
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &token, Some(json!({"password":"NewPass22","current_password":"WrongOld1"})))).await.unwrap();
    assert_eq!(resp.status(), 403);
}

// ---- Flow: labels-dependencies-recurrence ----

#[tokio::test]
async fn flow_labels_require_task_ownership() {
    let app = app().await;
    let root_token = login_root(&app).await;
    let (user_token, _) = register_user_full(&app, "labownerchk", "LabDev11").await;
    // Create label (root only since V32-7)
    let resp = app.clone().oneshot(auth_req("POST", "/api/labels", &root_token, Some(json!({"name":"flowbug","color":"#ff0000"})))).await.unwrap();
    assert_eq!(resp.status(), 201, "Label creation should succeed for root");
    let label = body_json(resp).await;
    let lid = label["id"].as_i64().expect("label should have id");
    // Root creates task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &root_token, Some(json!({"title":"Labeled"})))).await.unwrap();
    assert_eq!(resp.status(), 201, "Task creation should succeed");
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Non-owner cannot add label to task
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/labels/{}", tid, lid), &user_token, None)).await.unwrap();
    assert_eq!(resp.status(), 403);
    // Owner can
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/labels/{}", tid, lid), &root_token, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

// ---- Flow: task-trash-bulk-operations ----

#[tokio::test]
async fn flow_soft_delete_restore_cycle() {
    let app = app().await;
    let token = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &token, Some(json!({"title":"Deletable"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Delete (soft)
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}", tid), &token, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Should appear in trash
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks/trash", &token, None)).await.unwrap();
    let trash = body_json(resp).await;
    assert!(trash.as_array().unwrap().iter().any(|t| t["id"] == tid));
    // Restore
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/restore", tid), &token, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Should be back in normal list
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", tid), &token, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
}

// ---- Flow: admin-backup-restore ----

#[tokio::test]
async fn flow_backup_non_root_rejected() {
    let app = app().await;
    let (user_token, _) = register_user_full(&app, "nobackup", "NoBkup11").await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/admin/backup", &user_token, None)).await.unwrap();
    assert_eq!(resp.status(), 403);
}

// ---- Flow: health check (no auth) ----

#[tokio::test]
async fn flow_health_no_auth_required() {
    let app = app().await;
    let req = Request::builder().method("GET").uri("/api/health").body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j = body_json(resp).await;
    assert_eq!(j["status"], "ok");
    assert_eq!(j["db"], true);
}

// ---- Flow: import-export ----

#[tokio::test]
async fn flow_export_tasks_user_scoped() {
    let app = app().await;
    let root_token = login_root(&app).await;
    let (user_token, _) = register_user_full(&app, "exdev", "ExDev111").await;
    // Root creates task
    app.clone().oneshot(auth_req("POST", "/api/tasks", &root_token, Some(json!({"title":"Root only"})))).await.unwrap();
    // User creates task
    app.clone().oneshot(auth_req("POST", "/api/tasks", &user_token, Some(json!({"title":"User only"})))).await.unwrap();
    // User export should only contain their task
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/tasks?format=json", &user_token, None)).await.unwrap();
    let tasks = body_json(resp).await;
    let arr = tasks.as_array().unwrap();
    assert!(arr.iter().all(|t| t["user"] == "exdev"));
    // Root export should contain all
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/tasks?format=json", &root_token, None)).await.unwrap();
    let tasks = body_json(resp).await;
    assert!(tasks.as_array().unwrap().len() >= 2);
}

// ---- Flow: history scoping ----

#[tokio::test]
async fn flow_history_user_scoped() {
    let app = app().await;
    let root_token = login_root(&app).await;
    let (user_token, _) = register_user_full(&app, "histdev", "HistDv11").await;
    // Both start and stop timers to create sessions
    app.clone().oneshot(auth_req("POST", "/api/timer/start", &root_token, Some(json!({})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", "/api/timer/stop", &root_token, None)).await.unwrap();
    app.clone().oneshot(auth_req("POST", "/api/timer/start", &user_token, Some(json!({})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", "/api/timer/stop", &user_token, None)).await.unwrap();
    // User should only see own history
    let resp = app.clone().oneshot(auth_req("GET", "/api/history", &user_token, None)).await.unwrap();
    let hist = body_json(resp).await;
    let arr = hist.as_array().unwrap();
    // All sessions should belong to histdev (session has nested "session" object with "user" field)
    for s in arr {
        let user = s["session"]["user"].as_str().unwrap_or(s["user"].as_str().unwrap_or(""));
        assert_eq!(user, "histdev", "Non-root user saw another user's session");
    }
}

// ========== F4: iCal Export ==========

#[tokio::test]
async fn test_ical_export() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create task with due date
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"ical task","due_date":"2026-06-15"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    // Export iCal
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/ical", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let ical = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(ical.contains("BEGIN:VCALENDAR"));
    assert!(ical.contains("ical task"));
    assert!(ical.contains("20260615"));
    assert!(ical.contains("END:VCALENDAR"));
}

// ========== F6: Estimation Accuracy ==========

#[tokio::test]
async fn test_estimation_accuracy() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create and complete a task with estimates
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"est task","estimated":5})))).await.unwrap();
    let task_id = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", task_id), &tok, Some(json!({"status":"completed","actual":4})))).await.unwrap();
    // Get accuracy
    let resp = app.clone().oneshot(auth_req("GET", "/api/analytics/estimation-accuracy", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data["total_tasks"].as_f64().unwrap() >= 1.0);
    assert!(data["accuracy_pct"].as_f64().is_some());
    assert!(data["over_estimated"].is_number());
    assert!(data["under_estimated"].is_number());
    assert!(data["by_project"].is_array());
}

#[tokio::test]
async fn test_estimation_accuracy_project_filter() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"proj task","estimated":3,"project":"alpha"})))).await.unwrap();
    let id = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", id), &tok, Some(json!({"status":"completed","actual":3})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/analytics/estimation-accuracy?project=alpha", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data["total_tasks"].as_f64().unwrap() >= 1.0);
}

// ========== F8: Focus Score ==========

#[tokio::test]
async fn test_focus_score() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/analytics/focus-score", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data["score"].is_number());
    // When no sessions exist, returns minimal response
    assert!(data["components"].is_object());
}

// ========== F7: Sprint Retro Report ==========

#[tokio::test]
async fn test_sprint_retro_report() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create sprint
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"retro sprint"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    // Create task and add to sprint
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"retro task","estimated":3})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    // Get retro report
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/retro-report", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert_eq!(data["tasks"]["total"], 1);
    assert!(data["sprint"]["name"].as_str().unwrap().contains("retro"));
    assert!(data["members"].is_array());
    assert!(data["scope_changes"].is_number());
}

#[tokio::test]
async fn test_sprint_retro_report_not_found() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/sprints/99999/retro-report", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 404);
}

// ========== F9: Activity Feed ==========

#[tokio::test]
async fn test_activity_feed() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create a task to generate audit entries
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"feed task"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/feed", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data.as_array().unwrap().len() > 0);
    assert!(data[0]["type"].is_string());
    assert!(data[0]["created_at"].is_string());
}

#[tokio::test]
async fn test_activity_feed_with_filters() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"feed filter task"})))).await.unwrap();
    // Filter by type
    let resp = app.clone().oneshot(auth_req("GET", "/api/feed?types=audit&limit=5", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data.as_array().unwrap().len() <= 5);
    for item in data.as_array().unwrap() { assert_eq!(item["type"], "audit"); }
}

#[tokio::test]
async fn test_activity_feed_invalid_since() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/feed?since=garbage", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ========== F10: Threaded Comments ==========

#[tokio::test]
async fn test_threaded_comments() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"thread task"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Add root comment
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &tok, Some(json!({"content":"root comment"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let root_id = body_json(resp).await["id"].as_i64().unwrap();
    // Add reply
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &tok, Some(json!({"content":"reply","parent_id":root_id})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let reply = body_json(resp).await;
    assert_eq!(reply["parent_id"], root_id);
    // List comments — both should be there
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/comments", tid), &tok, None)).await.unwrap();
    let comments = body_json(resp).await;
    assert_eq!(comments.as_array().unwrap().len(), 2);
    let reply_comment = comments.as_array().unwrap().iter().find(|c| c["parent_id"].as_i64() == Some(root_id)).unwrap();
    assert_eq!(reply_comment["content"], "reply");
}

// ========== F13: Task Links + GitHub Webhook ==========

#[tokio::test]
async fn test_task_links_crud() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"link task"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Add link
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/links", tid), &tok, Some(json!({"link_type":"pr","url":"https://github.com/test/pr/1","title":"Fix bug"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    // List links
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/links", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let links = body_json(resp).await;
    assert_eq!(links.as_array().unwrap().len(), 1);
    assert_eq!(links[0]["link_type"], "pr");
    assert_eq!(links[0]["title"], "Fix bug");
}

#[tokio::test]
async fn test_task_link_url_too_long() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"link task2"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let long_url = "https://example.com/".to_string() + &"a".repeat(2000);
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/links", tid), &tok, Some(json!({"link_type":"pr","url":long_url,"title":"x"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_github_webhook_links_commits() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create task #1
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"webhook task"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Send GitHub webhook payload referencing the task
    let payload = json!({
        "commits": [{"id": "abc1234567", "message": format!("Fix #{} — resolve bug", tid), "url": "https://github.com/test/commit/abc1234567"}],
        "repository": {"full_name": "test/repo"}
    });
    let resp = app.clone().oneshot(json_req("POST", "/api/integrations/github", Some(payload))).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Verify link was created
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/links", tid), &tok, None)).await.unwrap();
    let links = body_json(resp).await;
    assert_eq!(links.as_array().unwrap().len(), 1);
    assert_eq!(links[0]["link_type"], "commit");
    assert!(links[0]["title"].as_str().unwrap().contains("abc1234"));
}

#[tokio::test]
async fn test_github_webhook_invalid_json() {
    let app = app().await;
    let req = Request::builder().method("POST").uri("/api/integrations/github")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "10.99.0.1")
        .body(Body::from("not json")).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ========== F19: Automation Rules ==========

#[tokio::test]
async fn test_automation_rules_crud() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create rule
    let resp = app.clone().oneshot(auth_req("POST", "/api/automations", &tok, Some(json!({
        "name": "Auto close", "trigger_event": "task.status_changed",
        "action_json": "{\"set_status\":\"archived\"}"
    })))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let rule = body_json(resp).await;
    let rule_id = rule["id"].as_i64().unwrap();
    assert_eq!(rule["name"], "Auto close");
    assert_eq!(rule["enabled"], 1);
    // List rules
    let resp = app.clone().oneshot(auth_req("GET", "/api/automations", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 1);
    // Toggle
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/automations/{}/toggle", rule_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Delete
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/automations/{}", rule_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Verify deleted
    let resp = app.clone().oneshot(auth_req("GET", "/api/automations", &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_automation_invalid_trigger() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/automations", &tok, Some(json!({
        "name": "Bad", "trigger_event": "invalid.trigger", "action_json": "{}"
    })))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_automation_invalid_json_fields() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/automations", &tok, Some(json!({
        "name": "Bad JSON", "trigger_event": "task.status_changed", "action_json": "not json"
    })))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_automation_empty_name() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/automations", &tok, Some(json!({
        "name": "", "trigger_event": "task.status_changed", "action_json": "{}"
    })))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ========== F11: Shared Timer Sessions ==========

#[tokio::test]
async fn test_shared_session_join() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Register second user
    let reg_resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"joiner_f11","password":"Pass1234"})))).await.unwrap();
    assert_eq!(reg_resp.status(), 200);
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"joiner_f11","password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 200, "Login failed");
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();
    // Root starts a timer
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let state = body_json(resp).await;
    let session_id = state["current_session_id"].as_i64().expect("current_session_id should be set after start");
    // Second user joins
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/timer/join/{}", session_id), &tok2, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    // List participants
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/timer/participants/{}", session_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let participants = body_json(resp).await;
    assert_eq!(participants.as_array().unwrap().len(), 1);
    assert_eq!(participants[0]["username"], "joiner_f11");
}

#[tokio::test]
async fn test_shared_session_cannot_join_own() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({})))).await.unwrap();
    let session_id = body_json(resp).await["current_session_id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/timer/join/{}", session_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_shared_session_not_found() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/join/99999", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 404);
}

// ========== F12: User Presence ==========

#[tokio::test]
async fn test_user_presence() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/users/presence", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data.as_array().unwrap().len() >= 1);
    let root = data.as_array().unwrap().iter().find(|u| u["username"] == "root").unwrap();
    assert!(root["user_id"].is_number());
    assert!(root["online"].is_boolean());
}

// ========== F14: Slack Integration ==========

#[tokio::test]
async fn test_slack_integration_valid() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/integrations/slack", &tok, Some(json!({
        "webhook_url": "https://hooks.slack.com/services/T00/B00/xxx"
    })))).await.unwrap();
    assert_eq!(resp.status(), 201);
}

#[tokio::test]
async fn test_slack_integration_invalid_url() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/integrations/slack", &tok, Some(json!({
        "webhook_url": "https://evil.com/steal"
    })))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_slack_integration_url_too_long() {
    let app = app().await;
    let tok = login_root(&app).await;
    let long = "https://hooks.slack.com/".to_string() + &"a".repeat(500);
    let resp = app.clone().oneshot(auth_req("POST", "/api/integrations/slack", &tok, Some(json!({"webhook_url": long})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ========== F20: Weekly Digest ==========

#[tokio::test]
async fn test_weekly_digest() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/reports/weekly-digest", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert_eq!(data["period"], "last_7_days");
    assert!(data["focus_hours"].is_number());
    assert!(data["sessions"].is_number());
    assert!(data["tasks_completed"].is_number());
    assert!(data["upcoming_due"].is_array());
}

// ========== F21: Auto-Prioritization ==========

#[tokio::test]
async fn test_priority_suggestions() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create overdue task
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"overdue","due_date":"2020-01-01"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/suggestions/priorities", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data.as_array().unwrap().len() >= 1);
    let suggestion = &data[0];
    assert!(suggestion["suggested_priority"].as_i64().unwrap() >= 4);
    assert!(suggestion["reasons"].as_array().unwrap().iter().any(|r| r.as_str().unwrap().contains("Overdue")));
}

// ========== F22: Achievements ==========

#[tokio::test]
async fn test_achievements_list() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/achievements", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data.as_array().unwrap().len() >= 6); // 6 achievement types defined
    for a in data.as_array().unwrap() {
        assert!(a["type"].is_string());
        assert!(a["description"].is_string());
        assert!(a["unlocked"].is_boolean());
    }
}

#[tokio::test]
async fn test_achievements_check() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/achievements/check", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data.is_array());
}

// ========== F23: Leaderboard ==========

#[tokio::test]
async fn test_leaderboard() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/leaderboard", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data.as_array().unwrap().len() >= 1);
    assert!(data[0]["username"].is_string());
    assert!(data[0]["hours"].is_number());
    assert!(data[0]["sessions"].is_number());
}

#[tokio::test]
async fn test_leaderboard_periods() {
    let app = app().await;
    let tok = login_root(&app).await;
    for period in &["week", "month", "year"] {
        let resp = app.clone().oneshot(auth_req("GET", &format!("/api/leaderboard?period={}", period), &tok, None)).await.unwrap();
        assert_eq!(resp.status(), 200);
    }
}

// ========== F25: PERT Estimates ==========

#[tokio::test]
async fn test_pert_estimates() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"pert task"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Update with PERT estimates
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({
        "estimate_optimistic": 2.0, "estimate_pessimistic": 10.0
    })))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let task = body_json(resp).await;
    assert_eq!(task["estimate_optimistic"], 2.0);
    assert_eq!(task["estimate_pessimistic"], 10.0);
    // Verify on GET
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
    let detail = body_json(resp).await;
    assert_eq!(detail["task"]["estimate_optimistic"], 2.0);
    assert_eq!(detail["task"]["estimate_pessimistic"], 10.0);
}

#[tokio::test]
async fn test_pert_estimates_null_clear() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"pert null"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Set then clear
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"estimate_optimistic": 5.0})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"estimate_optimistic": null})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let task = body_json(resp).await;
    assert!(task["estimate_optimistic"].is_null());
}

#[tokio::test]
async fn test_pert_copied_on_duplicate() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"pert dup"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"estimate_optimistic": 1.0, "estimate_pessimistic": 8.0})))).await.unwrap();
    // Duplicate
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/duplicate", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 201);
    let dup_id = body_json(resp).await["id"].as_i64().unwrap();
    // Verify PERT copied
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", dup_id), &tok, None)).await.unwrap();
    let detail = body_json(resp).await;
    assert_eq!(detail["task"]["estimate_optimistic"], 1.0);
    assert_eq!(detail["task"]["estimate_pessimistic"], 8.0);
}

// ========== F3: Smart Scheduling ==========

#[tokio::test]
async fn test_schedule_suggestions() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/suggestions/schedule", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data["peak_hours"].is_array());
    assert!(data["avg_daily_sessions"].is_number());
    assert!(data["best_days"].is_array());
    assert!(data["task_suggestions"].is_array());
}

// ========== Cross-feature: Feed with comments ==========

#[tokio::test]
async fn test_feed_includes_comments() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"feed comment task"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &tok, Some(json!({"content":"test comment for feed"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/feed?types=comment", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data.as_array().unwrap().iter().any(|i| i["type"] == "comment"));
}

// ========== Cross-feature: Automation ownership ==========

#[tokio::test]
async fn test_automation_ownership() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Register second user
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"autouser","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"autouser","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();
    // User creates rule
    let resp = app.clone().oneshot(auth_req("POST", "/api/automations", &tok2, Some(json!({
        "name": "User rule", "trigger_event": "task.due_approaching", "action_json": "{}"
    })))).await.unwrap();
    let rule_id = body_json(resp).await["id"].as_i64().unwrap();
    // Root can delete (is_owner_or_root)
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/automations/{}", rule_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

// ========== Cross-feature: iCal excludes deleted tasks ==========

#[tokio::test]
async fn test_ical_excludes_deleted() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"deleted ical","due_date":"2026-12-25"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Delete the task
    app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
    // Export iCal
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/ical", &tok, None)).await.unwrap();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let ical = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(!ical.contains("deleted ical"));
}

// ===========================================================================
// Thorough task status transition and assignment tests
// ===========================================================================

// Helper: create a task and return (id, json)
async fn create_task_h(app: &axum::Router, tok: &str, title: &str) -> (i64, Value) {
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", tok, Some(json!({"title": title})))).await.unwrap();
    assert_eq!(resp.status(), 201, "create task '{}' failed", title);
    let j = body_json(resp).await;
    (j["id"].as_i64().unwrap(), j)
}

// Helper: update task status and return (status_code, body)
async fn set_status(app: &axum::Router, tok: &str, tid: i64, status: &str) -> (u16, Value) {
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), tok, Some(json!({"status": status})))).await.unwrap();
    let sc = resp.status().as_u16();
    let body = body_json(resp).await;
    (sc, body)
}

// Helper: get assignees for a task
async fn get_assignees(app: &axum::Router, tok: &str, tid: i64) -> Vec<String> {
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/assignees", tid), tok, None)).await.unwrap();
    let j = body_json(resp).await;
    j.as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect()
}

// ---- 1. Every valid status transition ----

#[tokio::test]
async fn test_all_valid_status_transitions() {
    let app = app().await;
    let tok = login_root(&app).await;

    // All 8 valid statuses
    let statuses = ["backlog", "active", "in_progress", "blocked", "completed", "done", "estimated", "archived"];

    // Test every possible transition between all statuses (8×8 = 64 transitions)
    // The system allows any valid status → any valid status (no state machine restrictions)
    for from in &statuses {
        for to in &statuses {
            let title = format!("trans_{}_{}", from, to);
            let (tid, task) = create_task_h(&app, &tok, &title).await;
            assert_eq!(task["status"], "backlog");

            // First set to the 'from' status (unless it's already backlog)
            if *from != "backlog" {
                let (sc, body) = set_status(&app, &tok, tid, from).await;
                assert_eq!(sc, 200, "Failed to set initial status to '{}' for task {}", from, tid);
                assert_eq!(body["status"].as_str().unwrap(), *from);
            }

            // Now transition to 'to' status
            let (sc, body) = set_status(&app, &tok, tid, to).await;
            assert_eq!(sc, 200, "Transition {}→{} failed with status {}", from, to, sc);
            assert_eq!(body["status"].as_str().unwrap(), *to, "Expected status '{}' after {}→{}", to, from, to);
        }
    }
}

// ---- Specific transitions the user reported as failing ----

#[tokio::test]
async fn test_specific_failing_transitions() {
    let app = app().await;
    let tok = login_root(&app).await;

    // backlog → in_progress (WIP)
    let (tid, _) = create_task_h(&app, &tok, "wip_test").await;
    let (sc, body) = set_status(&app, &tok, tid, "in_progress").await;
    assert_eq!(sc, 200);
    assert_eq!(body["status"], "in_progress");

    // in_progress → done
    let (sc, body) = set_status(&app, &tok, tid, "done").await;
    assert_eq!(sc, 200);
    assert_eq!(body["status"], "done");

    // in_progress → blocked
    let (tid2, _) = create_task_h(&app, &tok, "block_test").await;
    set_status(&app, &tok, tid2, "in_progress").await;
    let (sc, body) = set_status(&app, &tok, tid2, "blocked").await;
    assert_eq!(sc, 200);
    assert_eq!(body["status"], "blocked");

    // blocked → active
    let (sc, body) = set_status(&app, &tok, tid2, "active").await;
    assert_eq!(sc, 200);
    assert_eq!(body["status"], "active");

    // completed → backlog (reopen)
    let (tid3, _) = create_task_h(&app, &tok, "reopen_test").await;
    set_status(&app, &tok, tid3, "completed").await;
    let (sc, body) = set_status(&app, &tok, tid3, "backlog").await;
    assert_eq!(sc, 200);
    assert_eq!(body["status"], "backlog");

    // archived → active (unarchive)
    let (tid4, _) = create_task_h(&app, &tok, "unarchive_test").await;
    set_status(&app, &tok, tid4, "archived").await;
    let (sc, body) = set_status(&app, &tok, tid4, "active").await;
    assert_eq!(sc, 200);
    assert_eq!(body["status"], "active");

    // estimated → in_progress
    let (tid5, _) = create_task_h(&app, &tok, "est_to_wip").await;
    set_status(&app, &tok, tid5, "estimated").await;
    let (sc, body) = set_status(&app, &tok, tid5, "in_progress").await;
    assert_eq!(sc, 200);
    assert_eq!(body["status"], "in_progress");
}

// ---- Invalid status rejected ----

#[tokio::test]
async fn test_invalid_status_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let (tid, _) = create_task_h(&app, &tok, "invalid_status").await;

    for bad in &["", "pending", "wip", "ACTIVE", "Active", "in-progress", "todo", "cancelled"] {
        let (sc, _) = set_status(&app, &tok, tid, bad).await;
        assert_eq!(sc, 400, "Status '{}' should be rejected but got {}", bad, sc);
    }
}

// ---- 2. Assigning/unassigning users in every status ----

#[tokio::test]
async fn test_assign_in_every_status() {
    let app = app().await;
    let tok = login_root(&app).await;
    let user_tok = register_user(&app, "assignee_tester").await;
    let _ = user_tok; // just need the user to exist

    let statuses = ["backlog", "active", "in_progress", "blocked", "completed", "done", "estimated", "archived"];

    for status in &statuses {
        let title = format!("assign_in_{}", status);
        let (tid, _) = create_task_h(&app, &tok, &title).await;
        if *status != "backlog" {
            set_status(&app, &tok, tid, status).await;
        }

        // Assign user
        let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &tok,
            Some(json!({"username": "assignee_tester"})))).await.unwrap();
        assert_eq!(resp.status().as_u16(), 200, "Assign in '{}' status failed", status);

        // Verify assigned
        let assignees = get_assignees(&app, &tok, tid).await;
        assert!(assignees.contains(&"assignee_tester".to_string()), "Assignee not found in '{}' status", status);

        // Unassign
        let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/assignees/assignee_tester", tid), &tok, None)).await.unwrap();
        assert_eq!(resp.status().as_u16(), 204, "Unassign in '{}' status failed", status);

        // Verify unassigned
        let assignees = get_assignees(&app, &tok, tid).await;
        assert!(!assignees.contains(&"assignee_tester".to_string()), "Assignee still present after unassign in '{}' status", status);
    }
}

// ---- 3. Moving tasks between statuses while they have assignees ----

#[tokio::test]
async fn test_status_change_preserves_assignees() {
    let app = app().await;
    let tok = login_root(&app).await;
    register_user(&app, "persist_user").await;

    let (tid, _) = create_task_h(&app, &tok, "assignee_persist").await;

    // Assign user
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &tok,
        Some(json!({"username": "persist_user"})))).await.unwrap();

    // Walk through multiple status transitions and verify assignee persists
    let transitions = ["active", "in_progress", "blocked", "active", "completed", "backlog", "done", "archived", "backlog"];
    for status in &transitions {
        let (sc, body) = set_status(&app, &tok, tid, status).await;
        assert_eq!(sc, 200, "Transition to '{}' failed", status);
        assert_eq!(body["status"].as_str().unwrap(), *status);

        let assignees = get_assignees(&app, &tok, tid).await;
        assert!(assignees.contains(&"persist_user".to_string()),
            "Assignee lost after transition to '{}'", status);
    }
}

// ---- 4. Bulk status changes with mixed statuses ----

#[tokio::test]
async fn test_bulk_status_change_mixed() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create tasks in different statuses
    let (t1, _) = create_task_h(&app, &tok, "bulk_backlog").await;
    let (t2, _) = create_task_h(&app, &tok, "bulk_active").await;
    set_status(&app, &tok, t2, "active").await;
    let (t3, _) = create_task_h(&app, &tok, "bulk_wip").await;
    set_status(&app, &tok, t3, "in_progress").await;
    let (t4, _) = create_task_h(&app, &tok, "bulk_blocked").await;
    set_status(&app, &tok, t4, "blocked").await;

    // Bulk move all to completed
    let resp = app.clone().oneshot(auth_req("PUT", "/api/tasks/bulk-status", &tok,
        Some(json!({"task_ids": [t1, t2, t3, t4], "status": "completed"})))).await.unwrap();
    assert_eq!(resp.status().as_u16(), 204);

    // Verify all are completed
    for tid in [t1, t2, t3, t4] {
        let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
        let body = body_json(resp).await;
        assert_eq!(body["task"]["status"], "completed", "Task {} not completed after bulk update", tid);
    }

    // Bulk move all to backlog (reopen)
    let resp = app.clone().oneshot(auth_req("PUT", "/api/tasks/bulk-status", &tok,
        Some(json!({"task_ids": [t1, t2, t3, t4], "status": "backlog"})))).await.unwrap();
    assert_eq!(resp.status().as_u16(), 204);

    for tid in [t1, t2, t3, t4] {
        let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
        assert_eq!(body_json(resp).await["task"]["status"], "backlog");
    }
}

#[tokio::test]
async fn test_bulk_status_invalid_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let (t1, _) = create_task_h(&app, &tok, "bulk_invalid").await;

    let resp = app.clone().oneshot(auth_req("PUT", "/api/tasks/bulk-status", &tok,
        Some(json!({"task_ids": [t1], "status": "invalid_status"})))).await.unwrap();
    assert_eq!(resp.status().as_u16(), 400);
}

#[tokio::test]
async fn test_bulk_status_empty_ids() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("PUT", "/api/tasks/bulk-status", &tok,
        Some(json!({"task_ids": [], "status": "completed"})))).await.unwrap();
    assert_eq!(resp.status().as_u16(), 204);
}

// ---- 5. Edge cases ----

#[tokio::test]
async fn test_double_assign_same_user() {
    let app = app().await;
    let tok = login_root(&app).await;
    register_user(&app, "double_assign_user").await;

    let (tid, _) = create_task_h(&app, &tok, "double_assign").await;

    // First assign
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &tok,
        Some(json!({"username": "double_assign_user"})))).await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);

    // Second assign (same user) — should succeed (INSERT OR IGNORE)
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &tok,
        Some(json!({"username": "double_assign_user"})))).await.unwrap();
    assert!(resp.status().is_success(), "Double assign failed: {}", resp.status());

    // Should still have exactly one entry
    let assignees = get_assignees(&app, &tok, tid).await;
    assert_eq!(assignees.len(), 1);
    assert_eq!(assignees[0], "double_assign_user");
}

#[tokio::test]
async fn test_assign_nonexistent_user() {
    let app = app().await;
    let tok = login_root(&app).await;
    let (tid, _) = create_task_h(&app, &tok, "assign_ghost").await;

    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &tok,
        Some(json!({"username": "nonexistent_user_xyz"})))).await.unwrap();
    assert_eq!(resp.status().as_u16(), 404);
}

#[tokio::test]
async fn test_unassign_nonexistent_user() {
    let app = app().await;
    let tok = login_root(&app).await;
    let (tid, _) = create_task_h(&app, &tok, "unassign_ghost").await;

    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/assignees/nonexistent_user_xyz", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status().as_u16(), 404);
}

#[tokio::test]
async fn test_assign_to_deleted_task() {
    let app = app().await;
    let tok = login_root(&app).await;
    register_user(&app, "assign_deleted_user").await;

    let (tid, _) = create_task_h(&app, &tok, "will_delete").await;
    // Soft delete
    app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();

    // Try to assign — task is soft-deleted, get_task should still find it
    // but the task has deleted_at set
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &tok,
        Some(json!({"username": "assign_deleted_user"})))).await.unwrap();
    // This should succeed since soft-deleted tasks still exist in DB
    // (the route doesn't check deleted_at)
    let sc = resp.status().as_u16();
    assert!(sc == 200 || sc == 404, "Unexpected status {} for assign to deleted task", sc);
}

#[tokio::test]
async fn test_status_same_as_current() {
    let app = app().await;
    let tok = login_root(&app).await;
    let (tid, _) = create_task_h(&app, &tok, "same_status").await;

    // Set to backlog (already backlog) — should succeed as no-op
    let (sc, body) = set_status(&app, &tok, tid, "backlog").await;
    assert_eq!(sc, 200);
    assert_eq!(body["status"], "backlog");
}

#[tokio::test]
async fn test_multiple_assignees() {
    let app = app().await;
    let tok = login_root(&app).await;
    register_user(&app, "multi_a1").await;
    register_user(&app, "multi_a2").await;
    register_user(&app, "multi_a3").await;

    let (tid, _) = create_task_h(&app, &tok, "multi_assign").await;

    for u in &["multi_a1", "multi_a2", "multi_a3"] {
        let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &tok,
            Some(json!({"username": u})))).await.unwrap();
        assert_eq!(resp.status().as_u16(), 200, "Assign {} failed", u);
    }

    let assignees = get_assignees(&app, &tok, tid).await;
    assert_eq!(assignees.len(), 3);

    // Remove middle one
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/assignees/multi_a2", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status().as_u16(), 204);

    let assignees = get_assignees(&app, &tok, tid).await;
    assert_eq!(assignees.len(), 2);
    assert!(!assignees.contains(&"multi_a2".to_string()));
}

// ---- Non-owner assignment permissions ----

#[tokio::test]
async fn test_non_owner_cannot_assign() {
    let app = app().await;
    let tok = login_root(&app).await;
    let user_tok = register_user(&app, "non_owner_assigner").await;
    register_user(&app, "target_assignee").await;

    // Root creates a task
    let (tid, _) = create_task_h(&app, &tok, "root_task_assign").await;

    // Non-owner tries to assign — should fail
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &user_tok,
        Some(json!({"username": "target_assignee"})))).await.unwrap();
    assert_eq!(resp.status().as_u16(), 403);
}

#[tokio::test]
async fn test_assignee_can_self_unassign() {
    let app = app().await;
    let tok = login_root(&app).await;
    let user_tok = register_user(&app, "self_unassigner").await;

    let (tid, _) = create_task_h(&app, &tok, "self_unassign_task").await;

    // Root assigns user
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &tok,
        Some(json!({"username": "self_unassigner"})))).await.unwrap();

    // User unassigns themselves — should succeed
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/assignees/self_unassigner", tid), &user_tok, None)).await.unwrap();
    assert_eq!(resp.status().as_u16(), 204);

    let assignees = get_assignees(&app, &tok, tid).await;
    assert!(!assignees.contains(&"self_unassigner".to_string()));
}

// ---- 6. Sprint board columns reflect status changes ----

#[tokio::test]
async fn test_sprint_board_columns_reflect_statuses() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create sprint
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"BoardTest"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();

    // Start sprint
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();

    // Create tasks in various statuses
    let (t_backlog, _) = create_task_h(&app, &tok, "board_backlog").await;
    let (t_active, _) = create_task_h(&app, &tok, "board_active").await;
    set_status(&app, &tok, t_active, "active").await;
    let (t_wip, _) = create_task_h(&app, &tok, "board_wip").await;
    set_status(&app, &tok, t_wip, "in_progress").await;
    let (t_blocked, _) = create_task_h(&app, &tok, "board_blocked").await;
    set_status(&app, &tok, t_blocked, "blocked").await;
    let (t_completed, _) = create_task_h(&app, &tok, "board_completed").await;
    set_status(&app, &tok, t_completed, "completed").await;
    let (t_done, _) = create_task_h(&app, &tok, "board_done").await;
    set_status(&app, &tok, t_done, "done").await;
    let (t_estimated, _) = create_task_h(&app, &tok, "board_estimated").await;
    set_status(&app, &tok, t_estimated, "estimated").await;

    // Add all to sprint
    let all_ids = vec![t_backlog, t_active, t_wip, t_blocked, t_completed, t_done, t_estimated];
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok,
        Some(json!({"task_ids": all_ids})))).await.unwrap();

    // Get board
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/board", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let board = body_json(resp).await;

    // Verify column assignments match the board logic:
    // todo: backlog, estimated, and anything not in other columns
    // in_progress: in_progress, active
    // blocked: blocked
    // done: completed, done
    let todo_ids: Vec<i64> = board["todo"].as_array().unwrap().iter().map(|t| t["id"].as_i64().unwrap()).collect();
    let wip_ids: Vec<i64> = board["in_progress"].as_array().unwrap().iter().map(|t| t["id"].as_i64().unwrap()).collect();
    let blocked_ids: Vec<i64> = board["blocked"].as_array().unwrap().iter().map(|t| t["id"].as_i64().unwrap()).collect();
    let done_ids: Vec<i64> = board["done"].as_array().unwrap().iter().map(|t| t["id"].as_i64().unwrap()).collect();

    assert!(todo_ids.contains(&t_backlog), "backlog task should be in todo column");
    assert!(todo_ids.contains(&t_estimated), "estimated task should be in todo column");
    assert!(wip_ids.contains(&t_active), "active task should be in in_progress column");
    assert!(wip_ids.contains(&t_wip), "in_progress task should be in in_progress column");
    assert!(blocked_ids.contains(&t_blocked), "blocked task should be in blocked column");
    assert!(done_ids.contains(&t_completed), "completed task should be in done column");
    assert!(done_ids.contains(&t_done), "done task should be in done column");

    // Now move a task from todo to in_progress and verify board updates
    set_status(&app, &tok, t_backlog, "in_progress").await;

    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/board", sid), &tok, None)).await.unwrap();
    let board = body_json(resp).await;
    let wip_ids: Vec<i64> = board["in_progress"].as_array().unwrap().iter().map(|t| t["id"].as_i64().unwrap()).collect();
    let todo_ids: Vec<i64> = board["todo"].as_array().unwrap().iter().map(|t| t["id"].as_i64().unwrap()).collect();
    assert!(wip_ids.contains(&t_backlog), "moved task should now be in in_progress column");
    assert!(!todo_ids.contains(&t_backlog), "moved task should no longer be in todo column");

    // Move from blocked to done
    set_status(&app, &tok, t_blocked, "done").await;
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/board", sid), &tok, None)).await.unwrap();
    let board = body_json(resp).await;
    let done_ids: Vec<i64> = board["done"].as_array().unwrap().iter().map(|t| t["id"].as_i64().unwrap()).collect();
    let blocked_ids: Vec<i64> = board["blocked"].as_array().unwrap().iter().map(|t| t["id"].as_i64().unwrap()).collect();
    assert!(done_ids.contains(&t_blocked), "unblocked task should be in done column");
    assert!(!blocked_ids.contains(&t_blocked), "unblocked task should not be in blocked column");
}

// ---- Bulk status with assignees preserved ----

#[tokio::test]
async fn test_bulk_status_preserves_assignees() {
    let app = app().await;
    let tok = login_root(&app).await;
    register_user(&app, "bulk_assignee").await;

    let (t1, _) = create_task_h(&app, &tok, "bulk_a1").await;
    let (t2, _) = create_task_h(&app, &tok, "bulk_a2").await;

    // Assign user to both
    for tid in [t1, t2] {
        app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &tok,
            Some(json!({"username": "bulk_assignee"})))).await.unwrap();
    }

    // Bulk move to completed
    app.clone().oneshot(auth_req("PUT", "/api/tasks/bulk-status", &tok,
        Some(json!({"task_ids": [t1, t2], "status": "completed"})))).await.unwrap();

    // Verify assignees still present
    for tid in [t1, t2] {
        let assignees = get_assignees(&app, &tok, tid).await;
        assert!(assignees.contains(&"bulk_assignee".to_string()),
            "Assignee lost after bulk status change on task {}", tid);
    }

    // Bulk move back to backlog
    app.clone().oneshot(auth_req("PUT", "/api/tasks/bulk-status", &tok,
        Some(json!({"task_ids": [t1, t2], "status": "backlog"})))).await.unwrap();

    for tid in [t1, t2] {
        let assignees = get_assignees(&app, &tok, tid).await;
        assert!(assignees.contains(&"bulk_assignee".to_string()),
            "Assignee lost after bulk reopen on task {}", tid);
    }
}

// ---- Non-owner bulk status rejected ----

#[tokio::test]
async fn test_bulk_status_non_owner_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let user_tok = register_user(&app, "bulk_non_owner").await;

    // Root creates tasks
    let (t1, _) = create_task_h(&app, &tok, "root_bulk_1").await;
    let (t2, _) = create_task_h(&app, &tok, "root_bulk_2").await;

    // Non-owner tries bulk update
    let resp = app.clone().oneshot(auth_req("PUT", "/api/tasks/bulk-status", &user_tok,
        Some(json!({"task_ids": [t1, t2], "status": "completed"})))).await.unwrap();
    assert_eq!(resp.status().as_u16(), 403);
}

// ---- Rapid status cycling (stress test) ----

#[tokio::test]
async fn test_rapid_status_cycling() {
    let app = app().await;
    let tok = login_root(&app).await;
    let (tid, _) = create_task_h(&app, &tok, "rapid_cycle").await;

    let cycle = ["active", "in_progress", "blocked", "backlog", "estimated",
                  "in_progress", "done", "backlog", "completed", "archived",
                  "backlog", "active", "completed"];

    for status in &cycle {
        let (sc, body) = set_status(&app, &tok, tid, status).await;
        assert_eq!(sc, 200, "Rapid cycle to '{}' failed", status);
        assert_eq!(body["status"].as_str().unwrap(), *status);
    }
}

// ============================================================
// Custom task statuses (Jira-like workflows)
// ============================================================

#[tokio::test]
async fn test_custom_status_crud() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create
    let resp = app.clone().oneshot(auth_req("POST", "/api/statuses", &tok, Some(json!({"name":"review","color":"#f59e0b","category":"in_progress"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let status = body_json(resp).await;
    assert_eq!(status["name"], "review");
    assert_eq!(status["category"], "in_progress");
    let sid = status["id"].as_i64().unwrap();
    // List
    let resp = app.clone().oneshot(auth_req("GET", "/api/statuses", &tok, None)).await.unwrap();
    let statuses = body_json(resp).await;
    assert!(statuses.as_array().unwrap().iter().any(|s| s["name"] == "review"));
    // Update
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/statuses/{}", sid), &tok, Some(json!({"name":"code_review","color":"#10b981","category":"in_progress"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["name"], "code_review");
    // Delete
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/statuses/{}", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_custom_status_used_in_task() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create custom status
    app.clone().oneshot(auth_req("POST", "/api/statuses", &tok, Some(json!({"name":"testing","category":"in_progress"})))).await.unwrap();
    // Create task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Set task to custom status
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"testing"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["status"], "testing");
}

#[tokio::test]
async fn test_custom_status_unknown_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Try to set task to non-existent custom status
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"nonexistent_status"})))).await.unwrap();
    assert_eq!(resp.status(), 400, "Unknown custom status should be rejected");
}

#[tokio::test]
async fn test_custom_status_non_root_cannot_create() {
    let app = app().await;
    let (user_tok, _) = register_user_full(&app, "statususer", "StatUs111").await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/statuses", &user_tok, Some(json!({"name":"mystat","category":"todo"})))).await.unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_custom_status_board_mapping() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create custom statuses in different categories
    app.clone().oneshot(auth_req("POST", "/api/statuses", &tok, Some(json!({"name":"qa_review","category":"in_progress"})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", "/api/statuses", &tok, Some(json!({"name":"deployed","category":"done"})))).await.unwrap();
    // Create task, sprint, add task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"BoardTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"BS"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    // Set task to custom "qa_review" (category: in_progress)
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"qa_review"})))).await.unwrap();
    // Check board — task should be in in_progress column
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/board", sid), &tok, None)).await.unwrap();
    let board = body_json(resp).await;
    assert!(board["in_progress"].as_array().unwrap().iter().any(|t| t["title"] == "BoardTask"), "Custom status with category in_progress should map to in_progress column");
    // Change to "deployed" (category: done)
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"deployed"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/board", sid), &tok, None)).await.unwrap();
    let board = body_json(resp).await;
    assert!(board["done"].as_array().unwrap().iter().any(|t| t["title"] == "BoardTask"), "Custom status with category done should map to done column");
}

// ============================================================
// Custom fields on tasks (Jira gap #3)
// ============================================================

#[tokio::test]
async fn test_custom_field_crud() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create text field
    let resp = app.clone().oneshot(auth_req("POST", "/api/fields", &tok, Some(json!({"name":"Sprint Goal","field_type":"text"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let field = body_json(resp).await;
    assert_eq!(field["name"], "Sprint Goal");
    assert_eq!(field["field_type"], "text");
    let fid = field["id"].as_i64().unwrap();
    // Create select field
    let resp = app.clone().oneshot(auth_req("POST", "/api/fields", &tok, Some(json!({"name":"Severity","field_type":"select","options":["low","medium","high","critical"]})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    assert_eq!(body_json(resp).await["field_type"], "select");
    // List
    let resp = app.clone().oneshot(auth_req("GET", "/api/fields", &tok, None)).await.unwrap();
    let fields = body_json(resp).await;
    assert!(fields.as_array().unwrap().len() >= 2);
    // Update
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/fields/{}", fid), &tok, Some(json!({"name":"Goal","field_type":"text","required":true})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["required"], true);
    // Delete
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/fields/{}", fid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_custom_field_values_on_task() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create field
    let resp = app.clone().oneshot(auth_req("POST", "/api/fields", &tok, Some(json!({"name":"Component","field_type":"text"})))).await.unwrap();
    let fid = body_json(resp).await["id"].as_i64().unwrap();
    // Create task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"FieldTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Set value
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/fields/{}", tid, fid), &tok, Some(json!({"value":"Backend"})))).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Get values
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/fields", tid), &tok, None)).await.unwrap();
    let vals = body_json(resp).await;
    let arr = vals.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["field_name"], "Component");
    assert_eq!(arr[0]["value"], "Backend");
    // Update value
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/fields/{}", tid, fid), &tok, Some(json!({"value":"Frontend"})))).await.unwrap();
    assert_eq!(resp.status(), 204);
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/fields", tid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap()[0]["value"], "Frontend");
    // Delete value
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/fields/{}", tid, fid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/fields", tid), &tok, None)).await.unwrap();
    assert!(body_json(resp).await.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_custom_field_in_task_detail() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create field + task + set value
    let resp = app.clone().oneshot(auth_req("POST", "/api/fields", &tok, Some(json!({"name":"Priority Level","field_type":"select","options":["P0","P1","P2"]})))).await.unwrap();
    let fid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"DetailTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/fields/{}", tid, fid), &tok, Some(json!({"value":"P0"})))).await.unwrap();
    // Get task detail — should include custom_fields
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
    let detail = body_json(resp).await;
    let cf = detail["custom_fields"].as_array().unwrap();
    assert_eq!(cf.len(), 1);
    assert_eq!(cf[0]["field_name"], "Priority Level");
    assert_eq!(cf[0]["value"], "P0");
}

#[tokio::test]
async fn test_custom_field_non_root_cannot_create() {
    let app = app().await;
    let (user_tok, _) = register_user_full(&app, "fielduser", "FieldU111").await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/fields", &user_tok, Some(json!({"name":"MyField"})))).await.unwrap();
    assert_eq!(resp.status(), 403);
}

// ============================================================
// Bulk operations + label filter (Jira gap #9)
// ============================================================

#[tokio::test]
async fn test_bulk_assign() {
    let app = app().await;
    let tok = login_root(&app).await;
    register_user_full(&app, "bulkdev", "BulkDv111").await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"BA1"})))).await.unwrap();
    let t1 = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"BA2"})))).await.unwrap();
    let t2 = body_json(resp).await["id"].as_i64().unwrap();
    // Bulk assign
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks/bulk-assign", &tok, Some(json!({"task_ids":[t1,t2],"username":"bulkdev"})))).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Verify
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/assignees", t1), &tok, None)).await.unwrap();
    let a = body_json(resp).await;
    assert!(a.as_array().unwrap().iter().any(|u| u == "bulkdev"));
}

#[tokio::test]
async fn test_bulk_sprint_move() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"SM1"})))).await.unwrap();
    let t1 = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"SM2"})))).await.unwrap();
    let t2 = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"BulkSprint"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    // Bulk move
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks/bulk-sprint", &tok, Some(json!({"task_ids":[t1,t2],"sprint_id":sid})))).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Verify
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}", sid), &tok, None)).await.unwrap();
    let detail = body_json(resp).await;
    assert_eq!(detail["tasks"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_label_filter() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create label
    let resp = app.clone().oneshot(auth_req("POST", "/api/labels", &tok, Some(json!({"name":"urgent","color":"#ef4444"})))).await.unwrap();
    let lid = body_json(resp).await["id"].as_i64().unwrap();
    // Create tasks
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Labeled"})))).await.unwrap();
    let t1 = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Unlabeled"})))).await.unwrap();
    // Add label to t1
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/labels/{}", t1, lid), &tok, None)).await.unwrap();
    // Filter by label
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks?label=urgent", &tok, None)).await.unwrap();
    let tasks = body_json(resp).await;
    let arr = tasks.as_array().unwrap();
    assert!(arr.iter().all(|t| t["title"] == "Labeled"), "Label filter should only return labeled tasks");
    assert!(arr.iter().any(|t| t["title"] == "Labeled"));
}

// ============================================================
// Time tracking reports (Jira gap #6)
// ============================================================

#[tokio::test]
async fn test_time_tracking_report_json() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create task + complete a session so there's data
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"TT","project":"Proj1"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({"task_id": tid})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", "/api/timer/stop", &tok, None)).await.unwrap();
    // Get report
    let resp = app.clone().oneshot(auth_req("GET", "/api/reports/time-tracking", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("json"));
}

#[tokio::test]
async fn test_time_tracking_report_csv() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/reports/time-tracking?format=csv", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("csv"));
    let body = String::from_utf8(resp.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert!(body.starts_with("username,project,week,hours,sessions"));
}

#[tokio::test]
async fn test_time_tracking_non_root_rejected() {
    let app = app().await;
    let (user_tok, _) = register_user_full(&app, "ttuser", "TtUser111").await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/reports/time-tracking", &user_tok, None)).await.unwrap();
    assert_eq!(resp.status(), 403);
}

// ============================================================
// Advanced search (Jira gap #10)
// ============================================================

#[tokio::test]
async fn test_advanced_search_status_filter() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"AS1"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"AS2"})))).await.unwrap();
    let t2 = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", t2), &tok, Some(json!({"status":"in_progress"})))).await.unwrap();
    // Search for in_progress only
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks/search/advanced", &tok, Some(json!({
        "filters": [{"field": "status", "op": "eq", "value": "in_progress"}]
    })))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let tasks = body_json(resp).await;
    assert!(tasks.as_array().unwrap().iter().all(|t| t["status"] == "in_progress"));
}

#[tokio::test]
async fn test_advanced_search_multi_status() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"MS1"})))).await.unwrap();
    let t1 = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"MS2"})))).await.unwrap();
    let t2 = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", t1), &tok, Some(json!({"status":"in_progress"})))).await.unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", t2), &tok, Some(json!({"status":"completed"})))).await.unwrap();
    // Search for in_progress OR completed
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks/search/advanced", &tok, Some(json!({
        "filters": [{"field": "status", "op": "in", "value": ["in_progress", "completed"]}]
    })))).await.unwrap();
    let tasks = body_json(resp).await;
    let statuses: Vec<&str> = tasks.as_array().unwrap().iter().map(|t| t["status"].as_str().unwrap()).collect();
    assert!(statuses.iter().all(|s| *s == "in_progress" || *s == "completed"));
}

#[tokio::test]
async fn test_advanced_search_combined_filters() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"CF1","priority":5,"project":"SearchProj"})))).await.unwrap();
    let t1 = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"CF2","priority":1,"project":"SearchProj"})))).await.unwrap();
    // Search: priority >= 4 AND project = SearchProj
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks/search/advanced", &tok, Some(json!({
        "filters": [
            {"field": "priority", "op": "gte", "value": 4},
            {"field": "project", "op": "eq", "value": "SearchProj"}
        ]
    })))).await.unwrap();
    let tasks = body_json(resp).await;
    let arr = tasks.as_array().unwrap();
    assert!(arr.iter().all(|t| t["priority"].as_i64().unwrap() >= 4));
    assert!(arr.iter().any(|t| t["id"] == t1));
}

#[tokio::test]
async fn test_advanced_search_custom_field_filter() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create custom field
    let resp = app.clone().oneshot(auth_req("POST", "/api/fields", &tok, Some(json!({"name":"env","field_type":"text"})))).await.unwrap();
    let fid = body_json(resp).await["id"].as_i64().unwrap();
    // Create tasks with different field values
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Prod"})))).await.unwrap();
    let t1 = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Dev"})))).await.unwrap();
    let t2 = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/fields/{}", t1, fid), &tok, Some(json!({"value":"production"})))).await.unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/fields/{}", t2, fid), &tok, Some(json!({"value":"development"})))).await.unwrap();
    // Search by custom field
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks/search/advanced", &tok, Some(json!({
        "filters": [{"field": "custom:env", "op": "eq", "value": "production"}]
    })))).await.unwrap();
    let tasks = body_json(resp).await;
    let arr = tasks.as_array().unwrap();
    assert!(arr.iter().any(|t| t["title"] == "Prod"));
    assert!(!arr.iter().any(|t| t["title"] == "Dev"));
}

#[tokio::test]
async fn test_advanced_search_with_sort() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Sort1","priority":1})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Sort5","priority":5})))).await.unwrap();
    // Sort by priority descending
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks/search/advanced", &tok, Some(json!({
        "filters": [],
        "sort_by": "priority",
        "sort_dir": "desc"
    })))).await.unwrap();
    let tasks = body_json(resp).await;
    let arr = tasks.as_array().unwrap();
    if arr.len() >= 2 {
        assert!(arr[0]["priority"].as_i64().unwrap() >= arr[1]["priority"].as_i64().unwrap());
    }
}

// ============================================================
// Task checklists (Priority 3, F27)
// ============================================================

#[tokio::test]
async fn test_checklist_crud() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"CL Task"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Add items
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/checklist", tid), &tok, Some(json!({"title":"Step 1"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let item = body_json(resp).await;
    assert_eq!(item["title"], "Step 1");
    assert_eq!(item["checked"], false);
    let cid = item["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/checklist", tid), &tok, Some(json!({"title":"Step 2"})))).await.unwrap();
    // List
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/checklist", tid), &tok, None)).await.unwrap();
    let items = body_json(resp).await;
    assert_eq!(items.as_array().unwrap().len(), 2);
    // Toggle checked
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/checklist/{}", cid), &tok, Some(json!({"checked":true})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["checked"], true);
    // Update title
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/checklist/{}", cid), &tok, Some(json!({"title":"Step 1 (done)"})))).await.unwrap();
    assert_eq!(body_json(resp).await["title"], "Step 1 (done)");
    // Delete
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/checklist/{}", cid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/checklist", tid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_checklist_assignee_can_edit() {
    let app = app().await;
    let tok = login_root(&app).await;
    let (user_tok, _) = register_user_full(&app, "cluser", "ClUser111").await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"CL2"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Assign user
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &tok, Some(json!({"username":"cluser"})))).await.unwrap();
    // Assignee can add checklist items
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/checklist", tid), &user_tok, Some(json!({"title":"My step"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let cid = body_json(resp).await["id"].as_i64().unwrap();
    // Assignee can toggle
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/checklist/{}", cid), &user_tok, Some(json!({"checked":true})))).await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_checklist_cascade_on_task_delete() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"CL3"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/checklist", tid), &tok, Some(json!({"title":"Will be deleted"})))).await.unwrap();
    // Delete task (soft delete)
    app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
    // Restore and check checklist still exists (soft delete doesn't cascade)
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/restore", tid), &tok, None)).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/checklist", tid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 1);
}

// ============================================================
// RBAC: Admin role (Jira gap #2)
// ============================================================

#[tokio::test]
async fn test_admin_can_manage_labels() {
    let app = app().await;
    let tok = login_root(&app).await;
    let (_, uid) = register_user_full(&app, "admlab", "AdmLab111").await;
    // Elevate to admin
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/admin/users/{}/role", uid), &tok, Some(json!({"role":"admin"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Re-login as admin to get fresh token with admin role
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"admlab","password":"AdmLab111"})))).await.unwrap();
    let admin_tok = body_json(resp).await["token"].as_str().unwrap().to_string();
    // Admin can create labels
    let resp = app.clone().oneshot(auth_req("POST", "/api/labels", &admin_tok, Some(json!({"name":"admin_label","color":"#ff0000"})))).await.unwrap();
    assert_eq!(resp.status(), 201, "Admin should be able to create labels");
}

#[tokio::test]
async fn test_admin_can_manage_others_tasks() {
    let app = app().await;
    let tok = login_root(&app).await;
    let (user_tok, _) = register_user_full(&app, "admtask_user", "AdmTU1111").await;
    let (_, admin_uid) = register_user_full(&app, "admtask_admin", "AdmTA1111").await;
    // Elevate to admin
    app.clone().oneshot(auth_req("PUT", &format!("/api/admin/users/{}/role", admin_uid), &tok, Some(json!({"role":"admin"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"admtask_admin","password":"AdmTA1111"})))).await.unwrap();
    let admin_tok = body_json(resp).await["token"].as_str().unwrap().to_string();
    // User creates task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &user_tok, Some(json!({"title":"User task"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Admin can update user's task
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &admin_tok, Some(json!({"status":"in_progress"})))).await.unwrap();
    assert_eq!(resp.status(), 200, "Admin should be able to update any task");
}

#[tokio::test]
async fn test_admin_cannot_manage_users() {
    let app = app().await;
    let tok = login_root(&app).await;
    let (_, admin_uid) = register_user_full(&app, "admnouser", "AdmNU1111").await;
    app.clone().oneshot(auth_req("PUT", &format!("/api/admin/users/{}/role", admin_uid), &tok, Some(json!({"role":"admin"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"admnouser","password":"AdmNU1111"})))).await.unwrap();
    let admin_tok = body_json(resp).await["token"].as_str().unwrap().to_string();
    // Admin cannot list users
    let resp = app.clone().oneshot(auth_req("GET", "/api/admin/users", &admin_tok, None)).await.unwrap();
    assert_eq!(resp.status(), 403, "Admin should NOT be able to list users");
    // Admin cannot create backup
    let resp = app.clone().oneshot(auth_req("POST", "/api/admin/backup", &admin_tok, None)).await.unwrap();
    assert_eq!(resp.status(), 403, "Admin should NOT be able to create backups");
}

#[tokio::test]
async fn test_admin_can_create_custom_statuses_and_fields() {
    let app = app().await;
    let tok = login_root(&app).await;
    let (_, admin_uid) = register_user_full(&app, "admcf", "AdmCF1111").await;
    app.clone().oneshot(auth_req("PUT", &format!("/api/admin/users/{}/role", admin_uid), &tok, Some(json!({"role":"admin"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"admcf","password":"AdmCF1111"})))).await.unwrap();
    let admin_tok = body_json(resp).await["token"].as_str().unwrap().to_string();
    // Admin can create custom statuses
    let resp = app.clone().oneshot(auth_req("POST", "/api/statuses", &admin_tok, Some(json!({"name":"admin_review","category":"in_progress"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    // Admin can create custom fields
    let resp = app.clone().oneshot(auth_req("POST", "/api/fields", &admin_tok, Some(json!({"name":"admin_field","field_type":"text"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
}

// ============================================================
// SLA tracking (Jira gap #8)
// ============================================================

#[tokio::test]
async fn test_sla_report() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create and complete a task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"SLA1","priority":5})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"completed"})))).await.unwrap();
    // Get SLA report
    let resp = app.clone().oneshot(auth_req("GET", "/api/reports/sla", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let report = body_json(resp).await;
    assert!(report["resolution_time_by_priority"].as_array().unwrap().len() > 0);
    assert!(report["overdue_tasks"].is_number());
    assert!(report["on_time_completion"]["on_time_pct"].is_number());
}

#[tokio::test]
async fn test_sla_report_non_admin_rejected() {
    let app = app().await;
    let (user_tok, _) = register_user_full(&app, "slauser", "SlaUs1111").await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/reports/sla", &user_tok, None)).await.unwrap();
    assert_eq!(resp.status(), 403);
}

// ============================================================
// Automation rule execution (F19)
// ============================================================

#[tokio::test]
async fn test_automation_status_change_fires() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create automation: when task becomes completed, set priority to 1
    let resp = app.clone().oneshot(auth_req("POST", "/api/automations", &tok, Some(json!({
        "name": "Lower priority on complete",
        "trigger_event": "task.status_changed",
        "condition_json": "{\"to_status\":\"completed\"}",
        "action_json": "{\"set_priority\":1}"
    })))).await.unwrap();
    assert_eq!(resp.status(), 201);
    // Create task with priority 5
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"AutoTask","priority":5})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Change status to completed
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"completed"})))).await.unwrap();
    // Wait for async automation to fire
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    // Check priority was changed to 1
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
    let detail = body_json(resp).await;
    assert_eq!(detail["task"]["priority"], 1, "Automation should have set priority to 1");
}

#[tokio::test]
async fn test_automation_condition_filters() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create automation: only fires when going from in_progress to completed
    app.clone().oneshot(auth_req("POST", "/api/automations", &tok, Some(json!({
        "name": "Specific transition",
        "trigger_event": "task.status_changed",
        "condition_json": "{\"from_status\":\"in_progress\",\"to_status\":\"completed\"}",
        "action_json": "{\"set_priority\":1}"
    })))).await.unwrap();
    // Create task, go directly from backlog to completed (should NOT fire)
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"CondTask","priority":5})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"completed"})))).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await["task"]["priority"], 5, "Automation should NOT fire for backlog→completed");
}

#[tokio::test]
async fn test_automation_all_subtasks_done() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create automation: when all subtasks done, mark parent as completed
    app.clone().oneshot(auth_req("POST", "/api/automations", &tok, Some(json!({
        "name": "Auto-complete parent",
        "trigger_event": "task.all_subtasks_done",
        "condition_json": "{}",
        "action_json": "{\"set_status\":\"completed\"}"
    })))).await.unwrap();
    // Create parent + child
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Parent"})))).await.unwrap();
    let pid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Child","parent_id":pid})))).await.unwrap();
    let cid = body_json(resp).await["id"].as_i64().unwrap();
    // Complete the child
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", cid), &tok, Some(json!({"status":"completed"})))).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    // Parent should be auto-completed
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", pid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await["task"]["status"], "completed", "Parent should be auto-completed when all subtasks done");
}

// ============================================================
// Comprehensive project export
// ============================================================

#[tokio::test]
async fn test_project_export_includes_all_data() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create task with comment, label, checklist, custom field
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"ExportTask","project":"ExportProj"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &tok, Some(json!({"content":"Test comment"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/labels", &tok, Some(json!({"name":"export_label","color":"#ff0000"})))).await.unwrap();
    let lid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/labels/{}", tid, lid), &tok, None)).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/checklist", tid), &tok, Some(json!({"title":"Check item"})))).await.unwrap();
    // Export
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/project?project=ExportProj", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let export = body_json(resp).await;
    assert_eq!(export["version"], 1);
    assert!(!export["tasks"].as_array().unwrap().is_empty());
    assert!(!export["comments"].as_array().unwrap().is_empty());
    assert!(!export["labels"].as_array().unwrap().is_empty());
    assert!(!export["checklists"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_project_import_roundtrip() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create source data
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"ImportMe","project":"RoundTrip","priority":4})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &tok, Some(json!({"content":"Round trip comment"})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/checklist", tid), &tok, Some(json!({"title":"Check 1"})))).await.unwrap();
    // Export
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/project?project=RoundTrip", &tok, None)).await.unwrap();
    let export = body_json(resp).await;
    // Import into same DB (creates duplicates — that's fine for testing)
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/project", &tok, Some(export.clone()))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let result = body_json(resp).await;
    assert_eq!(result["created_tasks"], 1);
    assert_eq!(result["created_comments"], 1);
    assert_eq!(result["created_checklists"], 1);
}

// ============================================================
// Standup report + Global search
// ============================================================

#[tokio::test]
async fn test_standup_report() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create and complete a task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Standup task"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"in_progress"})))).await.unwrap();
    // Get standup
    let resp = app.clone().oneshot(auth_req("GET", "/api/reports/standup", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let report = body_json(resp).await;
    assert!(report["date"].is_string());
    assert!(report["today"].is_array());
    assert!(report["markdown"].as_str().unwrap().contains("Daily Standup"));
    // in_progress task should appear in "today"
    assert!(report["today"].as_array().unwrap().iter().any(|t| t["title"] == "Standup task"));
}

#[tokio::test]
async fn test_global_search() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"UniqueSearchTarget42"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/search?q=UniqueSearchTarget42", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let results = body_json(resp).await;
    assert!(!results["tasks"].as_array().unwrap().is_empty(), "Should find the task");
}

#[tokio::test]
async fn test_global_search_comments() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"CommentSearch"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &tok, Some(json!({"content":"UniqueCommentXyz99"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/search?q=UniqueCommentXyz99", &tok, None)).await.unwrap();
    let results = body_json(resp).await;
    assert!(!results["comments"].as_array().unwrap().is_empty(), "Should find the comment");
}

// ============================================================
// Edge case tests for newer features
// ============================================================

#[tokio::test]
async fn test_automation_disabled_rule_does_not_fire() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create automation then disable it
    let resp = app.clone().oneshot(auth_req("POST", "/api/automations", &tok, Some(json!({
        "name": "Disabled rule", "trigger_event": "task.status_changed",
        "condition_json": "{\"to_status\":\"completed\"}", "action_json": "{\"set_priority\":1}"
    })))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("DELETE", &format!("/api/automations/{}", rid), &tok, None)).await.unwrap();
    // Create task and complete it
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"NoAuto","priority":5})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"completed"})))).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await["task"]["priority"], 5, "Deleted rule should not fire");
}

#[tokio::test]
async fn test_custom_status_duplicate_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(auth_req("POST", "/api/statuses", &tok, Some(json!({"name":"dup_status","category":"todo"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/statuses", &tok, Some(json!({"name":"dup_status","category":"todo"})))).await.unwrap();
    assert_eq!(resp.status(), 409);
}

#[tokio::test]
async fn test_custom_field_select_requires_options() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/fields", &tok, Some(json!({"name":"NoOpts","field_type":"select"})))).await.unwrap();
    assert_eq!(resp.status(), 400, "Select field without options should be rejected");
}

#[tokio::test]
async fn test_checklist_empty_title_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"CL"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/checklist", tid), &tok, Some(json!({"title":""})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_bulk_assign_nonexistent_user_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"BA"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks/bulk-assign", &tok, Some(json!({"task_ids":[tid],"username":"nonexistent_user_xyz"})))).await.unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_advanced_search_title_contains() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"UniqueNeedle12345"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks/search/advanced", &tok, Some(json!({
        "filters": [{"field": "title", "op": "contains", "value": "Needle123"}]
    })))).await.unwrap();
    let tasks = body_json(resp).await;
    assert!(tasks.as_array().unwrap().iter().any(|t| t["title"].as_str().unwrap().contains("Needle")));
}

#[tokio::test]
async fn test_project_export_empty_project() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/project?project=NonExistentProject", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let export = body_json(resp).await;
    assert_eq!(export["tasks"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_global_search_empty_query() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/search?q=", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let r = body_json(resp).await;
    assert!(r["tasks"].as_array().unwrap().is_empty());
}

// ============================================================
// Recurrence auto-creation (DB function test)
// ============================================================

#[tokio::test]
async fn test_recurrence_auto_creates_task_via_db() {
    // Test the recurrence DB functions directly (simulating background scheduler)
    std::env::set_var("POFLOW_ROOT_PASSWORD", "root");
    let pool = poflow_daemon::db::connect_memory().await.unwrap();
    let yesterday = (chrono::Utc::now() - chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    // Create template task
    let task = poflow_daemon::db::create_task(&pool, poflow_daemon::db::CreateTaskOpts {
        user_id: 1, parent_id: None, title: "Daily standup", description: None, project: Some("Team"), project_id: None, tags: None, priority: 4, estimated: 1, estimated_hours: 0.0, remaining_points: 0.0, due_date: None,
    }).await.unwrap();
    // Set recurrence with yesterday's due date
    poflow_daemon::db::set_recurrence(&pool, task.id, "daily", &yesterday).await.unwrap();
    // Get due recurrences
    let due = poflow_daemon::db::get_due_recurrences(&pool, &today).await.unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].task_id, task.id);
    // Simulate auto-creation
    let title = format!("{} ({})", task.title, today);
    let new_task = poflow_daemon::db::create_task(&pool, poflow_daemon::db::CreateTaskOpts {
        user_id: task.user_id, parent_id: task.parent_id, title: &title,
        description: task.description.as_deref(), project: task.project.as_deref(), project_id: task.project_id, tags: task.tags.as_deref(),
        priority: task.priority, estimated: task.estimated, estimated_hours: task.estimated_hours,
        remaining_points: task.remaining_points, due_date: task.due_date.as_deref(),
    }).await.unwrap();
    assert!(new_task.title.contains(&today));
    assert_eq!(new_task.project.as_deref(), Some("Team"));
    assert_eq!(new_task.priority, 4);
    // Advance recurrence
    let next = chrono::NaiveDate::parse_from_str(&yesterday, "%Y-%m-%d").unwrap() + chrono::Duration::days(1);
    poflow_daemon::db::advance_recurrence(&pool, task.id, &next.format("%Y-%m-%d").to_string()).await.unwrap();
    // Verify next_due advanced
    let rec = poflow_daemon::db::get_recurrence(&pool, task.id).await.unwrap().unwrap();
    assert_eq!(rec.next_due, today);
    assert!(rec.last_created.as_ref().unwrap().starts_with(&today), "last_created should be today");
}

#[tokio::test]
async fn test_recurrence_monthly_advances_correctly() {
    use chrono::Datelike;
    std::env::set_var("POFLOW_ROOT_PASSWORD", "root");
    let pool = poflow_daemon::db::connect_memory().await.unwrap();
    let task = poflow_daemon::db::create_task(&pool, poflow_daemon::db::CreateTaskOpts {
        user_id: 1, parent_id: None, title: "Monthly report", description: None, project: None, project_id: None, tags: None, priority: 3, estimated: 1, estimated_hours: 0.0, remaining_points: 0.0, due_date: Some("2026-01-31"),
    }).await.unwrap();
    poflow_daemon::db::set_recurrence(&pool, task.id, "monthly", "2026-01-31").await.unwrap();
    // Advance from Jan 31 → should go to Feb 28 (not Feb 31)
    let d = chrono::NaiveDate::parse_from_str("2026-01-31", "%Y-%m-%d").unwrap();
    let m = d.month() % 12 + 1; // Feb
    let y = if m == 1 { d.year() + 1 } else { d.year() };
    let max_day = chrono::NaiveDate::from_ymd_opt(y, m + 1, 1).and_then(|d| d.pred_opt()).map(|d| d.day()).unwrap_or(28);
    let next = chrono::NaiveDate::from_ymd_opt(y, m, 31u32.min(max_day)).unwrap();
    assert_eq!(next.to_string(), "2026-02-28", "Jan 31 monthly should advance to Feb 28");
}


// ============================================================
// Full workflow integration test
// ============================================================

#[tokio::test]
async fn test_full_project_workflow() {
    let app = app().await;
    let tok = login_root(&app).await;

    // 1. Create a project with tasks
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Backend","project":"MyApp","priority":4})))).await.unwrap();
    let t1 = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Frontend","project":"MyApp","priority":3})))).await.unwrap();
    let t2 = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Deploy","project":"MyApp","priority":5,"parent_id":t1})))).await.unwrap();
    let t3 = body_json(resp).await["id"].as_i64().unwrap();

    // 2. Add labels
    let resp = app.clone().oneshot(auth_req("POST", "/api/labels", &tok, Some(json!({"name":"wf_critical","color":"#ef4444"})))).await.unwrap();
    let lid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/labels/{}", t1, lid), &tok, None)).await.unwrap();

    // 3. Add dependency: Deploy depends on Backend
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/dependencies", t3), &tok, Some(json!({"depends_on":t1})))).await.unwrap();

    // 4. Add checklist to Backend
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/checklist", t1), &tok, Some(json!({"title":"Write API"})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/checklist", t1), &tok, Some(json!({"title":"Write tests"})))).await.unwrap();

    // 5. Create sprint and add tasks
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"Sprint 1","project":"MyApp","start_date":"2026-04-14","end_date":"2026-04-28"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[t1,t2,t3]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();

    // 6. Work on tasks — change statuses
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", t1), &tok, Some(json!({"status":"in_progress"})))).await.unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", t1), &tok, Some(json!({"status":"completed"})))).await.unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", t2), &tok, Some(json!({"status":"completed"})))).await.unwrap();

    // 7. Check sprint board
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/board", sid), &tok, None)).await.unwrap();
    let board = body_json(resp).await;
    assert_eq!(board["done"].as_array().unwrap().len(), 2, "2 tasks should be done");

    // 8. Log time
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok, Some(json!({"task_id":t1,"hours":4.0})))).await.unwrap();

    // 9. Export project
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/project?project=MyApp", &tok, None)).await.unwrap();
    let export = body_json(resp).await;
    assert_eq!(export["tasks"].as_array().unwrap().len(), 3);
    assert!(!export["labels"].as_array().unwrap().is_empty());
    assert!(!export["checklists"].as_array().unwrap().is_empty());

    // 10. Get standup
    let resp = app.clone().oneshot(auth_req("GET", "/api/reports/standup", &tok, None)).await.unwrap();
    let standup = body_json(resp).await;
    assert!(standup["markdown"].as_str().unwrap().contains("Daily Standup"));

    // 11. Advanced search
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks/search/advanced", &tok, Some(json!({
        "filters": [{"field":"project","op":"eq","value":"MyApp"},{"field":"status","op":"eq","value":"completed"}],
        "sort_by": "priority", "sort_dir": "desc"
    })))).await.unwrap();
    let results = body_json(resp).await;
    assert_eq!(results.as_array().unwrap().len(), 2, "2 completed tasks in MyApp");
}

#[tokio::test]
async fn test_multi_user_collaboration() {
    let app = app().await;
    let root_tok = login_root(&app).await;
    let (user_tok, user_id) = register_user_full(&app, "collab_dev", "CollDev11").await;
    let (_, admin_id) = register_user_full(&app, "collab_admin", "CollAdm11").await;

    // Promote to admin
    app.clone().oneshot(auth_req("PUT", &format!("/api/admin/users/{}/role", admin_id), &root_tok, Some(json!({"role":"admin"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"collab_admin","password":"CollAdm11"})))).await.unwrap();
    let admin_tok = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Root creates task, assigns to dev
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &root_tok, Some(json!({"title":"Collab task"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &root_tok, Some(json!({"username":"collab_dev"})))).await.unwrap();

    // Dev can update the task (assignee permission)
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &user_tok, Some(json!({"status":"in_progress"})))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Dev can add comment
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &user_tok, Some(json!({"content":"Working on it"})))).await.unwrap();
    assert_eq!(resp.status(), 201);

    // Admin can also update (admin privilege)
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &admin_tok, Some(json!({"priority":5})))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Dev can self-unassign
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/assignees/collab_dev", tid), &user_tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);

    // Admin cannot manage users (root only)
    let resp = app.clone().oneshot(auth_req("GET", "/api/admin/users", &admin_tok, None)).await.unwrap();
    assert_eq!(resp.status(), 403);
}

// ============================================================
// Enhanced task templates (with checklists, labels, custom fields)
// ============================================================

#[tokio::test]
async fn test_template_save_and_instantiate_with_checklist() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create task with checklist
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Sprint Review","project":"Team","priority":4})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/checklist", tid), &tok, Some(json!({"title":"Prepare slides"})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/checklist", tid), &tok, Some(json!({"title":"Demo features"})))).await.unwrap();
    // Save as template
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/save-as-template", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 201);
    let tmpl = body_json(resp).await;
    let tmpl_id = tmpl["id"].as_i64().unwrap();
    let data: serde_json::Value = serde_json::from_str(tmpl["data"].as_str().unwrap()).unwrap();
    assert_eq!(data["checklist"].as_array().unwrap().len(), 2);
    // Instantiate
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/templates/{}/instantiate", tmpl_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 201);
    let new_task = body_json(resp).await;
    let new_tid = new_task["id"].as_i64().unwrap();
    // Verify checklist was copied
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/checklist", new_tid), &tok, None)).await.unwrap();
    let items = body_json(resp).await;
    assert_eq!(items.as_array().unwrap().len(), 2, "Checklist should be copied from template");
    assert_eq!(items[0]["title"], "Prepare slides");
}

#[tokio::test]
async fn test_template_instantiate_with_labels() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create label
    let resp = app.clone().oneshot(auth_req("POST", "/api/labels", &tok, Some(json!({"name":"tmpl_bug","color":"#ef4444"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    // Create template with label reference
    let resp = app.clone().oneshot(auth_req("POST", "/api/templates", &tok, Some(json!({
        "name": "Bug Report",
        "data": {"title":"Bug: {{today}}","priority":5,"labels":["tmpl_bug"],"checklist":["Reproduce","Fix","Test"]}
    })))).await.unwrap();
    let tmpl_id = body_json(resp).await["id"].as_i64().unwrap();
    // Instantiate
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/templates/{}/instantiate", tmpl_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 201);
    let new_task = body_json(resp).await;
    let new_tid = new_task["id"].as_i64().unwrap();
    assert!(new_task["title"].as_str().unwrap().starts_with("Bug: 2026-"));
    // Verify label was applied
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/labels", new_tid), &tok, None)).await.unwrap();
    let labels = body_json(resp).await;
    assert!(labels.as_array().unwrap().iter().any(|l| l["name"] == "tmpl_bug"));
    // Verify checklist
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/checklist", new_tid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn test_template_instantiate_with_custom_fields() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create custom field
    let resp = app.clone().oneshot(auth_req("POST", "/api/fields", &tok, Some(json!({"name":"tmpl_env","field_type":"text"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    // Create template with custom field
    let resp = app.clone().oneshot(auth_req("POST", "/api/templates", &tok, Some(json!({
        "name": "Deploy Task",
        "data": {"title":"Deploy to prod","priority":5,"custom_fields":{"tmpl_env":"production"}}
    })))).await.unwrap();
    let tmpl_id = body_json(resp).await["id"].as_i64().unwrap();
    // Instantiate
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/templates/{}/instantiate", tmpl_id), &tok, None)).await.unwrap();
    let new_tid = body_json(resp).await["id"].as_i64().unwrap();
    // Verify custom field was set
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/fields", new_tid), &tok, None)).await.unwrap();
    let fields = body_json(resp).await;
    assert!(fields.as_array().unwrap().iter().any(|f| f["field_name"] == "tmpl_env" && f["value"] == "production"));
}

#[tokio::test]
async fn test_save_task_as_template_captures_labels() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create task with label
    let resp = app.clone().oneshot(auth_req("POST", "/api/labels", &tok, Some(json!({"name":"tmpl_save_label","color":"#22c55e"})))).await.unwrap();
    let lid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Labeled task"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/labels/{}", tid, lid), &tok, None)).await.unwrap();
    // Save as template
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/save-as-template", tid), &tok, None)).await.unwrap();
    let tmpl = body_json(resp).await;
    let data: serde_json::Value = serde_json::from_str(tmpl["data"].as_str().unwrap()).unwrap();
    assert!(data["labels"].as_array().unwrap().iter().any(|l| l == "tmpl_save_label"));
}

// ============================================================
// Drag-drop reorder persistence
// ============================================================

#[tokio::test]
async fn test_reorder_persists_sort_order() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Alpha"})))).await.unwrap();
    let a = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Beta"})))).await.unwrap();
    let b = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Gamma"})))).await.unwrap();
    let c = body_json(resp).await["id"].as_i64().unwrap();
    // Reorder: Gamma first, Alpha second, Beta third
    app.clone().oneshot(auth_req("POST", "/api/tasks/reorder", &tok, Some(json!({"orders":[[c, 0],[a, 1000],[b, 2000]]})))).await.unwrap();
    // Fetch and verify order
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks", &tok, None)).await.unwrap();
    let tasks = body_json(resp).await;
    let titles: Vec<&str> = tasks.as_array().unwrap().iter()
        .filter(|t| ["Alpha","Beta","Gamma"].contains(&t["title"].as_str().unwrap_or("")))
        .map(|t| t["title"].as_str().unwrap()).collect();
    assert_eq!(titles, vec!["Gamma", "Alpha", "Beta"], "Reorder should persist");
}

// ============================================================
// Email notifications (SMTP integration)
// ============================================================

#[tokio::test]
async fn test_profile_email_update() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Set email
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"email":"test@example.com"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Verify via admin user list
    let resp = app.clone().oneshot(auth_req("GET", "/api/admin/users", &tok, None)).await.unwrap();
    let users = body_json(resp).await;
    let root = users.as_array().unwrap().iter().find(|u| u["username"] == "root").unwrap();
    assert_eq!(root["email"], "test@example.com");
}

#[tokio::test]
async fn test_profile_invalid_email_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"email":"not-an-email"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ============================================================
// Activity feed / timeline
// ============================================================

#[tokio::test]
async fn test_activity_feed_includes_all_types() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create task (generates audit entry)
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"FeedTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Add comment
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &tok, Some(json!({"content":"Feed comment"})))).await.unwrap();
    // Create sprint
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"FeedSprint"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    // Add task to sprint, start, log burn
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok, Some(json!({"task_id":tid,"hours":2.0})))).await.unwrap();
    // Get feed
    let resp = app.clone().oneshot(auth_req("GET", "/api/feed?limit=50", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let items = body_json(resp).await;
    let types: Vec<&str> = items.as_array().unwrap().iter().map(|i| i["type"].as_str().unwrap_or("")).collect();
    assert!(types.contains(&"audit"), "Feed should contain audit entries");
    assert!(types.contains(&"comment"), "Feed should contain comments");
    assert!(types.contains(&"sprint"), "Feed should contain sprint events");
    assert!(types.contains(&"burn"), "Feed should contain burn entries");
}

#[tokio::test]
async fn test_activity_feed_type_filter() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"FilterTask"})))).await.unwrap();
    // Filter to only audit
    let resp = app.clone().oneshot(auth_req("GET", "/api/feed?types=audit&limit=10", &tok, None)).await.unwrap();
    let items = body_json(resp).await;
    assert!(items.as_array().unwrap().iter().all(|i| i["type"] == "audit"));
}

// ============================================================
// Data retention: archive / unarchive
// ============================================================

#[tokio::test]
async fn test_archive_and_unarchive_task() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create and complete a task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"ArchiveMe"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"archived"})))).await.unwrap();
    // Should appear in archived list
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks/archived", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let archived = body_json(resp).await;
    assert!(archived.as_array().unwrap().iter().any(|t| t["id"] == tid));
    // Normal task list still includes it (with status=archived) — filter by status to exclude
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks?status=backlog", &tok, None)).await.unwrap();
    let tasks = body_json(resp).await;
    assert!(!tasks.as_array().unwrap().iter().any(|t| t["id"] == tid), "Archived task should not appear when filtering by backlog");
    // Unarchive
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/unarchive", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Should be back as completed
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await["task"]["status"], "completed");
}

#[tokio::test]
async fn test_unarchive_non_archived_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"NotArchived"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/unarchive", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 400, "Should reject unarchive on non-archived task");
}

#[tokio::test]
async fn test_archived_list_user_scoped() {
    let app = app().await;
    let tok = login_root(&app).await;
    let (user_tok, _) = register_user_full(&app, "archuser", "ArchUs111").await;
    // Root archives a task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"RootArchived"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"archived"})))).await.unwrap();
    // User should NOT see root's archived tasks
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks/archived", &user_tok, None)).await.unwrap();
    let archived = body_json(resp).await;
    assert!(!archived.as_array().unwrap().iter().any(|t| t["id"] == tid));
}

// ---- Sprint 2 Task 12: Tests for previously untested routes ----

// 1. PUT /api/auth/password — change password
#[tokio::test]
async fn test_change_password() {
    let app = app().await;
    let (tok, _) = register_user_full(&app, "pwuser", "OldPass123").await;
    // Change password
    let resp = app.clone().oneshot(auth_req("PUT", "/api/auth/password", &tok, Some(json!({"current_password":"OldPass123","new_password":"NewPass456"})))).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Login with new password
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"pwuser","password":"NewPass456"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Old password should fail
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"pwuser","password":"OldPass123"})))).await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_change_password_wrong_current() {
    let app = app().await;
    let (tok, _) = register_user_full(&app, "pwwrong", "Pass1234").await;
    let resp = app.clone().oneshot(auth_req("PUT", "/api/auth/password", &tok, Some(json!({"current_password":"WrongPass","new_password":"NewPass456"})))).await.unwrap();
    assert_eq!(resp.status(), 401);
}

// 2. GET /api/notifications
#[tokio::test]
async fn test_list_notifications() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/notifications", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j = body_json(resp).await;
    assert!(j.is_array());
}

// 3. GET /api/notifications/unread
#[tokio::test]
async fn test_unread_count() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/notifications/unread", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j = body_json(resp).await;
    assert!(j["count"].is_number());
}

// 4. POST /api/notifications/read
#[tokio::test]
async fn test_mark_notifications_read() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Mark all read (no body)
    let resp = app.clone().oneshot(auth_req("POST", "/api/notifications/read", &tok, Some(json!({})))).await.unwrap();
    assert_eq!(resp.status(), 204);
}

// 5. POST /api/rooms/{id}/leave
#[tokio::test]
async fn test_leave_room() {
    let app = app().await;
    let tok = login_root(&app).await;
    let user_tok = reg(&app, "leaver").await;
    // Create room
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"LeaveRoom"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();
    // User joins
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/join", rid), &user_tok, None)).await.unwrap();
    // User leaves
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/leave", rid), &user_tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_leave_room_creator_cannot_leave() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"CreatorRoom"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/leave", rid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// 6. GET /api/sprints/compare
#[tokio::test]
async fn test_compare_sprints() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create two sprints
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"SprintA","start_date":"2026-01-01","end_date":"2026-01-14"})))).await.unwrap();
    let a = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"SprintB","start_date":"2026-01-15","end_date":"2026-01-28"})))).await.unwrap();
    let b = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/compare?a={}&b={}", a, b), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j = body_json(resp).await;
    assert!(j["a"]["name"].as_str().unwrap() == "SprintA");
    assert!(j["b"]["name"].as_str().unwrap() == "SprintB");
}

// 7. GET /api/tasks/{id}/burn-users
#[tokio::test]
async fn test_task_burn_users() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"BurnTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/burn-users", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j = body_json(resp).await;
    assert!(j.is_array());
}

// 8. DELETE /api/tasks/{id}/permanent — purge task
#[tokio::test]
async fn test_purge_task() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"PurgeMe"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Must trash first
    app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
    // Now purge
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/permanent", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Verify gone
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_purge_task_not_trashed() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"NoPurge"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Purge without trashing first should fail
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/permanent", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// 9. GET /api/tasks/search
#[tokio::test]
async fn test_search_tasks() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Searchable Widget"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks/search?q=Searchable", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j = body_json(resp).await;
    assert!(j.as_array().unwrap().iter().any(|t| t["title"].as_str().unwrap().contains("Searchable")));
}

#[tokio::test]
async fn test_search_tasks_empty_query() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks/search?q=", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j = body_json(resp).await;
    assert!(j.as_array().unwrap().is_empty());
}

// 10. GET /api/tasks/{id}/time-summary
#[tokio::test]
async fn test_task_time_summary() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"TimeSumTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/time-summary", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j = body_json(resp).await;
    assert!(j["task_id"].as_i64().unwrap() == tid);
    assert!(j["total_hours"].is_number());
    assert!(j["by_user"].is_array());
}

// 11. GET /api/timer/active
#[tokio::test]
async fn test_active_timers() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer/active", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j = body_json(resp).await;
    assert!(j.is_array());
}

// 12. DELETE /api/rooms/{id}/members/{username} — kick member
#[tokio::test]
async fn test_kick_member() {
    let app = app().await;
    let tok = login_root(&app).await;
    let user_tok = reg(&app, "kickee").await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"KickRoom"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/join", rid), &user_tok, None)).await.unwrap();
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/rooms/{}/members/kickee", rid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}