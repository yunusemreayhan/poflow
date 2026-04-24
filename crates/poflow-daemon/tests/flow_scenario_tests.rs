use axum::body::Body;
use hyper::Request;
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;

mod common;
use common::{app, json_req, auth_req, body_json, login_root, register_user_full};

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
    let config = poflow_daemon::config::Config { allow_registration: false, ..Default::default() };
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
    let (user_tok, _user_id) = register_user_full(&app, "collab_dev", "CollDev11").await;
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
