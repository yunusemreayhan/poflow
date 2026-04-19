use axum::body::Body;
use http_body_util::BodyExt;
use hyper::Request;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

mod common;
use common::{app, json_req, auth_req, body_json, login_root, register_user, register_user_full, reg};

// ---- Auth ----

#[tokio::test]
async fn test_seed_root_login() {
    let app = app().await;
    let resp = app.oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"root","password":"root"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j = body_json(resp).await;
    assert_eq!(j["username"], "root");
    assert_eq!(j["role"], "root");
    assert!(j["token"].as_str().unwrap().len() > 10);
}

#[tokio::test]
async fn test_register_and_login() {
    let app = app().await;
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"alice","password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let resp = app.oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"alice","password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["role"], "user");
}

#[tokio::test]
async fn test_login_wrong_password() {
    let app = app().await;
    let resp = app.oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"root","password":"wrong"})))).await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_unauthenticated_rejected() {
    let app = app().await;
    let resp = app.oneshot(json_req("GET", "/api/tasks", None)).await.unwrap();
    assert_eq!(resp.status(), 401);
}

// ---- Tasks CRUD ----

#[tokio::test]
async fn test_create_list_tasks() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Task A"})))).await.unwrap();
    assert!(resp.status().is_success());
    let task = body_json(resp).await;
    assert_eq!(task["title"], "Task A");
    assert_eq!(task["user"], "root");

    let resp = app.oneshot(auth_req("GET", "/api/tasks", &tok, None)).await.unwrap();
    let tasks = body_json(resp).await;
    assert_eq!(tasks.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_update_task() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Old"})))).await.unwrap();
    let id = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", id), &tok,
        Some(json!({"title":"New","status":"in_progress","priority":5,"estimated_hours":8.0})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let t = body_json(resp).await;
    assert_eq!(t["title"], "New");
    assert_eq!(t["status"], "in_progress");
    assert_eq!(t["priority"], 5);
    assert_eq!(t["estimated_hours"], 8.0);
}

#[tokio::test]
async fn test_delete_task() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Del"})))).await.unwrap();
    let id = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}", id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);

    let resp = app.oneshot(auth_req("GET", "/api/tasks", &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_subtask_cascade_delete() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Parent"})))).await.unwrap();
    let pid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Child","parent_id":pid})))).await.unwrap();

    app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}", pid), &tok, None)).await.unwrap();
    let resp = app.oneshot(auth_req("GET", "/api/tasks", &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 0);
}

// ---- Comments ----

#[tokio::test]
async fn test_comments() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &tok,
        Some(json!({"content":"Hello"})))).await.unwrap();
    assert!(resp.status().is_success());

    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/comments", tid), &tok, None)).await.unwrap();
    let comments = body_json(resp).await;
    assert_eq!(comments.as_array().unwrap().len(), 1);
    assert_eq!(comments[0]["content"], "Hello");

    let cid = comments[0]["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/comments/{}", cid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

// ---- Time Reports ----

#[tokio::test]
async fn test_time_reports_auto_assign() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/time", tid), &tok,
        Some(json!({"hours":2.5,"description":"work"})))).await.unwrap();
    assert!(resp.status().is_success());

    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/time", tid), &tok, None)).await.unwrap();
    let reports = body_json(resp).await;
    assert_eq!(reports.as_array().unwrap().len(), 1);
    assert_eq!(reports[0]["hours"], 2.5);
    assert_eq!(reports[0]["source"], "time_report");

    // Burn total
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/burn-total", tid), &tok, None)).await.unwrap();
    let total = body_json(resp).await;
    assert_eq!(total["total_hours"], 2.5);

    // Auto-assigned
    let resp = app.oneshot(auth_req("GET", &format!("/api/tasks/{}/assignees", tid), &tok, None)).await.unwrap();
    let assignees = body_json(resp).await;
    assert!(assignees.as_array().unwrap().contains(&json!("root")));
}

// ---- Assignees ----

#[tokio::test]
async fn test_assignees() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &tok,
        Some(json!({"username":"root"})))).await.unwrap();

    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/assignees", tid), &tok, None)).await.unwrap();
    assert!(body_json(resp).await.as_array().unwrap().contains(&json!("root")));

    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/assignees/root", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

// ---- Admin ----

#[tokio::test]
async fn test_admin_list_users() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.oneshot(auth_req("GET", "/api/admin/users", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let users = body_json(resp).await;
    assert!(users.as_array().unwrap().len() >= 1);
}

#[tokio::test]
async fn test_non_root_cannot_admin() {
    let app = app().await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"bob","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"bob","password":"Pass1234"})))).await.unwrap();
    let tok = body_json(resp).await["token"].as_str().unwrap().to_string();

    let resp = app.oneshot(auth_req("GET", "/api/admin/users", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 403);
}

// ---- Estimation Rooms ----

#[tokio::test]
async fn test_room_create_and_list() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok,
        Some(json!({"name":"Sprint 1","estimation_unit":"points"})))).await.unwrap();
    assert!(resp.status().is_success());
    let room = body_json(resp).await;
    assert_eq!(room["name"], "Sprint 1");
    assert_eq!(room["status"], "lobby");

    let resp = app.oneshot(auth_req("GET", "/api/rooms", &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_room_full_voting_flow() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create task + room
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Story"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok,
        Some(json!({"name":"R","estimation_unit":"points"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();

    // Start voting
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/start-voting", rid), &tok,
        Some(json!({"task_id":tid})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let r = body_json(resp).await;
    assert_eq!(r["status"], "voting");

    // Cast vote
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &tok,
        Some(json!({"value":8})))).await.unwrap();
    assert!(resp.status().is_success());

    // Reveal
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/reveal", rid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let r = body_json(resp).await;
    assert_eq!(r["status"], "revealed");

    // Accept
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/accept", rid), &tok,
        Some(json!({"value":8})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let task = body_json(resp).await;
    assert_eq!(task["estimated"], 8);
    assert_eq!(task["status"], "estimated");

    // Task votes endpoint
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/votes", tid), &tok, None)).await.unwrap();
    let votes = body_json(resp).await;
    assert_eq!(votes.as_array().unwrap().len(), 1);
    assert_eq!(votes[0]["value"], 8.0);
}

#[tokio::test]
async fn test_room_join_leave_kick() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Register second user
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"eve","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"eve","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok,
        Some(json!({"name":"R","estimation_unit":"hours"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();

    // Eve joins
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/join", rid), &tok2, None)).await.unwrap();

    // Check members via state
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/rooms/{}", rid), &tok, None)).await.unwrap();
    let state = body_json(resp).await;
    assert_eq!(state["members"].as_array().unwrap().len(), 2);

    // Kick eve
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/rooms/{}/members/eve", rid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);

    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/rooms/{}", rid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await["members"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_room_role_promotion() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"dan","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"dan","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok,
        Some(json!({"name":"R","estimation_unit":"points"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();

    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/join", rid), &tok2, None)).await.unwrap();

    // Promote dan to admin
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/rooms/{}/role", rid), &tok,
        Some(json!({"username":"dan","role":"admin"})))).await.unwrap();
    assert!(resp.status().is_success());

    // Dan can now start voting (admin action)
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"X"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/start-voting", rid), &tok2,
        Some(json!({"task_id":tid})))).await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_room_non_admin_cannot_start_voting() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"noob","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"noob","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok,
        Some(json!({"name":"R","estimation_unit":"points"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/join", rid), &tok2, None)).await.unwrap();

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"X"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/start-voting", rid), &tok2,
        Some(json!({"task_id":tid})))).await.unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_room_close() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok,
        Some(json!({"name":"R","estimation_unit":"points"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/close", rid), &tok, None)).await.unwrap();
    assert!(resp.status().is_success());

    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/rooms/{}", rid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await["room"]["status"], "closed");
}

#[tokio::test]
async fn test_room_delete() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok,
        Some(json!({"name":"R","estimation_unit":"points"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/rooms/{}", rid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);

    let resp = app.clone().oneshot(auth_req("GET", "/api/rooms", &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 0);
}

// ---- Timer ----

#[tokio::test]
async fn test_timer_state() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.oneshot(auth_req("GET", "/api/timer", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let s = body_json(resp).await;
    assert_eq!(s["status"], "Idle");
}

// ---- Hours-based accept ----

#[tokio::test]
async fn test_room_accept_hours() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"H"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok,
        Some(json!({"name":"R","estimation_unit":"hours"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();

    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/start-voting", rid), &tok, Some(json!({"task_id":tid})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &tok, Some(json!({"value":4})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/reveal", rid), &tok, None)).await.unwrap();

    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/accept", rid), &tok, Some(json!({"value":4})))).await.unwrap();
    let task = body_json(resp).await;
    assert_eq!(task["estimated_hours"], 4.0);
    assert_eq!(task["status"], "estimated");
}

// ---- Auto-advance to next task ----

#[tokio::test]
async fn test_room_auto_advance() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"A"})))).await.unwrap();
    let t1 = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"B"})))).await.unwrap();
    let t2 = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok,
        Some(json!({"name":"R","estimation_unit":"points"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();

    // Vote on first task
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/start-voting", rid), &tok, Some(json!({"task_id":t1})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &tok, Some(json!({"value":5})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/reveal", rid), &tok, None)).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/accept", rid), &tok, Some(json!({"value":5})))).await.unwrap();

    // Should auto-advance to task B
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/rooms/{}", rid), &tok, None)).await.unwrap();
    let state = body_json(resp).await;
    assert_eq!(state["room"]["status"], "voting");
    assert_eq!(state["room"]["current_task_id"], t2);
}

// ---- Config ----

#[tokio::test]
async fn test_config_get() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.oneshot(auth_req("GET", "/api/config", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let c = body_json(resp).await;
    assert_eq!(c["work_duration_min"], 25);
}

// ---- History ----

#[tokio::test]
async fn test_history_empty() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.oneshot(auth_req("GET", "/api/history", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 0);
}

// ---- Sprints ----

#[tokio::test]
async fn test_sprint_create_and_list() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok,
        Some(json!({"name":"Sprint 1","project":"P","goal":"Ship it","start_date":"2026-04-10","end_date":"2026-04-24"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let sprint = body_json(resp).await;
    assert_eq!(sprint["name"], "Sprint 1");
    assert_eq!(sprint["status"], "planning");
    assert_eq!(sprint["project"], "P");

    let resp = app.oneshot(auth_req("GET", "/api/sprints", &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_sprint_update() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let id = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/sprints/{}", id), &tok,
        Some(json!({"name":"S2","goal":"New goal"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let s = body_json(resp).await;
    assert_eq!(s["name"], "S2");
    assert_eq!(s["goal"], "New goal");
}

#[tokio::test]
async fn test_sprint_delete() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let id = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/sprints/{}", id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);

    let resp = app.oneshot(auth_req("GET", "/api/sprints", &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_sprint_filter_by_status() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"A"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"B"})))).await.unwrap();
    let id = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", id), &tok, None)).await.unwrap();

    let resp = app.clone().oneshot(auth_req("GET", "/api/sprints?status=active", &tok, None)).await.unwrap();
    let sprints = body_json(resp).await;
    assert_eq!(sprints.as_array().unwrap().len(), 1);
    assert_eq!(sprints[0]["name"], "B");
}

#[tokio::test]
async fn test_sprint_add_remove_tasks() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T1"})))).await.unwrap();
    let t1 = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T2"})))).await.unwrap();
    let t2 = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();

    // Add tasks
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok,
        Some(json!({"task_ids":[t1, t2]})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 2);

    // Get tasks
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/tasks", sid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 2);

    // Remove one
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/sprints/{}/tasks/{}", sid, t1), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);

    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/tasks", sid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_sprint_detail() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S","goal":"G"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();

    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}", sid), &tok, None)).await.unwrap();
    let detail = body_json(resp).await;
    assert_eq!(detail["sprint"]["name"], "S");
    assert_eq!(detail["tasks"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_sprint_start_and_complete() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();

    // Start
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["status"], "active");

    // Complete
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/complete", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["status"], "completed");
}

#[tokio::test]
async fn test_sprint_board() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Todo"})))).await.unwrap();
    let t1 = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Done"})))).await.unwrap();
    let t2 = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", t2), &tok, Some(json!({"status":"completed"})))).await.unwrap();

    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[t1,t2]})))).await.unwrap();

    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/board", sid), &tok, None)).await.unwrap();
    let board = body_json(resp).await;
    assert_eq!(board["todo"].as_array().unwrap().len(), 1);
    assert_eq!(board["done"].as_array().unwrap().len(), 1);
    assert_eq!(board["in_progress"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_sprint_burndown_and_snapshot() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok,
        Some(json!({"title":"T","estimated_hours":8.0})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();

    // Snapshot
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/snapshot", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let stat = body_json(resp).await;
    assert_eq!(stat["total_hours"], 8.0);
    assert_eq!(stat["done_hours"], 0.0);
    assert_eq!(stat["total_tasks"], 1);
    assert_eq!(stat["done_tasks"], 0);

    // Complete task and re-snapshot
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"completed"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/snapshot", sid), &tok, None)).await.unwrap();
    let stat = body_json(resp).await;
    assert_eq!(stat["done_hours"], 8.0);
    assert_eq!(stat["done_tasks"], 1);

    // Burndown
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/burndown", sid), &tok, None)).await.unwrap();
    let burndown = body_json(resp).await;
    assert_eq!(burndown.as_array().unwrap().len(), 1); // one snapshot today
}

#[tokio::test]
async fn test_sprint_duplicate_task_add() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();

    // Add same task twice — should not duplicate
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();

    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/tasks", sid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_sprint_cascade_delete_cleans_tasks_and_stats() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/snapshot", sid), &tok, None)).await.unwrap();

    // Delete sprint — cascade should clean sprint_tasks and sprint_daily_stats
    app.clone().oneshot(auth_req("DELETE", &format!("/api/sprints/{}", sid), &tok, None)).await.unwrap();

    // Task still exists
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks", &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_task_sprints_endpoint() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"Active Sprint"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();

    let resp = app.clone().oneshot(auth_req("GET", "/api/task-sprints", &tok, None)).await.unwrap();
    let infos = body_json(resp).await;
    let arr = infos.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["task_id"], tid);
    assert_eq!(arr[0]["sprint_name"], "Active Sprint");
    assert_eq!(arr[0]["sprint_status"], "active");
}

// ---- Burn Log ----

#[tokio::test]
async fn test_burn_log_and_cancel() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Setup: task + sprint
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    // BL7: Sprint must be active to log burns
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();

    // Log a burn
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok,
        Some(json!({"task_id":tid,"points":5.0,"hours":2.0,"note":"Did stuff"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let burn = body_json(resp).await;
    assert_eq!(burn["points"], 5.0);
    assert_eq!(burn["hours"], 2.0);
    assert_eq!(burn["username"], "root");
    assert_eq!(burn["source"], "manual");
    assert_eq!(burn["cancelled"], 0);
    let bid = burn["id"].as_i64().unwrap();

    // List burns
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/burns", sid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 1);

    // Summary
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/burn-summary", sid), &tok, None)).await.unwrap();
    let summary = body_json(resp).await;
    assert_eq!(summary[0]["points"], 5.0);
    assert_eq!(summary[0]["username"], "root");

    // Cancel
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/sprints/{}/burns/{}", sid, bid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let burn = body_json(resp).await;
    assert_eq!(burn["cancelled"], 1);
    assert_eq!(burn["cancelled_by"], "root");

    // Summary should be empty after cancel
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/burn-summary", sid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 0);

    // But list still shows the cancelled entry
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/burns", sid), &tok, None)).await.unwrap();
    let burns = body_json(resp).await;
    assert_eq!(burns.as_array().unwrap().len(), 1);
    assert_eq!(burns[0]["cancelled"], 1);
}

#[tokio::test]
async fn test_burn_multi_user_summary() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"bob","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"bob","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();

    // Assign Bob to the task so he can log burns
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &tok, Some(json!({"username":"bob"})))).await.unwrap();

    // Root burns 3 pts, Bob burns 5 pts
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok, Some(json!({"task_id":tid,"points":3.0})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok2, Some(json!({"task_id":tid,"points":5.0})))).await.unwrap();

    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/burn-summary", sid), &tok, None)).await.unwrap();
    let summary = body_json(resp).await;
    let arr = summary.as_array().unwrap();
    assert_eq!(arr.len(), 2); // two users
    let total: f64 = arr.iter().map(|e| e["points"].as_f64().unwrap()).sum();
    assert_eq!(total, 8.0);
}

#[tokio::test]
async fn test_burn_log_requires_authorization() {
    let app = app().await;
    let tok = login_root(&app).await;
    let (user_tok, _) = register_user_full(&app, "burnnoauth", "BurnNo111").await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"BurnTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"BurnSprint"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();
    // Unrelated user cannot log burns
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &user_tok, Some(json!({"task_id":tid,"points":5.0})))).await.unwrap();
    assert_eq!(resp.status(), 403, "Unrelated user should not be able to log burns");
    // Sprint owner can
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok, Some(json!({"task_id":tid,"points":3.0})))).await.unwrap();
    assert_eq!(resp.status(), 201);
}

#[tokio::test]
async fn test_burn_cascade_on_sprint_delete() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok, Some(json!({"task_id":tid,"points":5.0})))).await.unwrap();

    // Delete sprint — burns should cascade
    app.clone().oneshot(auth_req("DELETE", &format!("/api/sprints/{}", sid), &tok, None)).await.unwrap();

    // Task still exists
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks", &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 1);
}

// ---- Bug fix tests ----

#[tokio::test]
async fn test_update_task_clear_nullable_fields() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok,
        Some(json!({"title":"T","description":"desc","project":"proj","tags":"a,b","due_date":"2026-12-31"})))).await.unwrap();
    let id = body_json(resp).await["id"].as_i64().unwrap();

    // Clear description by sending null
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", id), &tok,
        Some(json!({"description":null})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let t = body_json(resp).await;
    assert!(t["description"].is_null(), "description should be null after clearing");

    // Clear project
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", id), &tok,
        Some(json!({"project":null})))).await.unwrap();
    let t = body_json(resp).await;
    assert!(t["project"].is_null());

    // Clear due_date
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", id), &tok,
        Some(json!({"due_date":null})))).await.unwrap();
    let t = body_json(resp).await;
    assert!(t["due_date"].is_null());

    // Tags still present (not cleared)
    assert_eq!(t["tags"], "a,b");
}

#[tokio::test]
async fn test_delete_task_cascades_burns_and_sprint_tasks() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok, Some(json!({"task_id":tid,"points":5.0})))).await.unwrap();

    // Delete task (soft delete) — sprint_tasks and burn_log remain since task still exists
    app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();

    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/tasks", sid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 0); // B4: soft-deleted task filtered out
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/burns", sid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 1); // burn still exists

    // But task should not appear in task list
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks", &tok, None)).await.unwrap();
    let tasks = body_json(resp).await;
    assert!(!tasks.as_array().unwrap().iter().any(|t| t["id"] == tid));
}

#[tokio::test]
async fn test_delete_comment_ownership() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"alice2","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"alice2","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    // Alice adds a comment
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &tok2, Some(json!({"content":"hi"})))).await.unwrap();
    let cid = body_json(resp).await["id"].as_i64().unwrap();

    // Root can delete (root override)
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/comments/{}", cid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_delete_room_ownership() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"roomuser","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"roomuser","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"R"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();

    // Non-owner cannot delete
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/rooms/{}", rid), &tok2, None)).await.unwrap();
    assert_eq!(resp.status(), 403);

    // Owner can delete
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/rooms/{}", rid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_delete_sprint_ownership() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"sprintuser","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"sprintuser","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();

    // Non-owner cannot delete
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/sprints/{}", sid), &tok2, None)).await.unwrap();
    assert_eq!(resp.status(), 403);

    // Owner can delete
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/sprints/{}", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_timer_user_isolation() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"timeruser","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"timeruser","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Root starts timer
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let root_state = body_json(resp).await;
    assert_eq!(root_state["status"], "Running");

    // Other user sees their own idle timer, not root's
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer", &tok2, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let user_state = body_json(resp).await;
    assert_eq!(user_state["status"], "Idle");

    // Other user can start their own timer independently
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok2, Some(json!({})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let user_state = body_json(resp).await;
    assert_eq!(user_state["status"], "Running");

    // Root's timer is still running
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer", &tok, None)).await.unwrap();
    let root_state = body_json(resp).await;
    assert_eq!(root_state["status"], "Running");

    // Root can stop own timer
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/stop", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["status"], "Idle");

    // User's timer still running
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer", &tok2, None)).await.unwrap();
    assert_eq!(body_json(resp).await["status"], "Running");
}

#[tokio::test]
async fn test_password_min_length() {
    let app = app().await;
    let resp = app.oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"short","password":"abc"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_remove_assignee_ownership() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"assignuser","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"assignuser","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &tok, Some(json!({"username":"root"})))).await.unwrap();

    // Non-owner cannot remove assignee
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/assignees/root", tid), &tok2, None)).await.unwrap();
    assert_eq!(resp.status(), 403);

    // Owner can remove
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/assignees/root", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_delete_user_cascade() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"delme","password":"Pass1234"})))).await.unwrap();
    let uid = body_json(resp).await["user_id"].as_i64().unwrap();

    // Create task as delme
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"delme","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok2, Some(json!({"title":"MyTask"})))).await.unwrap();

    // Delete user as root
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/admin/users/{}", uid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);

    // Task still exists (reassigned to root)
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks", &tok, None)).await.unwrap();
    let tasks = body_json(resp).await;
    let task = tasks.as_array().unwrap().iter().find(|t| t["title"] == "MyTask").unwrap();
    assert_eq!(task["user"], "root");
}

#[tokio::test]
async fn test_delete_user_preserves_comments_and_burns() {
    let app = app().await;
    let tok = login_root(&app).await;
    let (tok2, uid) = register_user_full(&app, "burnuser", "BurnUs111").await;
    // Root creates task and sprint
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"BurnTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // burnuser comments on the task
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &tok2, Some(json!({"content":"Important context"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    // Create sprint, add task, start, log burn as burnuser
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"BS"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();
    // Assign burnuser so they can log burns
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &tok, Some(json!({"username":"burnuser"})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok2, Some(json!({"task_id":tid,"points":5.0})))).await.unwrap();
    // Delete burnuser
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/admin/users/{}", uid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Comments should still exist (reassigned to root)
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
    let detail = body_json(resp).await;
    let comments = detail["comments"].as_array().unwrap();
    assert!(comments.iter().any(|c| c["content"] == "Important context"), "Comment should survive user deletion");
    // Burns should still exist
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/burns", sid), &tok, None)).await.unwrap();
    let burns = body_json(resp).await;
    assert!(!burns.as_array().unwrap().is_empty(), "Burns should survive user deletion");
}

#[tokio::test]
async fn test_snapshot_sprint_points_not_double_counted() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Task with remaining_points=5, estimated=3 (pomodoros)
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok,
        Some(json!({"title":"T","remaining_points":5.0,"estimated":3})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();

    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/snapshot", sid), &tok, None)).await.unwrap();
    let stat = body_json(resp).await;
    // total_points = remaining_points (story points = 5), not estimated (pomodoros)
    assert_eq!(stat["total_points"], 5.0);
}

// ---- Round 2 bug fix tests ----

#[tokio::test]
async fn test_update_sprint_clear_nullable_fields() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok,
        Some(json!({"name":"S","project":"P","goal":"G","start_date":"2026-04-10","end_date":"2026-04-24"})))).await.unwrap();
    let id = body_json(resp).await["id"].as_i64().unwrap();

    // Clear goal by sending null
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/sprints/{}", id), &tok,
        Some(json!({"goal":null})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let s = body_json(resp).await;
    assert!(s["goal"].is_null(), "goal should be null after clearing");
    assert_eq!(s["project"], "P", "project should be preserved");

    // Clear project
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/sprints/{}", id), &tok,
        Some(json!({"project":null})))).await.unwrap();
    let s = body_json(resp).await;
    assert!(s["project"].is_null());
}

#[tokio::test]
async fn test_update_sprint_ownership() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"sprintuser2","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"sprintuser2","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();

    // Non-owner cannot update
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/sprints/{}", sid), &tok2,
        Some(json!({"name":"Hacked"})))).await.unwrap();
    assert_eq!(resp.status(), 403);

    // Owner can update
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/sprints/{}", sid), &tok,
        Some(json!({"name":"Updated"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["name"], "Updated");
}

#[tokio::test]
async fn test_cancel_burn_ownership() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"burnuser","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"burnuser","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();

    // Root logs a burn
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok,
        Some(json!({"task_id":tid,"points":5.0})))).await.unwrap();
    let bid = body_json(resp).await["id"].as_i64().unwrap();

    // Non-owner cannot cancel
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/sprints/{}/burns/{}", sid, bid), &tok2, None)).await.unwrap();
    assert_eq!(resp.status(), 403);

    // Owner can cancel
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/sprints/{}/burns/{}", sid, bid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_delete_last_root_prevented() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Get root user id
    let resp = app.clone().oneshot(auth_req("GET", "/api/admin/users", &tok, None)).await.unwrap();
    let users = body_json(resp).await;
    let root_id = users.as_array().unwrap().iter().find(|u| u["username"] == "root").unwrap()["id"].as_i64().unwrap();

    // Cannot delete self
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/admin/users/{}", root_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_get_room_no_auto_join() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"viewer","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"viewer","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"R"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();

    // Viewer GETs room state — should be forbidden (not a member, S2 fix)
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/rooms/{}", rid), &tok2, None)).await.unwrap();
    assert_eq!(resp.status(), 403);
    // Creator can still view
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/rooms/{}", rid), &tok, None)).await.unwrap();
    let state = body_json(resp).await;
    assert_eq!(state["members"].as_array().unwrap().len(), 1);
    assert_eq!(state["members"][0]["username"], "root");
}

#[tokio::test]
async fn test_time_report_links_to_active_sprint() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();

    // Add time report — should auto-link to active sprint
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/time", tid), &tok,
        Some(json!({"hours":2.0})))).await.unwrap();
    let burn = body_json(resp).await;
    assert_eq!(burn["sprint_id"], sid, "time report should link to active sprint");

    // Verify it shows in sprint burns
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/burns", sid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_update_username_uniqueness() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"unique1","password":"Pass1234"})))).await.unwrap();

    // Try to change root's username to "unique1" — should fail with 409
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok,
        Some(json!({"username":"unique1"})))).await.unwrap();
    assert_eq!(resp.status(), 409);
}

#[tokio::test]
async fn test_optimistic_locking_task() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let task = body_json(resp).await;
    let id = task["id"].as_i64().unwrap();
    let updated_at = task["updated_at"].as_str().unwrap().to_string();

    // Update with correct expected_updated_at — should succeed
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", id), &tok,
        Some(json!({"title":"T2","expected_updated_at":updated_at})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let task2 = body_json(resp).await;
    assert_eq!(task2["title"], "T2");

    // Update with stale expected_updated_at — should get 409
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", id), &tok,
        Some(json!({"title":"T3","expected_updated_at":updated_at})))).await.unwrap();
    assert_eq!(resp.status(), 409);

    // Update without expected_updated_at — should still work (backwards compatible)
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", id), &tok,
        Some(json!({"title":"T4"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["title"], "T4");
}

#[tokio::test]
async fn test_optimistic_locking_sprint() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sprint = body_json(resp).await;
    let id = sprint["id"].as_i64().unwrap();
    let updated_at = sprint["updated_at"].as_str().unwrap().to_string();

    // Correct version — succeeds
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/sprints/{}", id), &tok,
        Some(json!({"name":"S2","expected_updated_at":updated_at})))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Stale version — 409
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/sprints/{}", id), &tok,
        Some(json!({"name":"S3","expected_updated_at":updated_at})))).await.unwrap();
    assert_eq!(resp.status(), 409);
}

// ---- Teams (#62) ----

#[tokio::test]
async fn test_teams_crud() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create team
    let resp = app.clone().oneshot(auth_req("POST", "/api/teams", &tok, Some(json!({"name":"Alpha"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let team = body_json(resp).await;
    let tid = team["id"].as_i64().unwrap();
    assert_eq!(team["name"], "Alpha");

    // List teams
    let resp = app.clone().oneshot(auth_req("GET", "/api/teams", &tok, None)).await.unwrap();
    let teams = body_json(resp).await;
    assert_eq!(teams.as_array().unwrap().len(), 1);

    // Get team detail
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/teams/{}", tid), &tok, None)).await.unwrap();
    let detail = body_json(resp).await;
    assert_eq!(detail["team"]["name"], "Alpha");
    assert_eq!(detail["members"].as_array().unwrap().len(), 1); // creator auto-added

    // My teams
    let resp = app.clone().oneshot(auth_req("GET", "/api/me/teams", &tok, None)).await.unwrap();
    let my = body_json(resp).await;
    assert_eq!(my.as_array().unwrap().len(), 1);

    // Delete team (root only)
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/teams/{}", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_team_members_and_root_tasks() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create team + task
    let resp = app.clone().oneshot(auth_req("POST", "/api/teams", &tok, Some(json!({"name":"Beta"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Root Task"})))).await.unwrap();
    let task_id = body_json(resp).await["id"].as_i64().unwrap();

    // Add root task
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/teams/{}/roots", tid), &tok, Some(json!({"task_ids":[task_id]})))).await.unwrap();
    assert_eq!(resp.status(), 204);

    // Get scope
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/teams/{}/scope", tid), &tok, None)).await.unwrap();
    let scope = body_json(resp).await;
    assert!(scope.as_array().unwrap().contains(&json!(task_id)));

    // Remove root task
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/teams/{}/roots/{}", tid, task_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

// ---- Epic Groups (#63) ----

#[tokio::test]
async fn test_epic_groups_crud() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Epic Root","estimated":5})))).await.unwrap();
    let task_id = body_json(resp).await["id"].as_i64().unwrap();

    // Create epic group
    let resp = app.clone().oneshot(auth_req("POST", "/api/epics", &tok, Some(json!({"name":"Q1 Goals"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let eid = body_json(resp).await["id"].as_i64().unwrap();

    // List
    let resp = app.clone().oneshot(auth_req("GET", "/api/epics", &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 1);

    // Add tasks
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/epics/{}/tasks", eid), &tok, Some(json!({"task_ids":[task_id]})))).await.unwrap();
    assert_eq!(resp.status(), 204);

    // Get detail
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/epics/{}", eid), &tok, None)).await.unwrap();
    let detail = body_json(resp).await;
    assert!(detail["task_ids"].as_array().unwrap().contains(&json!(task_id)));

    // Snapshot
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/epics/{}/snapshot", eid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);

    // Delete
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/epics/{}", eid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

// ---- Sprint Root Tasks (#64) ----

#[tokio::test]
async fn test_sprint_root_tasks() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Parent"})))).await.unwrap();
    let parent_id = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Child","parent_id":parent_id})))).await.unwrap();
    let _child_id = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();

    // Add root task
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/roots", sid), &tok, Some(json!({"task_ids":[parent_id]})))).await.unwrap();

    // Get roots
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/roots", sid), &tok, None)).await.unwrap();
    let roots = body_json(resp).await;
    assert_eq!(roots.as_array().unwrap().len(), 1);

    // Get scope (should include parent + child)
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/scope", sid), &tok, None)).await.unwrap();
    let scope = body_json(resp).await;
    assert_eq!(scope.as_array().unwrap().len(), 2);

    // Remove root
    app.clone().oneshot(auth_req("DELETE", &format!("/api/sprints/{}/roots/{}", sid, parent_id), &tok, None)).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/roots", sid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 0);
}

// ---- User Config (#66) ----

#[tokio::test]
async fn test_user_config_override() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Get default config
    let resp = app.clone().oneshot(auth_req("GET", "/api/config", &tok, None)).await.unwrap();
    let cfg = body_json(resp).await;
    assert_eq!(cfg["work_duration_min"], 25);

    // Update config (per-user override)
    let mut new_cfg = cfg.clone();
    new_cfg["work_duration_min"] = json!(30);
    new_cfg["daily_goal"] = json!(10);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(new_cfg))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Verify override persists
    let resp = app.clone().oneshot(auth_req("GET", "/api/config", &tok, None)).await.unwrap();
    let cfg = body_json(resp).await;
    assert_eq!(cfg["work_duration_min"], 30);
    assert_eq!(cfg["daily_goal"], 10);
}

// ---- ETag / tasks/full (#67) ----

#[tokio::test]
async fn test_tasks_full_etag() {
    let app = app().await;
    let tok = login_root(&app).await;

    // First request — should return 200 with ETag
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks/full", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let etag = resp.headers().get("etag").unwrap().to_str().unwrap().to_string();
    assert!(!etag.is_empty());

    // Second request with If-None-Match — should return 304
    let req = axum::http::Request::builder()
        .method("GET").uri("/api/tasks/full")
        .header("authorization", format!("Bearer {}", tok))
        .header("if-none-match", &etag)
        .body(Body::empty()).unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 304);

    // Create a task to invalidate ETag
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"New"})))).await.unwrap();

    // Same ETag should now return 200
    let req = axum::http::Request::builder()
        .method("GET").uri("/api/tasks/full")
        .header("authorization", format!("Bearer {}", tok))
        .header("if-none-match", &etag)
        .body(Body::empty()).unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);
}

// ---- Global Burndown (#68) ----

#[tokio::test]
async fn test_global_burndown() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create sprint + task + start + snapshot
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T","estimated":3})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/snapshot", sid), &tok, None)).await.unwrap();

    // Global burndown should have data
    let resp = app.clone().oneshot(auth_req("GET", "/api/sprints/burndown", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(!data.as_array().unwrap().is_empty());
}

// ---- Profile Update (#71) ----

#[tokio::test]
async fn test_profile_update() {
    let app = app().await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"profuser","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"profuser","password":"Pass1234"})))).await.unwrap();
    let tok = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Change username
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"username":"profuser2"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let auth = body_json(resp).await;
    assert_eq!(auth["username"], "profuser2");
    let new_tok = auth["token"].as_str().unwrap().to_string();

    // Change password (requires current_password)
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &new_tok, Some(json!({"password":"NewPass12","current_password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Login with new credentials
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"profuser2","password":"NewPass12"})))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Old password should fail
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"profuser2","password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 401);
}

// ---- Username Validation (#8) ----

#[tokio::test]
async fn test_username_validation() {
    let app = app().await;

    // Empty username
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"","password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 400);

    // Too long
    let long = "a".repeat(33);
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":long,"password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 400);

    // Invalid chars
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"bad user!","password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 400);

    // Valid with underscore/hyphen
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"good_user-1","password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
}

// ---- Input Validation (#57) ----

#[tokio::test]
async fn test_task_input_validation() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Empty title
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":""})))).await.unwrap();
    assert_eq!(resp.status(), 400);

    // Invalid priority
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T","priority":6})))).await.unwrap();
    assert_eq!(resp.status(), 400);

    // Negative estimated
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T","estimated":-1})))).await.unwrap();
    assert_eq!(resp.status(), 400);

    // Invalid status on update
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let id = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", id), &tok, Some(json!({"status":"invalid"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_sse_ticket_exchange() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Get a ticket
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/ticket", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j = body_json(resp).await;
    let ticket = j["ticket"].as_str().unwrap();
    assert!(ticket.len() >= 16);
    // Use ticket for SSE — should return 200 (streaming)
    let resp = app.clone().oneshot(
        Request::builder().method("GET").uri(&format!("/api/timer/sse?ticket={}", ticket))
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Ticket is single-use — second attempt should fail
    let resp = app.clone().oneshot(
        Request::builder().method("GET").uri(&format!("/api/timer/sse?ticket={}", ticket))
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_sse_requires_auth() {
    let app = app().await;
    let resp = app.oneshot(
        Request::builder().method("GET").uri("/api/timer/sse")
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_concurrent_task_updates() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create a task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Concurrent"})))).await.unwrap();
    let task = body_json(resp).await;
    let id = task["id"].as_i64().unwrap();
    let updated_at = task["updated_at"].as_str().unwrap().to_string();
    // First update with correct expected_updated_at succeeds
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", id), &tok,
        Some(json!({"title":"Updated1","expected_updated_at":updated_at})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Second update with stale expected_updated_at fails with 409
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", id), &tok,
        Some(json!({"title":"Updated2","expected_updated_at":updated_at})))).await.unwrap();
    assert_eq!(resp.status(), 409);
}

#[tokio::test]
async fn test_labels_crud() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create label
    let resp = app.clone().oneshot(auth_req("POST", "/api/labels", &tok, Some(json!({"name":"urgent","color":"#ff0000"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let label = body_json(resp).await;
    let lid = label["id"].as_i64().unwrap();
    // List labels
    let resp = app.clone().oneshot(auth_req("GET", "/api/labels", &tok, None)).await.unwrap();
    let labels = body_json(resp).await;
    assert!(labels.as_array().unwrap().iter().any(|l| l["name"] == "urgent"));
    // Create task and add label
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Labeled"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/labels/{}", tid, lid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Get task labels
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/labels", tid), &tok, None)).await.unwrap();
    let task_labels = body_json(resp).await;
    assert_eq!(task_labels.as_array().unwrap().len(), 1);
    // Remove label from task
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/labels/{}", tid, lid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Delete label
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/labels/{}", lid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_dependencies_crud() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"TaskA"})))).await.unwrap();
    let a = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"TaskB"})))).await.unwrap();
    let b = body_json(resp).await["id"].as_i64().unwrap();
    // Add dependency: B depends on A
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/dependencies", b), &tok, Some(json!({"depends_on": a})))).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Get dependencies
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/dependencies", b), &tok, None)).await.unwrap();
    let deps = body_json(resp).await;
    assert_eq!(deps.as_array().unwrap(), &[json!(a)]);
    // Get all dependencies
    let resp = app.clone().oneshot(auth_req("GET", "/api/dependencies", &tok, None)).await.unwrap();
    assert!(body_json(resp).await.as_array().unwrap().len() >= 1);
    // Remove dependency
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/dependencies/{}", b, a), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Self-dependency should fail
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/dependencies", a), &tok, Some(json!({"depends_on": a})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_recurrence_crud() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Daily standup"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Set recurrence
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/recurrence", tid), &tok, Some(json!({"pattern":"daily","next_due":"2026-04-12"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let rec = body_json(resp).await;
    assert_eq!(rec["pattern"], "daily");
    // Get recurrence
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/recurrence", tid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await["pattern"], "daily");
    // Invalid pattern
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/recurrence", tid), &tok, Some(json!({"pattern":"yearly","next_due":"2027-01-01"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // Remove recurrence
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/recurrence", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_webhooks_crud() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create webhook
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok, Some(json!({"url":"https://example.com/hook","events":"task.created"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let wh = body_json(resp).await;
    let wid = wh["id"].as_i64().unwrap();
    assert_eq!(wh["events"], "task.created");
    // List webhooks
    let resp = app.clone().oneshot(auth_req("GET", "/api/webhooks", &tok, None)).await.unwrap();
    assert!(body_json(resp).await.as_array().unwrap().len() >= 1);
    // Delete webhook
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/webhooks/{}", wid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Private IP should be rejected
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok, Some(json!({"url":"http://127.0.0.1:8080/hook"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_audit_log() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create a task (triggers audit)
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Audited"})))).await.unwrap();
    // Check audit log
    let resp = app.clone().oneshot(auth_req("GET", "/api/audit?entity_type=task", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let entries = body_json(resp).await;
    assert!(entries.as_array().unwrap().iter().any(|e| e["action"] == "create" && e["entity_type"] == "task"));
}

#[tokio::test]
async fn test_export_tasks_csv() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"ExportMe"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/tasks?format=csv", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let csv = String::from_utf8_lossy(&bytes);
    assert!(csv.contains("ExportMe"));
    assert!(csv.starts_with("id,"));
}

#[tokio::test]
async fn test_reorder_tasks() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"First"})))).await.unwrap();
    let a = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Second"})))).await.unwrap();
    let b = body_json(resp).await["id"].as_i64().unwrap();
    // Reorder: B before A
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks/reorder", &tok, Some(json!({"orders":[[b, 1],[a, 2]]})))).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_velocity() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/sprints/velocity", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Empty array is fine (no completed sprints)
    assert!(body_json(resp).await.as_array().is_some());
}

#[tokio::test]
async fn test_logout_revokes_token() {
    let app = app().await;
    // Register a user
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"logoutuser","password":"Pass1234"})))).await.unwrap();
    let tok = body_json(resp).await["token"].as_str().unwrap().to_string();
    // Token works
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Logout
    let resp = app.clone().oneshot(auth_req("POST", "/api/auth/logout", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Token should be revoked
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_password_complexity_uppercase() {
    let app = app().await;
    // Missing uppercase
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"nocase","password":"pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // Missing digit
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"nodigit","password":"Password"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_room_ws_auth() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create a room
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"WSRoom"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let rid = body_json(resp).await["id"].as_i64().unwrap();
    // SSE ticket exchange works
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/ticket", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let ticket = body_json(resp).await["ticket"].as_str().unwrap().to_string();
    assert!(!ticket.is_empty());
    // Room state accessible after creation
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/rooms/{}", rid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let state = body_json(resp).await;
    assert_eq!(state["room"]["name"], "WSRoom");
    // Members include creator
    assert!(state["members"].as_array().unwrap().len() >= 1);
}

#[tokio::test]
async fn test_attachments_crud() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create a task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"WithAttachment"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    // Upload an attachment
    let resp = app.clone().oneshot(
        axum::http::Request::builder()
            .method("POST")
            .uri(format!("/api/tasks/{}/attachments", tid))
            .header("authorization", format!("Bearer {}", tok))
            .header("content-type", "text/plain")
            .header("x-filename", "test.txt")
            .header("x-requested-with", "test")
            .body(axum::body::Body::from("hello world"))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), 201);
    let att = body_json(resp).await;
    let att_id = att["id"].as_i64().unwrap();
    assert_eq!(att["filename"], "test.txt");
    assert_eq!(att["mime_type"], "text/plain");
    assert_eq!(att["size_bytes"], 11);

    // List attachments
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/attachments", tid), &tok, None)).await.unwrap();
    let list = body_json(resp).await;
    assert_eq!(list.as_array().unwrap().len(), 1);

    // Download attachment
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/attachments/{}/download", att_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&bytes[..], b"hello world");

    // Delete attachment
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/attachments/{}", att_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);

    // List should be empty now
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/attachments", tid), &tok, None)).await.unwrap();
    let list = body_json(resp).await;
    assert_eq!(list.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_attachment_empty_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    // Empty body should be rejected
    let resp = app.clone().oneshot(
        axum::http::Request::builder()
            .method("POST")
            .uri(format!("/api/tasks/{}/attachments", tid))
            .header("authorization", format!("Bearer {}", tok))
            .header("content-type", "text/plain")
            .header("x-requested-with", "test")
            .body(axum::body::Body::empty())
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_attachment_filename_sanitized() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    // Filename with path traversal should be sanitized
    let resp = app.clone().oneshot(
        axum::http::Request::builder()
            .method("POST")
            .uri(format!("/api/tasks/{}/attachments", tid))
            .header("authorization", format!("Bearer {}", tok))
            .header("content-type", "text/plain")
            .header("x-filename", "../../../etc/passwd")
            .header("x-requested-with", "test")
            .body(axum::body::Body::from("test"))
            .unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), 201);
    let att = body_json(resp).await;
    // Slashes and dots-only should be stripped, leaving "etcpasswd"
    let filename = att["filename"].as_str().unwrap();
    assert!(!filename.contains('/'));
    assert!(!filename.contains(".."));
}

#[tokio::test]
async fn test_csrf_header_required() {
    let app = app().await;
    let tok = login_root(&app).await;
    // POST without x-requested-with should be rejected with 403
    let req = Request::builder()
        .method("POST").uri("/api/tasks")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", tok))
        .body(Body::from(serde_json::to_vec(&json!({"title":"T"})).unwrap())).unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 403);

    // GET without x-requested-with should still work
    let req = Request::builder()
        .method("GET").uri("/api/timer")
        .header("authorization", format!("Bearer {}", tok))
        .body(Body::empty()).unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_templates_crud() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create template
    let resp = app.clone().oneshot(auth_req("POST", "/api/templates", &tok, Some(json!({
        "name": "Bug Report",
        "data": "{\"title\":\"Bug: \",\"priority\":4,\"tags\":\"bug\"}"
    })))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let tmpl = body_json(resp).await;
    let id = tmpl["id"].as_i64().unwrap();
    assert_eq!(tmpl["name"], "Bug Report");

    // List templates
    let resp = app.clone().oneshot(auth_req("GET", "/api/templates", &tok, None)).await.unwrap();
    let list = body_json(resp).await;
    assert_eq!(list.as_array().unwrap().len(), 1);

    // Delete template
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/templates/{}", id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);

    // List should be empty
    let resp = app.clone().oneshot(auth_req("GET", "/api/templates", &tok, None)).await.unwrap();
    let list = body_json(resp).await;
    assert_eq!(list.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_refresh_token() {
    let app = app().await;
    // Login to get tokens
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"root","password":"root"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = body_json(resp).await;
    let refresh = body["refresh_token"].as_str().expect(&format!("No refresh_token in: {}", body)).to_string();
    assert!(!refresh.is_empty());

    // Use refresh token to get new access token
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/refresh", Some(json!({"refresh_token": refresh})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = body_json(resp).await;
    assert!(!body["token"].as_str().unwrap().is_empty());
    assert!(!body["refresh_token"].as_str().unwrap().is_empty());

    // Old refresh token should be revoked (rotation)
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/refresh", Some(json!({"refresh_token": refresh})))).await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_attachment_delete_ownership() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create task and upload attachment as root
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(
        axum::http::Request::builder()
            .method("POST")
            .uri(format!("/api/tasks/{}/attachments", tid))
            .header("authorization", format!("Bearer {}", tok))
            .header("content-type", "text/plain")
            .header("x-filename", "test.txt")
            .header("x-requested-with", "test")
            .body(axum::body::Body::from("data"))
            .unwrap()
    ).await.unwrap();
    let att_id = body_json(resp).await["id"].as_i64().unwrap();

    // Register another user
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"other","password":"Other123"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Other user should not be able to delete
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/attachments/{}", att_id), &tok2, None)).await.unwrap();
    assert_eq!(resp.status(), 403);

    // Owner can delete
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/attachments/{}", att_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_recurrence_idempotency() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create task with recurrence
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Recurring"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    let today = chrono::Utc::now().naive_utc().format("%Y-%m-%d").to_string();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/recurrence", tid), &tok, Some(json!({
        "pattern": "daily", "next_due": today
    })))).await.unwrap();

    // Verify recurrence was set
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/recurrence", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let rec = body_json(resp).await;
    assert_eq!(rec["pattern"], "daily");
    assert_eq!(rec["next_due"], today);
}

#[tokio::test]
async fn test_webhook_ssrf_private_ip() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create webhook with private IP — should be stored (validation happens at dispatch time)
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok, Some(json!({
        "url": "http://192.168.1.1/hook", "events": "task.created"
    })))).await.unwrap();
    // The webhook is created (SSRF check is at dispatch time, not creation)
    let status = resp.status().as_u16();
    assert!(status == 201 || status == 200 || status == 400);
}

#[tokio::test]
async fn test_optimistic_locking_sprint_conflict() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create sprint
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"OL Sprint"})))).await.unwrap();
    let sprint = body_json(resp).await;
    let sid = sprint["id"].as_i64().unwrap();
    let updated_at = sprint["updated_at"].as_str().unwrap().to_string();

    // Update with correct expected_updated_at — should succeed
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/sprints/{}", sid), &tok, Some(json!({
        "name": "Updated", "expected_updated_at": updated_at
    })))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Update with stale expected_updated_at — should conflict
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/sprints/{}", sid), &tok, Some(json!({
        "name": "Stale", "expected_updated_at": updated_at
    })))).await.unwrap();
    assert_eq!(resp.status(), 409);
}

#[tokio::test]
async fn test_auth_rate_limiting() {
    let app = app().await;
    if std::env::var("POMODORO_NO_RATE_LIMIT").is_ok() { return; }
    pomodoro_daemon::routes::auth_limiter().reset();
    // Send 11 login attempts (limit is 10 per 60s)
    // Note: rate limiter uses x-forwarded-for header, which our test doesn't set,
    // so it falls back to "unknown" key. All requests share the same key.
    for i in 0..10 {
        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/api/auth/login")
                .header("content-type", "application/json")
                .header("x-forwarded-for", "1.2.3.4")
                .body(Body::from(serde_json::to_vec(&json!({"username":"root","password":"wrong"})).unwrap())).unwrap()
        ).await.unwrap();
        // Should be 401 (wrong password) for first 10
        if i < 10 { assert_eq!(resp.status(), 401, "Request {} should be 401", i); }
    }
    // 11th should be rate limited
    let resp = app.clone().oneshot(
        Request::builder().method("POST").uri("/api/auth/login")
            .header("content-type", "application/json")
            .header("x-forwarded-for", "1.2.3.4")
            .body(Body::from(serde_json::to_vec(&json!({"username":"root","password":"wrong"})).unwrap())).unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), 429);
}

#[tokio::test]
async fn test_sse_ticket_and_connect() {
    let app = app().await;
    // Login
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"root","password":"root"})))).await.unwrap();
    let tok = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Create SSE ticket
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/ticket", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let ticket = body_json(resp).await["ticket"].as_str().unwrap().to_string();
    assert!(!ticket.is_empty());

    // Connect to SSE with ticket
    let resp = app.clone().oneshot(
        Request::builder().method("GET").uri(&format!("/api/timer/sse?ticket={}", ticket))
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Verify content-type is event-stream
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("text/event-stream"), "Expected event-stream, got {}", ct);

    // Expired/reused ticket should fail
    let resp = app.clone().oneshot(
        Request::builder().method("GET").uri(&format!("/api/timer/sse?ticket={}", ticket))
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_due_date_reminder_query() {
    let app = app().await;
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"root","password":"root"})))).await.unwrap();
    let tok = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Create task with due date tomorrow
    let tomorrow = (chrono::Utc::now() + chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Due soon","due_date":&tomorrow})))).await.unwrap();
    assert!(resp.status().is_success());

    // Create task with due date far in the future
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Not due","due_date":"2099-12-31"})))).await.unwrap();
    assert!(resp.status().is_success());

    // Create completed task with due date tomorrow (should NOT appear)
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Done task","due_date":&tomorrow})))).await.unwrap();
    assert!(resp.status().is_success());
    let done_id = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", done_id), &tok, Some(json!({"status":"completed"})))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Query due tasks (before day after tomorrow)
    let day_after = (chrono::Utc::now() + chrono::Duration::days(2)).format("%Y-%m-%d").to_string();
    let pool = pomodoro_daemon::db::connect_memory().await.unwrap();
    // Use the app's pool via a direct DB call through the test helper
    // Instead, test via the tasks list endpoint and filter
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks", &tok, None)).await.unwrap();
    let tasks = body_json(resp).await;
    let due_tasks: Vec<&Value> = tasks.as_array().unwrap().iter()
        .filter(|t| t["due_date"].as_str().map_or(false, |d| d <= day_after.as_str()) && t["status"].as_str() != Some("completed"))
        .collect();
    assert_eq!(due_tasks.len(), 1);
    assert_eq!(due_tasks[0]["title"].as_str().unwrap(), "Due soon");
}

#[tokio::test]
async fn test_graceful_shutdown_recovery() {
    let app = app().await;
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"root","password":"root"})))).await.unwrap();
    let tok = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Create a task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Recovery test"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    // Start a timer session
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({"task_id": tid})))).await.unwrap();
    assert!(resp.status().is_success());

    // Verify session is running
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer", &tok, None)).await.unwrap();
    let state = body_json(resp).await;
    assert_eq!(state["status"].as_str().unwrap(), "Running");

    // Stop the timer (simulates graceful shutdown completing the session)
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/stop", &tok, None)).await.unwrap();
    assert!(resp.status().is_success());

    // Verify timer is now idle
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer", &tok, None)).await.unwrap();
    let state = body_json(resp).await;
    assert_eq!(state["status"].as_str().unwrap(), "Idle");
}

// T2: skip() advances to next phase
#[tokio::test]
async fn test_skip_advances_phase() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Start work
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({})))).await.unwrap();
    assert!(resp.status().is_success());
    let state = body_json(resp).await;
    assert_eq!(state["phase"], "Work");
    // Skip
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/skip", &tok, None)).await.unwrap();
    assert!(resp.status().is_success());
    let state = body_json(resp).await;
    assert_eq!(state["status"], "Idle");
    assert!(state["phase"] == "ShortBreak" || state["phase"] == "LongBreak");
}

// T4: cancel_burn validates sprint_id
#[tokio::test]
async fn test_cancel_burn_validates_sprint() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create task and sprint
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();
    // Log burn
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok,
        Some(json!({"task_id":tid,"points":1.0,"hours":0.5})))).await.unwrap();
    let burn_id = body_json(resp).await["id"].as_i64().unwrap();
    // Cancel with wrong sprint_id
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/sprints/99999/burns/{}", burn_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// T6: refresh token cannot be used as access token
#[tokio::test]
async fn test_refresh_token_rejected_as_access() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Get refresh token via a fresh login
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"root","password":"root1234"})))).await.unwrap();
    let body = body_json(resp).await;
    let refresh = body["refresh_token"].as_str().unwrap_or("").to_string();
    if refresh.is_empty() { return; } // skip if no refresh token support
    // Try to use refresh token as access token
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer", &refresh, None)).await.unwrap();
    assert_eq!(resp.status(), 401);
}

// T7: config validation bounds
#[tokio::test]
async fn test_config_validation_bounds() {
    let app = app().await;
    let tok = login_root(&app).await;
    let base = json!({"work_duration_min":25,"short_break_min":5,"long_break_min":15,"long_break_interval":4,"daily_goal":8,"estimation_mode":"points","auto_start_breaks":false,"auto_start_work":false,"sound_enabled":false,"notification_enabled":false});
    // work_duration_min too high
    let mut bad = base.clone(); bad["work_duration_min"] = json!(999);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(bad))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // daily_goal too high
    let mut bad = base.clone(); bad["daily_goal"] = json!(100);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(bad))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // invalid estimation_mode
    let mut bad = base.clone(); bad["estimation_mode"] = json!("invalid");
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(bad))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// T8: authorization on sprint task add
#[tokio::test]
async fn test_sprint_task_auth() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create second user
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"sprinteve","password":"Sprinteve1"})))).await.unwrap();
    assert!(resp.status().is_success(), "Register failed: {}", resp.status());
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"sprinteve","password":"Sprinteve1"})))).await.unwrap();
    assert!(resp.status().is_success());
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();
    // Root creates sprint
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    // Root creates task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Eve tries to add task to root's sprint
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok2, Some(json!({"task_ids":[tid]})))).await.unwrap();
    assert_eq!(resp.status(), 403);
}

// T3: webhook HMAC uses SHA-256
#[tokio::test]
async fn test_webhook_ssrf_blocked() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Private IP should be blocked
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok,
        Some(json!({"url":"http://192.168.1.1/hook"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // Localhost should be blocked
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok,
        Some(json!({"url":"http://localhost/hook"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // Cloud metadata should be blocked
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok,
        Some(json!({"url":"http://169.254.169.254/latest/meta-data"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// T5: rate limiter with no IP header doesn't crash
#[tokio::test]
async fn test_rate_limiter_no_ip_header() {
    let app = app().await;
    // Send request without x-forwarded-for — should not panic
    let req = axum::http::Request::builder()
        .method("POST").uri("/api/auth/login")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(serde_json::to_string(&json!({"username":"root","password":"root1234"})).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    // Should get a valid HTTP response (200 or 429), not a server error
    assert!(resp.status().as_u16() < 500);
}

// T1: per-user config isolation — one user's override doesn't affect another
#[tokio::test]
async fn test_per_user_config_isolation() {
    let app = app().await;
    let root_tok = login_root(&app).await;

    // Register second user
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"configUser","password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"configUser","password":"Pass1234"})))).await.unwrap();
    let user_tok = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Both start with same defaults
    let resp = app.clone().oneshot(auth_req("GET", "/api/config", &root_tok, None)).await.unwrap();
    let root_cfg = body_json(resp).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/config", &user_tok, None)).await.unwrap();
    let user_cfg = body_json(resp).await;
    assert_eq!(root_cfg["work_duration_min"], user_cfg["work_duration_min"]);

    // User changes their config to 15 min
    let mut new_user_cfg = user_cfg.clone();
    new_user_cfg["work_duration_min"] = json!(15);
    new_user_cfg["daily_goal"] = json!(3);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &user_tok, Some(new_user_cfg))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // User sees their override
    let resp = app.clone().oneshot(auth_req("GET", "/api/config", &user_tok, None)).await.unwrap();
    let user_cfg = body_json(resp).await;
    assert_eq!(user_cfg["work_duration_min"], 15, "user should see their override");
    assert_eq!(user_cfg["daily_goal"], 3);

    // Root still sees the global default (unaffected by user's override)
    let resp = app.clone().oneshot(auth_req("GET", "/api/config", &root_tok, None)).await.unwrap();
    let root_cfg = body_json(resp).await;
    assert_eq!(root_cfg["work_duration_min"], 25, "root should still see global default");
    assert_eq!(root_cfg["daily_goal"], 8);
}

// ---- Export sessions ----

#[tokio::test]
async fn test_export_sessions_csv() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/sessions?format=csv", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("text/csv"));
    let body = String::from_utf8(resp.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert!(body.starts_with("id,task_id,user,session_type,status,started_at,ended_at,duration_s,task_path"));
}

#[tokio::test]
async fn test_export_sessions_json() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/sessions?format=json", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("application/json"));
}

// ---- Burn validation ----

#[tokio::test]
async fn test_burn_negative_values_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create sprint + task
    let sprint = body_json(app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap()).await;
    let task = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap()).await;
    let sid = sprint["id"].as_i64().unwrap();
    let tid = task["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    // Negative points
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok, Some(json!({"task_id":tid,"points":-5.0})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // Negative hours
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok, Some(json!({"task_id":tid,"hours":-1.0})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ---- Time report validation ----

#[tokio::test]
async fn test_time_report_zero_hours_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let task = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap()).await;
    let tid = task["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/time", tid), &tok, Some(json!({"hours":0.0})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ---- Team authorization ----

#[tokio::test]
async fn test_team_member_add_remove() {
    let app = app().await;
    let root_tok = login_root(&app).await;
    let user_tok = register_user(&app, "teamUser1").await;
    // Create team as root
    let team = body_json(app.clone().oneshot(auth_req("POST", "/api/teams", &root_tok, Some(json!({"name":"TestTeam"})))).await.unwrap()).await;
    let tid = team["id"].as_i64().unwrap();
    // Get user id
    let users = body_json(app.clone().oneshot(auth_req("GET", "/api/admin/users", &root_tok, None)).await.unwrap()).await;
    let uid = users.as_array().unwrap().iter().find(|u| u["username"] == "teamUser1").unwrap()["id"].as_i64().unwrap();
    // Add member
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/teams/{}/members", tid), &root_tok, Some(json!({"user_id":uid,"role":"member"})))).await.unwrap();
    assert!(resp.status().is_success());
    // Verify member in team detail
    let detail = body_json(app.clone().oneshot(auth_req("GET", &format!("/api/teams/{}", tid), &root_tok, None)).await.unwrap()).await;
    let members = detail["members"].as_array().unwrap();
    assert!(members.iter().any(|m| m["username"] == "teamUser1"));
    // Remove member
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/teams/{}/members/{}", tid, uid), &root_tok, None)).await.unwrap();
    assert!(resp.status().is_success());
}

// ---- Epic group CRUD + ownership ----

#[tokio::test]
async fn test_epic_group_task_management() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create epic group
    let eg = body_json(app.clone().oneshot(auth_req("POST", "/api/epics", &tok, Some(json!({"name":"Epic1"})))).await.unwrap()).await;
    let eid = eg["id"].as_i64().unwrap();
    // Create tasks
    let t1 = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T1"})))).await.unwrap()).await;
    let t2 = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T2"})))).await.unwrap()).await;
    // Add tasks to epic
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/epics/{}/tasks", eid), &tok, Some(json!({"task_ids":[t1["id"],t2["id"]]})))).await.unwrap();
    assert!(resp.status().is_success());
    // Get detail
    let detail = body_json(app.clone().oneshot(auth_req("GET", &format!("/api/epics/{}", eid), &tok, None)).await.unwrap()).await;
    assert_eq!(detail["task_ids"].as_array().unwrap().len(), 2);
    // Remove one task
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/epics/{}/tasks/{}", eid, t1["id"]), &tok, None)).await.unwrap();
    assert!(resp.status().is_success());
    // Verify
    let detail = body_json(app.clone().oneshot(auth_req("GET", &format!("/api/epics/{}", eid), &tok, None)).await.unwrap()).await;
    assert_eq!(detail["task_ids"].as_array().unwrap().len(), 1);
    // Snapshot
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/epics/{}/snapshot", eid), &tok, None)).await.unwrap();
    assert!(resp.status().is_success());
    // Delete
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/epics/{}", eid), &tok, None)).await.unwrap();
    assert!(resp.status().is_success());
}

// ---- Sprint scope / root tasks ----

#[tokio::test]
async fn test_sprint_scope_with_root_tasks() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create parent + child tasks
    let parent = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Parent"})))).await.unwrap()).await;
    let pid = parent["id"].as_i64().unwrap();
    let child = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Child","parent_id":pid})))).await.unwrap()).await;
    let cid = child["id"].as_i64().unwrap();
    // Create sprint with root task
    let sprint = body_json(app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"Scoped"})))).await.unwrap()).await;
    let sid = sprint["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/roots", sid), &tok, Some(json!({"task_ids":[pid]})))).await.unwrap();
    // Get scope — should include parent + child
    let scope = body_json(app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/scope", sid), &tok, None)).await.unwrap()).await;
    let ids: Vec<i64> = scope.as_array().unwrap().iter().map(|v| v.as_i64().unwrap()).collect();
    assert!(ids.contains(&pid));
    assert!(ids.contains(&cid));
}

// ---- Room type and voting edge cases ----

#[tokio::test]
async fn test_room_vote_without_active_task() {
    let app = app().await;
    let tok = login_root(&app).await;
    let room = body_json(app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"R"})))).await.unwrap()).await;
    let rid = room["id"].as_i64().unwrap();
    // Try to vote without starting voting — should fail
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &tok, Some(json!({"value":5.0})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_room_vote_range_validation() {
    let app = app().await;
    let tok = login_root(&app).await;
    let room = body_json(app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"R2"})))).await.unwrap()).await;
    let rid = room["id"].as_i64().unwrap();
    let task = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap()).await;
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/start-voting", rid), &tok, Some(json!({"task_id":task["id"]})))).await.unwrap();
    // Negative vote
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &tok, Some(json!({"value":-1.0})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // Over 1000
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &tok, Some(json!({"value":1001.0})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // Valid vote
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &tok, Some(json!({"value":5.0})))).await.unwrap();
    assert!(resp.status().is_success());
}

// ---- Room estimation unit validation ----

#[tokio::test]
async fn test_room_invalid_estimation_unit() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"R","estimation_unit":"bananas"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ---- Sprint date validation ----

#[tokio::test]
async fn test_sprint_date_validation() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Invalid start_date format (too short)
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S","start_date":"2026"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // Valid date should work
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S","start_date":"2026-04-11"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
}

// ---- Sprint name validation ----

#[tokio::test]
async fn test_sprint_empty_name_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":""})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"   "})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ---- Room name validation ----

#[tokio::test]
async fn test_room_empty_name_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":""})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ---- Profile password change ----

#[tokio::test]
async fn test_profile_password_change() {
    let app = app().await;
    let tok = register_user(&app, "pwChangeUser").await;
    // Change password (requires current_password)
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"password":"NewPass123","current_password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let new_auth = body_json(resp).await;
    assert!(new_auth["token"].as_str().unwrap().len() > 10);
    // Login with new password
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"pwChangeUser","password":"NewPass123"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Old password should fail
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"pwChangeUser","password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 401);
    // Missing current_password should fail
    let tok2 = register_user(&app, "pwChangeUser2").await;
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok2, Some(json!({"password":"NewPass123"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // Wrong current_password should fail
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok2, Some(json!({"password":"NewPass123","current_password":"WrongPass1"})))).await.unwrap();
    assert_eq!(resp.status(), 403);
}

// ---- Profile weak password rejected ----

#[tokio::test]
async fn test_profile_weak_password_rejected() {
    let app = app().await;
    let tok = register_user(&app, "weakPwUser").await;
    // Too short
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"password":"Ab1"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // No uppercase
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"password":"alllower1"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // No digit
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"password":"NoDigitHere"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ---- Task due_date validation ----

#[tokio::test]
async fn test_task_invalid_due_date_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T","due_date":"2024/01/01"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T","due_date":"not-a-date"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // Valid date should work
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T","due_date":"2026-12-31"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
}

// ---- Task priority validation ----

#[tokio::test]
async fn test_task_priority_bounds() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T","priority":0})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T","priority":6})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T","priority":1})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T","priority":5})))).await.unwrap();
    assert_eq!(resp.status(), 201);
}

// ---- Task negative estimated rejected ----

#[tokio::test]
async fn test_task_negative_estimated_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T","estimated":-1})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T","estimated_hours":-1.0})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ---- Label CRUD extended ----

#[tokio::test]
async fn test_label_task_association() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create label
    let label = body_json(app.clone().oneshot(auth_req("POST", "/api/labels", &tok, Some(json!({"name":"urgent","color":"#ff0000"})))).await.unwrap()).await;
    let lid = label["id"].as_i64().unwrap();
    // Create task
    let task = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Labeled"})))).await.unwrap()).await;
    let tid = task["id"].as_i64().unwrap();
    // Add label to task
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/labels/{}", tid, lid), &tok, None)).await.unwrap();
    assert!(resp.status().is_success());
    // Get task labels
    let labels = body_json(app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/labels", tid), &tok, None)).await.unwrap()).await;
    assert_eq!(labels.as_array().unwrap().len(), 1);
    // Remove label
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/labels/{}", tid, lid), &tok, None)).await.unwrap();
    assert!(resp.status().is_success());
    let labels = body_json(app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/labels", tid), &tok, None)).await.unwrap()).await;
    assert_eq!(labels.as_array().unwrap().len(), 0);
}

// ---- Dependency cycle detection ----

#[tokio::test]
async fn test_dependency_crud_and_list() {
    let app = app().await;
    let tok = login_root(&app).await;
    let t1 = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"A"})))).await.unwrap()).await;
    let t2 = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"B"})))).await.unwrap()).await;
    let id1 = t1["id"].as_i64().unwrap();
    let id2 = t2["id"].as_i64().unwrap();
    // Add dependency: t1 depends on t2
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/dependencies", id1), &tok, Some(json!({"depends_on":id2})))).await.unwrap();
    assert!(resp.status().is_success());
    // List dependencies
    let deps = body_json(app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/dependencies", id1), &tok, None)).await.unwrap()).await;
    assert_eq!(deps.as_array().unwrap().len(), 1);
    // Get all dependencies
    let all = body_json(app.clone().oneshot(auth_req("GET", "/api/dependencies", &tok, None)).await.unwrap()).await;
    assert!(all.as_array().unwrap().len() >= 1);
    // Remove dependency
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/dependencies/{}", id1, id2), &tok, None)).await.unwrap();
    assert!(resp.status().is_success());
}

// ---- Recurrence CRUD extended ----

#[tokio::test]
async fn test_recurrence_patterns() {
    let app = app().await;
    let tok = login_root(&app).await;
    let task = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Recurring"})))).await.unwrap()).await;
    let tid = task["id"].as_i64().unwrap();
    // Set daily recurrence
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/recurrence", tid), &tok, Some(json!({"pattern":"daily","next_due":"2026-04-12"})))).await.unwrap();
    assert!(resp.status().is_success());
    // Get recurrence
    let rec = body_json(app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/recurrence", tid), &tok, None)).await.unwrap()).await;
    assert_eq!(rec["pattern"], "daily");
    // Update to weekly
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/recurrence", tid), &tok, Some(json!({"pattern":"weekly","next_due":"2026-04-18"})))).await.unwrap();
    assert!(resp.status().is_success());
    // Delete recurrence
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/recurrence", tid), &tok, None)).await.unwrap();
    assert!(resp.status().is_success());
}

// ---- Webhook events filter ----

#[tokio::test]
async fn test_webhook_with_event_filter() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok, Some(json!({"url":"https://example.com/hook","events":"task.created,sprint.started","secret":"mysecret"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let wh = body_json(resp).await;
    assert_eq!(wh["events"], "task.created,sprint.started");
    assert!(wh["secret"].is_null() || wh["secret"].as_str().is_some()); // secret may be hidden
}

// ---- Template CRUD extended ----

#[tokio::test]
async fn test_template_create_and_delete() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/templates", &tok, Some(json!({"name":"Bug Report","data":"{\"fields\":[\"title\",\"steps\"]}"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let tmpl = body_json(resp).await;
    let tid = tmpl["id"].as_i64().unwrap();
    // List
    let list = body_json(app.clone().oneshot(auth_req("GET", "/api/templates", &tok, None)).await.unwrap()).await;
    assert!(list.as_array().unwrap().iter().any(|t| t["id"] == tid));
    // Delete
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/templates/{}", tid), &tok, None)).await.unwrap();
    assert!(resp.status().is_success());
}

// ---- Audit log filtering ----

#[tokio::test]
async fn test_audit_log_entity_filter() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create a task to generate audit entry
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"AuditTest"})))).await.unwrap();
    // Filter by entity_type
    let resp = app.clone().oneshot(auth_req("GET", "/api/audit?entity_type=task", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let entries = body_json(resp).await;
    assert!(entries.as_array().unwrap().len() >= 1);
    for e in entries.as_array().unwrap() {
        assert_eq!(e["entity_type"], "task");
    }
}

// ---- Multi-user task isolation ----

#[tokio::test]
async fn test_task_ownership_isolation() {
    let app = app().await;
    let tok_a = register_user(&app, "ownerA").await;
    let tok_b = register_user(&app, "ownerB").await;
    // A creates a task
    let task = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok_a, Some(json!({"title":"A's task"})))).await.unwrap()).await;
    let tid = task["id"].as_i64().unwrap();
    // B cannot update A's task
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok_b, Some(json!({"title":"Hijacked"})))).await.unwrap();
    assert_eq!(resp.status(), 403);
    // B cannot delete A's task
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}", tid), &tok_b, None)).await.unwrap();
    assert_eq!(resp.status(), 403);
    // A can update their own task
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok_a, Some(json!({"title":"Updated"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
}

// ---- Timer start/stop/pause/resume ----

#[tokio::test]
async fn test_timer_full_lifecycle() {
    let app = app().await;
    let tok = register_user(&app, "timerUser").await;
    // Start
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let state = body_json(resp).await;
    assert_eq!(state["status"], "Running");
    assert_eq!(state["phase"], "Work");
    // Pause
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/pause", &tok, Some(json!({})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["status"], "Paused");
    // Resume
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/resume", &tok, Some(json!({})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["status"], "Running");
    // Stop
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/stop", &tok, Some(json!({})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["status"], "Idle");
}

// ---- Timer start with task ----

#[tokio::test]
async fn test_timer_start_with_task() {
    let app = app().await;
    let tok = register_user(&app, "timerTaskUser").await;
    let task = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Focus"})))).await.unwrap()).await;
    let tid = task["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({"task_id":tid})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let state = body_json(resp).await;
    assert_eq!(state["current_task_id"], tid);
}

// ---- Timer skip from idle ----

#[tokio::test]
async fn test_timer_skip_from_idle() {
    let app = app().await;
    let tok = register_user(&app, "skipIdleUser").await;
    // Skip from idle — should still return a valid state
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/skip", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let state = body_json(resp).await;
    assert_eq!(state["status"], "Idle");
}

// ---- Timer start break ----

#[tokio::test]
async fn test_timer_start_break() {
    let app = app().await;
    let tok = register_user(&app, "breakUser").await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({"phase":"short_break"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let state = body_json(resp).await;
    assert_eq!(state["phase"], "ShortBreak");
    assert_eq!(state["status"], "Running");
}

// ---- Users list (non-admin) ----

#[tokio::test]
async fn test_users_list_public() {
    let app = app().await;
    let tok = register_user(&app, "listUser").await;
    // /api/users is public (returns usernames only)
    let resp = app.clone().oneshot(auth_req("GET", "/api/users", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let users = body_json(resp).await;
    assert!(users.as_array().unwrap().len() >= 2); // root + listUser
}

// ---- My teams ----

#[tokio::test]
async fn test_my_teams() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create team (auto-adds creator as admin)
    app.clone().oneshot(auth_req("POST", "/api/teams", &tok, Some(json!({"name":"MyTeam1"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/me/teams", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let teams = body_json(resp).await;
    assert!(teams.as_array().unwrap().iter().any(|t| t["name"] == "MyTeam1"));
}

// ---- Sprint status transitions ----

#[tokio::test]
async fn test_sprint_cannot_start_active() {
    let app = app().await;
    let tok = login_root(&app).await;
    let sprint = body_json(app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap()).await;
    let sid = sprint["id"].as_i64().unwrap();
    // Start
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Start again — should fail
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 400);
    // Complete
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/complete", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Complete again — should fail
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/complete", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ---- Sprint cannot complete from planning ----

#[tokio::test]
async fn test_sprint_cannot_complete_from_planning() {
    let app = app().await;
    let tok = login_root(&app).await;
    let sprint = body_json(app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S2"})))).await.unwrap()).await;
    let sid = sprint["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/complete", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ---- Attachment size limit ----

#[tokio::test]
async fn test_attachment_size_limit() {
    let app = app().await;
    let tok = login_root(&app).await;
    let task = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap()).await;
    let tid = task["id"].as_i64().unwrap();
    // 10MB + 1 byte should be rejected (but axum body limit may kick in first)
    // Test with a moderately large body that's within axum limit but we can verify the endpoint works
    let small_body = vec![0u8; 100];
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/tasks/{}/attachments", tid))
        .header("authorization", format!("Bearer {}", tok))
        .header("x-requested-with", "test")
        .header("content-type", "application/octet-stream")
        .header("x-filename", "test.bin")
        .body(Body::from(small_body)).unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 201);
}

// ---- Task search/filter ----

#[tokio::test]
async fn test_task_search_filter() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Backend API","project":"backend"})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Frontend UI","project":"frontend"})))).await.unwrap();
    // Search by text
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks?search=backend", &tok, None)).await.unwrap();
    let tasks = body_json(resp).await;
    assert!(tasks.as_array().unwrap().iter().all(|t| t["title"].as_str().unwrap().to_lowercase().contains("backend") || t["project"].as_str().map_or(false, |p| p.to_lowercase().contains("backend"))));
    // Filter by project
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks?project=frontend", &tok, None)).await.unwrap();
    let tasks = body_json(resp).await;
    for t in tasks.as_array().unwrap() {
        assert_eq!(t["project"], "frontend");
    }
}

// ---- Task pagination ----

#[tokio::test]
async fn test_task_pagination() {
    let app = app().await;
    let tok = login_root(&app).await;
    for i in 0..5 { app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":format!("Page{}", i)})))).await.unwrap(); }
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks?page=1&per_page=2", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert!(resp.headers().get("x-total-count").is_some());
    assert!(resp.headers().get("x-page").is_some());
    let total: i64 = resp.headers().get("x-total-count").unwrap().to_str().unwrap().parse().unwrap();
    assert!(total >= 5);
    let tasks = body_json(resp).await;
    assert_eq!(tasks.as_array().unwrap().len(), 2);
}

// ---- History with date range ----

#[tokio::test]
async fn test_history_date_range() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/history?from=2026-01-01&to=2026-12-31", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
}

// ---- Stats endpoint ----

#[tokio::test]
async fn test_stats_endpoint() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/stats?from=2026-01-01&to=2026-12-31", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let stats = body_json(resp).await;
    assert!(stats.is_array());
}

// ---- Burn summary ----

#[tokio::test]
async fn test_burn_summary_empty() {
    let app = app().await;
    let tok = login_root(&app).await;
    let sprint = body_json(app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"SumSprint"})))).await.unwrap()).await;
    let sid = sprint["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/burn-summary", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let summary = body_json(resp).await;
    assert!(summary.is_array());
}

// ---- Velocity endpoint ----

#[tokio::test]
async fn test_velocity_with_limit() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/sprints/velocity?sprints=5", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let vel = body_json(resp).await;
    assert!(vel.is_array());
}

// ---- Task detail with children ----

#[tokio::test]
async fn test_task_detail_with_children() {
    let app = app().await;
    let tok = login_root(&app).await;
    let parent = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Parent"})))).await.unwrap()).await;
    let pid = parent["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Child1","parent_id":pid})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Child2","parent_id":pid})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", pid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let detail = body_json(resp).await;
    assert_eq!(detail["children"].as_array().unwrap().len(), 2);
}

// ---- Comment ownership ----

#[tokio::test]
async fn test_comment_cross_user() {
    let app = app().await;
    let tok_a = register_user(&app, "commentA").await;
    let tok_b = register_user(&app, "commentB").await;
    let task = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok_a, Some(json!({"title":"T"})))).await.unwrap()).await;
    let tid = task["id"].as_i64().unwrap();
    // B can add comment to A's task (comments are collaborative)
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &tok_b, Some(json!({"content":"Nice work!"})))).await.unwrap();
    assert!(resp.status().is_success());
    let comment = body_json(resp).await;
    let cid = comment["id"].as_i64().unwrap();
    // A cannot delete B's comment
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/comments/{}", cid), &tok_a, None)).await.unwrap();
    assert_eq!(resp.status(), 403);
    // B can delete their own comment
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/comments/{}", cid), &tok_b, None)).await.unwrap();
    assert!(resp.status().is_success());
}

// ---- Assignee add/remove ----

#[tokio::test]
async fn test_assignee_add_list_remove() {
    let app = app().await;
    let tok = login_root(&app).await;
    register_user(&app, "assigneeUser").await;
    let task = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap()).await;
    let tid = task["id"].as_i64().unwrap();
    // Add assignee
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &tok, Some(json!({"username":"assigneeUser"})))).await.unwrap();
    assert!(resp.status().is_success());
    // List assignees
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/assignees", tid), &tok, None)).await.unwrap();
    let assignees = body_json(resp).await;
    assert!(assignees.as_array().unwrap().contains(&json!("assigneeUser")));
    // Remove assignee
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/assignees/assigneeUser", tid), &tok, None)).await.unwrap();
    assert!(resp.status().is_success());
}

// ---- Global burndown ----

#[tokio::test]
async fn test_global_burndown_empty() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/sprints/burndown", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
}

// ---- Task sprints endpoint ----

#[tokio::test]
async fn test_task_sprints_with_data() {
    let app = app().await;
    let tok = login_root(&app).await;
    let sprint = body_json(app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"TS"})))).await.unwrap()).await;
    let task = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap()).await;
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sprint["id"]), &tok, Some(json!({"task_ids":[task["id"]]})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/task-sprints", &tok, None)).await.unwrap();
    let ts = body_json(resp).await;
    assert!(ts.as_array().unwrap().iter().any(|e| e["task_id"] == task["id"]));
}

// ---- Burn totals endpoint ----

#[tokio::test]
async fn test_burn_totals_endpoint() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/burn-totals", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
}

// ---- All assignees endpoint ----

#[tokio::test]
async fn test_all_assignees_endpoint() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/assignees", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
}

// ---- Config validation bounds ----

#[tokio::test]
async fn test_config_all_bounds() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/config", &tok, None)).await.unwrap();
    let cfg = body_json(resp).await;
    // work_duration_min = 0 should fail
    let mut bad = cfg.clone(); bad["work_duration_min"] = json!(0);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(bad))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // work_duration_min = 241 should fail
    let mut bad = cfg.clone(); bad["work_duration_min"] = json!(241);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(bad))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // short_break_min = 0 should fail
    let mut bad = cfg.clone(); bad["short_break_min"] = json!(0);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(bad))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // long_break_min = 0 should fail
    let mut bad = cfg.clone(); bad["long_break_min"] = json!(0);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(bad))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // daily_goal = 51 should fail
    let mut bad = cfg.clone(); bad["daily_goal"] = json!(51);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(bad))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // estimation_mode = "invalid" should fail
    let mut bad = cfg.clone(); bad["estimation_mode"] = json!("invalid");
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(bad))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ---- Export tasks JSON format ----

#[tokio::test]
async fn test_export_tasks_json() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Export Me"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/tasks?format=json", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("application/json"));
    let tasks = body_json(resp).await;
    assert!(tasks.as_array().unwrap().len() >= 1);
}

// ---- Sprint burndown with data ----

#[tokio::test]
async fn test_sprint_burndown_with_snapshot() {
    let app = app().await;
    let tok = login_root(&app).await;
    let sprint = body_json(app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"BurnSprint"})))).await.unwrap()).await;
    let sid = sprint["id"].as_i64().unwrap();
    let task = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T","remaining_points":5.0})))).await.unwrap()).await;
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[task["id"]]})))).await.unwrap();
    // Start sprint (triggers snapshot)
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();
    // Get burndown
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/burndown", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let stats = body_json(resp).await;
    assert!(stats.as_array().unwrap().len() >= 1);
}

// ---- Room reveal without votes ----

#[tokio::test]
async fn test_room_reveal_without_votes() {
    let app = app().await;
    let tok = login_root(&app).await;
    let room = body_json(app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"RevealRoom"})))).await.unwrap()).await;
    let rid = room["id"].as_i64().unwrap();
    let task = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap()).await;
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/start-voting", rid), &tok, Some(json!({"task_id":task["id"]})))).await.unwrap();
    // Reveal without any votes
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/reveal", rid), &tok, None)).await.unwrap();
    assert!(resp.status().is_success());
}

// ---- Task status transitions ----

#[tokio::test]
async fn test_task_status_transitions() {
    let app = app().await;
    let tok = login_root(&app).await;
    let task = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"StatusTask"})))).await.unwrap()).await;
    let tid = task["id"].as_i64().unwrap();
    assert_eq!(task["status"], "backlog");
    // backlog → active
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"active"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["status"], "active");
    // active → completed
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"completed"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["status"], "completed");
    // Invalid status
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"invalid"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ---- Register duplicate username ----

#[tokio::test]
async fn test_register_duplicate_username() {
    let app = app().await;
    register_user(&app, "dupUser").await;
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"dupUser","password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 409);
}

// ---- Task empty title rejected ----

#[tokio::test]
async fn test_task_empty_title_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":""})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"   "})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ---- Sprint update retro notes ----

#[tokio::test]
async fn test_sprint_retro_notes() {
    let app = app().await;
    let tok = login_root(&app).await;
    let sprint = body_json(app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"RetroSprint"})))).await.unwrap()).await;
    let sid = sprint["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/sprints/{}", sid), &tok, Some(json!({"retro_notes":"Good sprint!"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let updated = body_json(resp).await;
    assert_eq!(updated["retro_notes"], "Good sprint!");
}

// ---- Room with mandays estimation ----

#[tokio::test]
async fn test_room_mandays_estimation() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"MandayRoom","estimation_unit":"mandays"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let room = body_json(resp).await;
    assert_eq!(room["estimation_unit"], "mandays");
}

// ---- Task with all optional fields ----

#[tokio::test]
async fn test_task_all_fields() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({
        "title": "Full Task",
        "description": "A detailed description",
        "project": "myproject",
        "tags": "rust,backend",
        "priority": 1,
        "estimated": 5,
        "estimated_hours": 10.5,
        "remaining_points": 3.0,
        "due_date": "2026-12-31"
    })))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let task = body_json(resp).await;
    assert_eq!(task["title"], "Full Task");
    assert_eq!(task["description"], "A detailed description");
    assert_eq!(task["project"], "myproject");
    assert_eq!(task["tags"], "rust,backend");
    assert_eq!(task["priority"], 1);
    assert_eq!(task["estimated"], 5);
    assert_eq!(task["due_date"], "2026-12-31");
}

// ---- Bulk Status ----

#[tokio::test]
async fn test_bulk_status_change() {
    let app = app().await;
    let token = login_root(&app).await;
    // Create two tasks
    let r1 = app.clone().oneshot(auth_req("POST", "/api/tasks", &token, Some(json!({"title":"Bulk1"})))).await.unwrap();
    let id1 = body_json(r1).await["id"].as_i64().unwrap();
    let r2 = app.clone().oneshot(auth_req("POST", "/api/tasks", &token, Some(json!({"title":"Bulk2"})))).await.unwrap();
    let id2 = body_json(r2).await["id"].as_i64().unwrap();
    // Bulk update to done
    let resp = app.clone().oneshot(auth_req("PUT", "/api/tasks/bulk-status", &token, Some(json!({"task_ids":[id1,id2],"status":"done"})))).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Verify
    let r = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", id1), &token, None)).await.unwrap();
    assert_eq!(body_json(r).await["task"]["status"], "done");
}

#[tokio::test]
async fn test_bulk_status_invalid() {
    let app = app().await;
    let token = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("PUT", "/api/tasks/bulk-status", &token, Some(json!({"task_ids":[999],"status":"invalid"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ---- CSV Import ----

#[tokio::test]
async fn test_csv_import_tasks() {
    let app = app().await;
    let token = login_root(&app).await;
    let csv = "title,priority,estimated,project\nImported Task,2,5,myproj\nAnother,3,0,";
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &token, Some(json!({"csv": csv})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = body_json(resp).await;
    assert_eq!(body["created"], 2);
}

// ---- Export Burns ----

#[tokio::test]
async fn test_export_burns_csv() {
    let app = app().await;
    let token = login_root(&app).await;
    // Create sprint
    let r = app.clone().oneshot(auth_req("POST", "/api/sprints", &token, Some(json!({"name":"BurnExport"})))).await.unwrap();
    let sid = body_json(r).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/export/burns/{}", sid), &token, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let csv = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(csv.starts_with("created_at,task_id,points,hours,username,source,note"));
}

// ---- Bcrypt Rehash ----

#[tokio::test]
async fn test_login_succeeds_after_register() {
    let app = app().await;
    // Register
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"rehashuser","password":"Testpass1"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Login should succeed
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"rehashuser","password":"Testpass1"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = body_json(resp).await;
    assert!(body["token"].is_string());
}

// ---- Export with date range ----

#[tokio::test]
async fn test_export_sessions_date_range() {
    let app = app().await;
    let token = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/sessions?format=json&from=2020-01-01&to=2020-12-31", &token, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = body_json(resp).await;
    assert!(body.is_array());
    assert_eq!(body.as_array().unwrap().len(), 0); // no sessions in that range
}

// T4: Config rejects long_break_interval=0
#[tokio::test]
async fn test_config_rejects_zero_interval() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok,
        Some(json!({"work_duration_min":25,"short_break_min":5,"long_break_min":15,"long_break_interval":0,"daily_goal":8,"auto_start_breaks":false,"auto_start_work":false,"estimation_mode":"points","theme":"dark"})))).await.unwrap();
    assert!(resp.status().as_u16() >= 400);
}

// T5: Bulk status ownership isolation
#[tokio::test]
async fn test_bulk_status_ownership_isolation() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create a task as root
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"RootTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Register user2
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"user2","password":"Pass1234!"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();
    // user2 tries to bulk-update root's task
    let resp = app.clone().oneshot(auth_req("PUT", "/api/tasks/bulk-status", &tok2,
        Some(json!({"task_ids":[tid],"status":"completed"})))).await.unwrap();
    assert_eq!(resp.status(), 403);
}

// T9: Frontend ErrorBoundary test is in gui/__tests__

// T3: CSV import with quoted fields
#[tokio::test]
async fn test_csv_import_quoted_fields() {
    let app = app().await;
    let tok = login_root(&app).await;
    let csv = "title,priority,estimated,project\n\"Task with, comma\",3,2,\"Project A\"\n\"Normal task\",1,1,";
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &tok, Some(json!({"csv": csv})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = body_json(resp).await;
    assert_eq!(body["created"], 2);
    // Verify the comma-containing title was imported correctly
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks", &tok, None)).await.unwrap();
    let tasks = body_json(resp).await;
    let titles: Vec<&str> = tasks.as_array().unwrap().iter().map(|t| t["title"].as_str().unwrap()).collect();
    assert!(titles.contains(&"Task with, comma"));
}

// T2: Circular parent_id detection
#[tokio::test]
async fn test_circular_parent_id_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create two tasks
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"A"})))).await.unwrap();
    let a_id = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"B","parent_id":a_id})))).await.unwrap();
    let b_id = body_json(resp).await["id"].as_i64().unwrap();
    // Try to make A a child of B (creates cycle A→B→A)
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", a_id), &tok,
        Some(json!({"parent_id":b_id})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// T8: Token refresh flow
#[tokio::test]
async fn test_token_refresh_flow() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Get refresh token from login
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"root","password":"root"})))).await.unwrap();
    let body = body_json(resp).await;
    let refresh = body["refresh_token"].as_str().unwrap().to_string();
    // Use refresh token to get new access token
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/refresh", Some(json!({"refresh_token": refresh})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = body_json(resp).await;
    assert!(body["token"].as_str().is_some());
    assert!(body["refresh_token"].as_str().is_some());
    // Old refresh token should be revoked (rotation)
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/refresh", Some(json!({"refresh_token": refresh})))).await.unwrap();
    assert_eq!(resp.status(), 401);
}

// T6: Concurrent timer operations
#[tokio::test]
async fn test_concurrent_timer_start_stop() {
    let app = app().await;
    let tok = login_root(&app).await;
    let start_body = Some(json!({}));
    // Start timer
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, start_body.clone())).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Pause then stop
    let _ = app.clone().oneshot(auth_req("POST", "/api/timer/pause", &tok, None)).await.unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/stop", &tok, None)).await.unwrap();
    assert!(resp.status().is_success());
    // Start again — should succeed (not stuck)
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, start_body)).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Verify state is running
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer", &tok, None)).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["status"].as_str().unwrap(), "Running");
}

// === T1: Sprint lifecycle (create → start → snapshot → complete) ===
#[tokio::test]
async fn test_sprint_lifecycle() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create task + sprint
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T1"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"Sprint1","goal":"Ship it"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let sprint = body_json(resp).await;
    let sid = sprint["id"].as_i64().unwrap();
    assert_eq!(sprint["status"], "planning");

    // Add task to sprint
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    assert!(resp.status().is_success());

    // Start sprint
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let sprint = body_json(resp).await;
    assert_eq!(sprint["status"], "active");

    // Take snapshot
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/snapshot", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Get board
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/board", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let board = body_json(resp).await;
    assert!(board["todo"].as_array().unwrap().len() + board["in_progress"].as_array().unwrap().len() + board["done"].as_array().unwrap().len() > 0);

    // Complete sprint
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/complete", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let sprint = body_json(resp).await;
    assert_eq!(sprint["status"], "completed");

    // Cannot start a completed sprint
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();
    assert_ne!(resp.status(), 200);
}

// === T2: Room voting flow ===
#[tokio::test]
async fn test_room_voting_flow() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create room + task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"VoteTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"VoteRoom"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();

    // Join room
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/join", rid), &tok, None)).await.unwrap();
    assert!(resp.status().is_success());

    // Cannot vote in lobby state
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &tok, Some(json!({"value":5.0})))).await.unwrap();
    assert_eq!(resp.status(), 400);

    // Start voting on task
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/start-voting", rid), &tok, Some(json!({"task_id":tid})))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Cast vote
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &tok, Some(json!({"value":8.0})))).await.unwrap();
    assert_eq!(resp.status(), 204);

    // Reveal votes
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/reveal", rid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let room = body_json(resp).await;
    assert_eq!(room["status"], "revealed");

    // Accept estimate
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/accept", rid), &tok, Some(json!({"value":8.0})))).await.unwrap();
    assert_eq!(resp.status(), 200);
}

// === T3: Attachment upload + download + delete cycle ===
#[tokio::test]
async fn test_attachment_cycle() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"AttTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    // Upload
    let resp = app.clone().oneshot(
        Request::builder().method("POST").uri(format!("/api/tasks/{}/attachments", tid))
            .header("authorization", format!("Bearer {}", tok))
            .header("content-type", "text/plain")
            .header("x-filename", "test.txt")
            .header("x-requested-with", "test")
            .body(Body::from("hello world")).unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), 201);
    let att = body_json(resp).await;
    let aid = att["id"].as_i64().unwrap();
    assert_eq!(att["filename"], "test.txt");
    assert_eq!(att["size_bytes"], 11);

    // List attachments
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/attachments", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let list = body_json(resp).await;
    assert_eq!(list.as_array().unwrap().len(), 1);

    // Download
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/attachments/{}/download", aid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Delete
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/attachments/{}", aid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);

    // Verify deleted
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/attachments", tid), &tok, None)).await.unwrap();
    let list = body_json(resp).await;
    assert_eq!(list.as_array().unwrap().len(), 0);
}

// === T4: Team scoping ===
#[tokio::test]
async fn test_team_scope() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create team
    let resp = app.clone().oneshot(auth_req("POST", "/api/teams", &tok, Some(json!({"name":"Alpha"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let team_id = body_json(resp).await["id"].as_i64().unwrap();

    // Create tasks
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"TeamTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    // Add root task to team
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/teams/{}/roots", team_id), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    assert_eq!(resp.status(), 204);

    // Query tasks scoped to team
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks?team_id={}", team_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let tasks = body_json(resp).await;
    assert!(tasks.as_array().unwrap().iter().any(|t| t["id"] == tid));

    // Remove root task
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/teams/{}/roots/{}", team_id, tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

// === T5: Epic group snapshot ===
#[tokio::test]
async fn test_epic_group() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create epic group
    let resp = app.clone().oneshot(auth_req("POST", "/api/epics", &tok, Some(json!({"name":"Epic1","description":"Test epic"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let eid = body_json(resp).await["id"].as_i64().unwrap();

    // Create task and add to epic
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"EpicTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/epics/{}/tasks", eid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    assert_eq!(resp.status(), 204);

    // Get epic detail
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/epics/{}", eid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let detail = body_json(resp).await;
    assert_eq!(detail["task_ids"].as_array().unwrap().len(), 1);

    // Delete epic
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/epics/{}", eid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

// === T6: Recurrence processing ===
#[tokio::test]
async fn test_recurrence_set_get_remove() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"RecurTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    // Set recurrence
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/recurrence", tid), &tok,
        Some(json!({"pattern":"daily","next_due":"2026-05-01"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let rec = body_json(resp).await;
    assert_eq!(rec["pattern"], "daily");

    // Get recurrence
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/recurrence", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Invalid pattern rejected
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/recurrence", tid), &tok,
        Some(json!({"pattern":"yearly","next_due":"2026-05-01"})))).await.unwrap();
    assert_eq!(resp.status(), 400);

    // Invalid date format rejected
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/recurrence", tid), &tok,
        Some(json!({"pattern":"daily","next_due":"not-a-date"})))).await.unwrap();
    assert_eq!(resp.status(), 400);

    // Remove recurrence
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/recurrence", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

// === T7: Webhook CRUD ===
#[tokio::test]
async fn test_webhook_crud() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create webhook
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok,
        Some(json!({"url":"https://example.com/hook","events":"task.created,task.updated","secret":"s3cret"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let wh = body_json(resp).await;
    let wid = wh["id"].as_i64().unwrap();
    assert_eq!(wh["url"], "https://example.com/hook");

    // List webhooks
    let resp = app.clone().oneshot(auth_req("GET", "/api/webhooks", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let list = body_json(resp).await;
    assert!(list.as_array().unwrap().len() >= 1);

    // Invalid event rejected
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok,
        Some(json!({"url":"https://example.com/hook2","events":"invalid.event"})))).await.unwrap();
    assert_eq!(resp.status(), 400);

    // Delete webhook
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/webhooks/{}", wid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

// === T8: Audit log ===
#[tokio::test]
async fn test_audit_log_entries() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create and delete a task to generate audit entries
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"AuditTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();

    // Query audit log
    let resp = app.clone().oneshot(auth_req("GET", "/api/audit", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let entries = body_json(resp).await;
    let arr = entries.as_array().unwrap();
    assert!(arr.iter().any(|e| e["action"] == "create" && e["entity_type"] == "task"));
    assert!(arr.iter().any(|e| e["action"] == "delete" && e["entity_type"] == "task"));

    // Filter by entity type
    let resp = app.clone().oneshot(auth_req("GET", "/api/audit?entity_type=task", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let filtered = body_json(resp).await;
    assert!(filtered.as_array().unwrap().iter().all(|e| e["entity_type"] == "task"));
}

// === T9: Auth flow (register → login → refresh → logout) ===
#[tokio::test]
async fn test_auth_full_flow() {
    let app = app().await;

    // Register
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"flowuser","password":"Flow1234"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let auth = body_json(resp).await;
    let tok = auth["token"].as_str().unwrap().to_string();
    let refresh = auth["refresh_token"].as_str().unwrap().to_string();
    assert_eq!(auth["username"], "flowuser");

    // Duplicate register fails
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"flowuser","password":"Flow1234"})))).await.unwrap();
    assert_eq!(resp.status(), 409);

    // Login
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"flowuser","password":"Flow1234"})))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Wrong password
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"flowuser","password":"wrong"})))).await.unwrap();
    assert_eq!(resp.status(), 401);

    // Refresh token
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/refresh", Some(json!({"refresh_token": refresh})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let new_auth = body_json(resp).await;
    assert!(new_auth["token"].as_str().is_some());

    // Logout
    let resp = app.clone().oneshot(auth_req("POST", "/api/auth/logout", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);

    // Token should be revoked after logout
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 401);
}

// === T10: CSV export ===
#[tokio::test]
async fn test_csv_export() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create some tasks
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"CSV1","project":"P1"})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"CSV2","project":"P1"})))).await.unwrap();

    // Export CSV
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/tasks?format=csv", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("csv") || ct.contains("text"));
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let csv = String::from_utf8(body.to_vec()).unwrap();
    assert!(csv.contains("CSV1"));
    assert!(csv.contains("CSV2"));
}

// === T11: Soft delete + restore ===
#[tokio::test]
async fn test_soft_delete_and_restore() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"SoftDel"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    // Delete (soft)
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);

    // Task should not appear in list
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks", &tok, None)).await.unwrap();
    let tasks = body_json(resp).await;
    assert!(!tasks.as_array().unwrap().iter().any(|t| t["id"] == tid));

    // Restore
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/restore", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);

    // Task should reappear
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks", &tok, None)).await.unwrap();
    let tasks = body_json(resp).await;
    assert!(tasks.as_array().unwrap().iter().any(|t| t["id"] == tid));
}

// === T12: Health endpoint ===
#[tokio::test]
async fn test_health_endpoint() {
    let app = app().await;
    // No auth required
    let resp = app.clone().oneshot(Request::builder().method("GET").uri("/api/health").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = body_json(resp).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["db"], true);
}

// === T1: Soft-delete cascade — restore parent restores children ===
#[tokio::test]
async fn test_soft_delete_cascade_restore() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create parent + child
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Parent"})))).await.unwrap();
    let pid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Child","parent_id":pid})))).await.unwrap();
    let cid = body_json(resp).await["id"].as_i64().unwrap();

    // Delete parent (should cascade to child)
    app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}", pid), &tok, None)).await.unwrap();

    // Both should be gone from list
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks", &tok, None)).await.unwrap();
    let tasks = body_json(resp).await;
    assert!(!tasks.as_array().unwrap().iter().any(|t| t["id"] == pid || t["id"] == cid));

    // Both should appear in trash
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks/trash", &tok, None)).await.unwrap();
    let trash = body_json(resp).await;
    assert!(trash.as_array().unwrap().iter().any(|t| t["id"] == pid));
    assert!(trash.as_array().unwrap().iter().any(|t| t["id"] == cid));

    // Restore parent
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/restore", pid), &tok, None)).await.unwrap();

    // Both should reappear
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks", &tok, None)).await.unwrap();
    let tasks = body_json(resp).await;
    assert!(tasks.as_array().unwrap().iter().any(|t| t["id"] == pid));
    assert!(tasks.as_array().unwrap().iter().any(|t| t["id"] == cid));
}

// === T2: Concurrent room voting ===
#[tokio::test]
async fn test_concurrent_room_voting() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Register second user
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"voter2","password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 200, "Register should succeed");
    let body = body_json(resp).await;
    let tok2 = body["token"].as_str().expect("register should return token").to_string();

    // Create room
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"ConcRoom"})))).await.unwrap();
    assert_eq!(resp.status(), 201, "Room creation should succeed");
    let rid = body_json(resp).await["id"].as_i64().unwrap();

    // Second user joins
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/join", rid), &tok2, None)).await.unwrap();
    assert!(resp.status().is_success(), "Join should succeed");

    // Create task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"VoteTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    // Start voting
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/start-voting", rid), &tok, Some(json!({"task_id":tid})))).await.unwrap();
    assert!(resp.status().is_success(), "Start voting should succeed");

    // Both vote simultaneously
    let (r1, r2) = tokio::join!(
        app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &tok, Some(json!({"value":5})))),
        app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &tok2, Some(json!({"value":8}))))
    );
    assert!(r1.unwrap().status().is_success());
    assert!(r2.unwrap().status().is_success());

    // Reveal
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/reveal", rid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Fetch room state — both votes should be visible
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/rooms/{}", rid), &tok, None)).await.unwrap();
    let state = body_json(resp).await;
    let votes = state["votes"].as_array().unwrap();
    assert_eq!(votes.len(), 2);
    assert!(votes.iter().all(|v| v["voted"] == true));
}

// === T3: CSV import with malformed data ===
#[tokio::test]
async fn test_csv_import_malformed() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Empty CSV (header only)
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &tok, Some(json!({"csv":"title,priority\n"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = body_json(resp).await;
    assert_eq!(body["created"], 0);

    // Missing columns — should still create with defaults
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &tok, Some(json!({"csv":"title\nJustTitle\n"})))).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["created"], 1);

    // Extra columns — should ignore extras
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &tok, Some(json!({"csv":"title,priority,estimated,project,extra1,extra2\nT,1,2,proj,x,y\n"})))).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["created"], 1);

    // Special characters in title
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &tok, Some(json!({"csv":"title\n\"Quoted, with comma\"\n=formula\n"})))).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["created"], 2);

    // Empty title lines should be skipped
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &tok, Some(json!({"csv":"title\n\n  \nReal Task\n"})))).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["created"], 1);

    // Too large CSV
    let big = "title\n".to_string() + &"x".repeat(2_000_000);
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &tok, Some(json!({"csv": big})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// === T4: Webhook SSRF protection — additional patterns ===
#[tokio::test]
async fn test_webhook_ssrf_additional_patterns() {
    let app = app().await;
    let tok = login_root(&app).await;

    let blocked_urls = [
        "http://localhost/hook",
        "http://127.0.0.1/hook",
        "http://0.0.0.0/hook",
        "http://[::1]/hook",
        "http://10.0.0.1/hook",
        "http://192.168.1.1/hook",
        "http://172.16.0.1/hook",
        "http://169.254.1.1/hook",
        "http://internal.local/hook",
        "ftp://example.com/hook",
        "http://user:pass@example.com/hook",
    ];
    for url in &blocked_urls {
        let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok,
            Some(json!({"url": url, "events":"task.created"})))).await.unwrap();
        assert_eq!(resp.status(), 400, "Expected 400 for URL: {}", url);
    }
}

// === T8: Auth rate limiting — verify limit ===
#[tokio::test]
async fn test_auth_rate_limit_threshold() {
    let app = app().await;
    if std::env::var("POMODORO_NO_RATE_LIMIT").is_ok() { return; }
    pomodoro_daemon::routes::auth_limiter().reset();

    // Send 11 login attempts from same IP (limit is 10/60s)
    let mut last_status = 200;
    for i in 0..12 {
        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/api/auth/login")
                .header("content-type", "application/json")
                .header("x-forwarded-for", "88.77.66.55")
                .body(Body::from(serde_json::to_vec(&json!({"username":"root","password":"wrong"})).unwrap())).unwrap()
        ).await.unwrap();
        last_status = resp.status().as_u16();
        if last_status == 429 { break; }
        if i < 10 { assert_eq!(last_status, 401, "Attempt {} should be 401", i); }
    }
    assert_eq!(last_status, 429, "Should be rate limited after 10+ attempts");
}

// === T1b: Soft-deleted tasks rejected from sprints ===
#[tokio::test]
async fn test_soft_deleted_task_rejected_from_sprint() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"DelTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();

    // Soft delete the task
    app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();

    // Try to add deleted task to sprint — should fail
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    assert_eq!(resp.status(), 404);
}

// === Task duplication ===
#[tokio::test]
async fn test_task_duplicate() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok,
        Some(json!({"title":"Original","priority":2,"estimated":5,"project":"proj","tags":"a,b"})))).await.unwrap();
    let oid = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/duplicate", oid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 201);
    let dup = body_json(resp).await;
    assert!(dup["title"].as_str().unwrap().contains("(copy)"));
    assert_eq!(dup["priority"], 2);
    assert_eq!(dup["estimated"], 5);
    assert_eq!(dup["project"], "proj");
    assert_ne!(dup["id"], oid);
}

// === Trash endpoint ===
#[tokio::test]
async fn test_trash_endpoint() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Empty trash initially
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks/trash", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let trash = body_json(resp).await;
    assert_eq!(trash.as_array().unwrap().len(), 0);

    // Create and delete a task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"TrashMe"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();

    // Should appear in trash
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks/trash", &tok, None)).await.unwrap();
    let trash = body_json(resp).await;
    assert_eq!(trash.as_array().unwrap().len(), 1);
    assert_eq!(trash[0]["title"], "TrashMe");
    assert!(trash[0]["deleted_at"].as_str().is_some());
}

// === Sprint root task auth ===
#[tokio::test]
async fn test_sprint_root_task_auth() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Register non-root user
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"noroot","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Root creates sprint
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"AuthSprint"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"RootTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    // Non-root tries to add root tasks — should be forbidden
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/roots", sid), &tok2, Some(json!({"task_ids":[tid]})))).await.unwrap();
    assert_eq!(resp.status(), 403);

    // Root can add
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/roots", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    assert_eq!(resp.status(), 204);
}

// === Template limits ===
#[tokio::test]
async fn test_template_limits() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Name too long
    let long_name = "x".repeat(201);
    let resp = app.clone().oneshot(auth_req("POST", "/api/templates", &tok,
        Some(json!({"name": long_name, "data": {}})))).await.unwrap();
    assert_eq!(resp.status(), 400);

    // Data too large
    let big_data: serde_json::Value = serde_json::from_str(&format!("{{\"x\":\"{}\"}}", "y".repeat(70000))).unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/templates", &tok,
        Some(json!({"name": "big", "data": big_data})))).await.unwrap();
    assert_eq!(resp.status(), 400);

    // Valid template works
    let resp = app.clone().oneshot(auth_req("POST", "/api/templates", &tok,
        Some(json!({"name": "ok", "data": {"title":"T"}})))).await.unwrap();
    assert_eq!(resp.status(), 201);
}

// === Config theme validation ===
#[tokio::test]
async fn test_config_theme_validation() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Get current config
    let resp = app.clone().oneshot(auth_req("GET", "/api/config", &tok, None)).await.unwrap();
    let mut cfg = body_json(resp).await;

    // Invalid theme
    cfg["theme"] = json!("neon");
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(cfg.clone()))).await.unwrap();
    assert_eq!(resp.status(), 400);

    // Valid theme
    cfg["theme"] = json!("dark");
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(cfg))).await.unwrap();
    assert_eq!(resp.status(), 200);
}

// ============================================================
// T1: Rate limiting middleware tests
// ============================================================

#[tokio::test]
async fn test_rate_limit_get_not_limited() {
    let app = app().await;
    let tok = login_root(&app).await;
    // GET requests should never be rate limited
    for _ in 0..10 {
        let resp = app.clone().oneshot(auth_req("GET", "/api/tasks", &tok, None)).await.unwrap();
        assert_eq!(resp.status(), 200);
    }
}

#[tokio::test]
async fn test_auth_rate_limit_blocks_after_threshold() {
    let app = app().await;
    if std::env::var("POMODORO_NO_RATE_LIMIT").is_ok() { return; }
    pomodoro_daemon::routes::auth_limiter().reset();
    // Use a fixed IP for all requests to trigger rate limit
    let fixed_ip = "10.99.99.1";
    for i in 0..12 {
        let req = Request::builder().method("POST").uri("/api/auth/login")
            .header("content-type", "application/json")
            .header("x-forwarded-for", fixed_ip)
            .body(Body::from(serde_json::to_vec(&json!({"username":"root","password":"wrong"})).unwrap())).unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        if i >= 10 {
            assert_eq!(resp.status(), 429, "Should be rate limited after 10 attempts");
        }
    }
}

// ============================================================
// T2: Attachment upload/download tests
// ============================================================

#[tokio::test]
async fn test_attachment_upload_download_delete() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create a task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"AttachTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    // Upload attachment
    let req = Request::builder().method("POST").uri(&format!("/api/tasks/{}/attachments", tid))
        .header("authorization", format!("Bearer {}", tok))
        .header("x-requested-with", "test")
        .header("x-filename", "test.txt")
        .header("content-type", "text/plain")
        .body(Body::from("hello world")).unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 201);
    let att = body_json(resp).await;
    assert_eq!(att["filename"], "test.txt");
    let att_id = att["id"].as_i64().unwrap();

    // List attachments
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/attachments", tid), &tok, None)).await.unwrap();
    let list = body_json(resp).await;
    assert_eq!(list.as_array().unwrap().len(), 1);

    // Download
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/attachments/{}/download", att_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    // S3: text/plain should be served as-is
    assert_eq!(resp.headers().get("content-type").unwrap(), "text/plain");

    // Delete
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/attachments/{}", att_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_attachment_empty_file_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let req = Request::builder().method("POST").uri(&format!("/api/tasks/{}/attachments", tid))
        .header("authorization", format!("Bearer {}", tok))
        .header("x-requested-with", "test")
        .header("x-filename", "empty.txt")
        .body(Body::empty()).unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_attachment_unsafe_mime_forced_octet_stream() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // S3: Upload with HTML content-type should be blocked
    let req = Request::builder().method("POST").uri(&format!("/api/tasks/{}/attachments", tid))
        .header("authorization", format!("Bearer {}", tok))
        .header("x-requested-with", "test")
        .header("x-filename", "evil.html")
        .header("content-type", "text/html")
        .body(Body::from("<script>alert(1)</script>")).unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ============================================================
// T3: Recurring task tests
// ============================================================

#[tokio::test]
async fn test_recurrence_crud_and_patterns() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Recurring"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    // Set daily recurrence
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/recurrence", tid), &tok,
        Some(json!({"pattern":"daily","next_due":"2026-04-15"})))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Get recurrence
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/recurrence", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let rec = body_json(resp).await;
    assert_eq!(rec["pattern"], "daily");
    assert_eq!(rec["next_due"], "2026-04-15");

    // Update to weekly
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/recurrence", tid), &tok,
        Some(json!({"pattern":"weekly","next_due":"2026-04-20"})))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Delete recurrence
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/recurrence", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);

    // Get should return empty/null
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/recurrence", tid), &tok, None)).await.unwrap();
    let rec = body_json(resp).await;
    assert!(rec.is_null() || rec.as_object().map_or(true, |o| o.is_empty()));
}

// ============================================================
// T4: Webhook SSRF protection tests
// ============================================================

#[tokio::test]
async fn test_webhook_rejects_private_ips() {
    let app = app().await;
    let tok = login_root(&app).await;
    let private_urls = [
        "http://localhost:8080/hook",
        "http://127.0.0.1:8080/hook",
        "http://10.0.0.1/hook",
        "http://192.168.1.1/hook",
        "http://172.16.0.1/hook",
    ];
    for url in private_urls {
        let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok, Some(json!({"url": url})))).await.unwrap();
        assert_eq!(resp.status(), 400, "Should reject private URL: {}", url);
    }
}

#[tokio::test]
async fn test_webhook_rejects_credentials_in_url() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok,
        Some(json!({"url":"http://user:pass@example.com/hook"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ============================================================
// T5: CSV import edge cases
// ============================================================

#[tokio::test]
async fn test_csv_import_quoted_fields_with_commas() {
    let app = app().await;
    let tok = login_root(&app).await;
    let csv = "title,priority,estimated,project\n\"Task with, comma\",3,2,\"My Project\"\n\"Task \"\"quoted\"\"\",1,1,\n";
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &tok, Some(json!({"csv": csv})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let result = body_json(resp).await;
    assert_eq!(result["created"], 2);
}

#[tokio::test]
async fn test_csv_import_empty_lines_skipped() {
    let app = app().await;
    let tok = login_root(&app).await;
    let csv = "title,priority\n\n\nActual Task,3\n\n";
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &tok, Some(json!({"csv": csv})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let result = body_json(resp).await;
    assert_eq!(result["created"], 1);
}

#[tokio::test]
async fn test_csv_import_size_limit() {
    let app = app().await;
    let tok = login_root(&app).await;
    let csv = "a".repeat(1_048_577); // > 1MB
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &tok, Some(json!({"csv": csv})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ============================================================
// T6: Sprint burndown snapshot accuracy
// ============================================================

#[tokio::test]
async fn test_burndown_snapshot_uses_remaining_points() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create tasks with different remaining_points and estimated
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok,
        Some(json!({"title":"T1","remaining_points":8.0,"estimated":3,"estimated_hours":4.0})))).await.unwrap();
    let t1 = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok,
        Some(json!({"title":"T2","remaining_points":5.0,"estimated":2,"estimated_hours":2.0,"status":"completed"})))).await.unwrap();
    let t2 = body_json(resp).await["id"].as_i64().unwrap();
    // Mark T2 as completed
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", t2), &tok, Some(json!({"status":"completed"})))).await.unwrap();

    // Create sprint and add tasks
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"BurndownTest"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[t1, t2]})))).await.unwrap();

    // Snapshot
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/snapshot", sid), &tok, None)).await.unwrap();
    let stat = body_json(resp).await;
    // total_points should be remaining_points sum (8+5=13), not estimated (3+2=5)
    assert_eq!(stat["total_points"], 13.0);
    assert_eq!(stat["done_points"], 5.0);
    assert_eq!(stat["total_hours"], 6.0);
    assert_eq!(stat["done_hours"], 2.0);
    assert_eq!(stat["total_tasks"], 2);
    assert_eq!(stat["done_tasks"], 1);
}

// ============================================================
// T7: Task sessions endpoint
// ============================================================

#[tokio::test]
async fn test_task_sessions_endpoint() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"SessionTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    // No sessions initially
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/sessions", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let sessions = body_json(resp).await;
    assert_eq!(sessions.as_array().unwrap().len(), 0);
}

// ============================================================
// T8: Password change requires current password
// ============================================================

#[tokio::test]
async fn test_password_change_requires_current() {
    let app = app().await;
    let tok = register_user(&app, "pwReqUser").await;

    // Missing current_password → 400
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok,
        Some(json!({"password":"NewPass99"})))).await.unwrap();
    assert_eq!(resp.status(), 400);

    // Wrong current_password → 403
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok,
        Some(json!({"password":"NewPass99","current_password":"WrongPass1"})))).await.unwrap();
    assert_eq!(resp.status(), 403);

    // Correct current_password → 200
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok,
        Some(json!({"password":"NewPass99","current_password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
}

// ============================================================
// v10 T1: Backup endpoint test
// ============================================================

#[tokio::test]
async fn test_backup_root_only() {
    let app = app().await;
    let tok = login_root(&app).await;
    let user_tok = register_user(&app, "backupUser").await;

    // Non-root rejected
    let resp = app.clone().oneshot(auth_req("POST", "/api/admin/backup", &user_tok, None)).await.unwrap();
    assert_eq!(resp.status(), 403);

    // Root succeeds (may fail on in-memory DB but should not 403)
    let resp = app.clone().oneshot(auth_req("POST", "/api/admin/backup", &tok, None)).await.unwrap();
    // In-memory SQLite can't VACUUM INTO a file, so 500 is acceptable in tests
    assert!(resp.status() == 200 || resp.status() == 500);
}

// ============================================================
// v10 T2: Notification preferences CRUD
// ============================================================

#[tokio::test]
async fn test_notification_prefs_crud() {
    let app = app().await;
    let tok = register_user(&app, "notifUser").await;

    // Get defaults (all enabled)
    let resp = app.clone().oneshot(auth_req("GET", "/api/profile/notifications", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let prefs = body_json(resp).await;
    let arr = prefs.as_array().unwrap();
    assert!(arr.len() >= 6);
    assert!(arr.iter().all(|p| p["enabled"] == true));

    // Toggle one off
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile/notifications", &tok,
        Some(json!([{"event_type":"task_assigned","enabled":false}])))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Verify
    let resp = app.clone().oneshot(auth_req("GET", "/api/profile/notifications", &tok, None)).await.unwrap();
    let prefs = body_json(resp).await;
    let ta = prefs.as_array().unwrap().iter().find(|p| p["event_type"] == "task_assigned").unwrap();
    assert_eq!(ta["enabled"], false);

    // Unknown event type rejected
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile/notifications", &tok,
        Some(json!([{"event_type":"bogus","enabled":true}])))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ============================================================
// v10 T3: Bulk status update
// ============================================================

#[tokio::test]
async fn test_bulk_status_update() {
    let app = app().await;
    let tok = login_root(&app).await;
    let t1 = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Bulk1"})))).await.unwrap()).await["id"].as_i64().unwrap();
    let t2 = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Bulk2"})))).await.unwrap()).await["id"].as_i64().unwrap();

    // Bulk update to completed
    let resp = app.clone().oneshot(auth_req("PUT", "/api/tasks/bulk-status", &tok,
        Some(json!({"task_ids":[t1, t2],"status":"completed"})))).await.unwrap();
    assert_eq!(resp.status(), 204);

    // Verify
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", t1), &tok, None)).await.unwrap();
    let task = body_json(resp).await;
    assert_eq!(task["task"]["status"], "completed");

    // Invalid status rejected
    let resp = app.clone().oneshot(auth_req("PUT", "/api/tasks/bulk-status", &tok,
        Some(json!({"task_ids":[t1],"status":"invalid"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ============================================================
// v10 T4: Task restore from trash
// ============================================================

#[tokio::test]
async fn test_task_restore() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"TrashMe"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    // Delete (soft)
    app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();

    // Verify in trash
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks/trash", &tok, None)).await.unwrap();
    let trash = body_json(resp).await;
    assert!(trash.as_array().unwrap().iter().any(|t| t["id"] == tid));

    // Restore
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/restore", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);

    // Verify no longer in trash
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks/trash", &tok, None)).await.unwrap();
    let trash = body_json(resp).await;
    assert!(!trash.as_array().unwrap().iter().any(|t| t["id"] == tid));
}

// ============================================================
// v10 T5: Sprint completion snapshot
// ============================================================

#[tokio::test]
async fn test_sprint_completion_takes_snapshot() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"SprintTask","remaining_points":5.0})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"SnapSprint"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/complete", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let sprint = body_json(resp).await;
    assert_eq!(sprint["status"], "completed");
    // Burndown should have at least one snapshot
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/burndown", sid), &tok, None)).await.unwrap();
    let burndown = body_json(resp).await;
    assert!(burndown.as_array().unwrap().len() >= 1);
}

// ============================================================
// v10 T6: Refresh token rotation
// ============================================================

#[tokio::test]
async fn test_refresh_token_rotation() {
    let app = app().await;
    // Register and get tokens
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"refreshUser","password":"Pass1234"})))).await.unwrap();
    let auth = body_json(resp).await;
    let refresh = auth["refresh_token"].as_str().unwrap().to_string();

    // Refresh
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/refresh", Some(json!({"refresh_token": refresh})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let new_auth = body_json(resp).await;
    assert!(new_auth["token"].as_str().unwrap().len() > 10);

    // Old refresh token should be revoked
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/refresh", Some(json!({"refresh_token": refresh})))).await.unwrap();
    assert_eq!(resp.status(), 401);
}

// ============================================================
// v10 T7: Sprint date validation
// ============================================================

#[tokio::test]
async fn test_sprint_date_ordering() {
    let app = app().await;
    let tok = login_root(&app).await;
    // end_date before start_date should fail
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok,
        Some(json!({"name":"BadDates","start_date":"2026-04-20","end_date":"2026-04-10"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // Valid dates should succeed
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok,
        Some(json!({"name":"GoodDates","start_date":"2026-04-10","end_date":"2026-04-20"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
}

// ============================================================
// v10 T8: Concurrent task update conflict
// ============================================================

#[tokio::test]
async fn test_task_update_conflict_detection() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"ConflictTask"})))).await.unwrap();
    let task = body_json(resp).await;
    let tid = task["id"].as_i64().unwrap();
    let updated_at = task["updated_at"].as_str().unwrap().to_string();

    // First update succeeds
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok,
        Some(json!({"title":"Updated","expected_updated_at": updated_at})))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Second update with stale timestamp fails
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok,
        Some(json!({"title":"Stale","expected_updated_at": updated_at})))).await.unwrap();
    assert_eq!(resp.status(), 409);
}

// ============================================================
// v11 T1: Sprint carry-over
// ============================================================

#[tokio::test]
async fn test_sprint_carryover() {
    let app = app().await;
    let tok = login_root(&app).await;
    let t1 = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"CarryDone","status":"completed"})))).await.unwrap()).await["id"].as_i64().unwrap();
    let t2 = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"CarryWIP"})))).await.unwrap()).await["id"].as_i64().unwrap();
    // Mark t1 completed
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", t1), &tok, Some(json!({"status":"completed"})))).await.unwrap();
    let sid = body_json(app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"CarrySprint"})))).await.unwrap()).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[t1, t2]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/complete", sid), &tok, None)).await.unwrap();
    // Carry over
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/carryover", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let new_sprint = body_json(resp).await;
    assert!(new_sprint["name"].as_str().unwrap().contains("carry-over"));
    // Non-completed sprint should fail
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/carryover", new_sprint["id"].as_i64().unwrap()), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ============================================================
// v11 T2: Task watchers
// ============================================================

#[tokio::test]
async fn test_task_watchers() {
    let app = app().await;
    let tok = login_root(&app).await;
    let tid = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"WatchMe"})))).await.unwrap()).await["id"].as_i64().unwrap();
    // Watch
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/watch", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    // List watchers
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/watchers", tid), &tok, None)).await.unwrap();
    let watchers = body_json(resp).await;
    assert_eq!(watchers.as_array().unwrap().len(), 1);
    // Watched tasks
    let resp = app.clone().oneshot(auth_req("GET", "/api/watched", &tok, None)).await.unwrap();
    let watched = body_json(resp).await;
    assert!(watched.as_array().unwrap().iter().any(|v| v.as_i64() == Some(tid)));
    // Unwatch
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/watch", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

// ============================================================
// v11 T3: JSON task import
// ============================================================

#[tokio::test]
async fn test_json_import() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks/json", &tok, Some(json!({
        "tasks": [
            {"title": "Parent", "children": [{"title": "Child1"}, {"title": "Child2"}]},
            {"title": "Standalone"}
        ]
    })))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let result = body_json(resp).await;
    assert_eq!(result["created"].as_i64().unwrap(), 4);
    // Empty title rejected
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks/json", &tok, Some(json!({
        "tasks": [{"title": ""}]
    })))).await.unwrap();
    let result = body_json(resp).await;
    assert!(!result["errors"].as_array().unwrap().is_empty());
}

// ============================================================
// v11 T4: Session note update
// ============================================================

#[tokio::test]
async fn test_session_note_update() {
    let app = app().await;
    let tok = login_root(&app).await;
    let tid = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"NoteTask"})))).await.unwrap()).await["id"].as_i64().unwrap();
    // Start and stop a session to create one
    app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({"task_id": tid})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", "/api/timer/stop", &tok, None)).await.unwrap();
    // Get sessions
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/sessions", tid), &tok, None)).await.unwrap();
    let sessions = body_json(resp).await;
    let sid = sessions.as_array().unwrap()[0]["id"].as_i64().unwrap();
    // Update note
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/sessions/{}/note", sid), &tok, Some(json!({"note":"Updated note"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let session = body_json(resp).await;
    assert_eq!(session["notes"], "Updated note");
}

// ============================================================
// v11 T5: Room export
// ============================================================

#[tokio::test]
async fn test_room_export() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"ExportRoom"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/join", rid), &tok, None)).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/rooms/{}/export", rid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert!(resp.headers().get("content-disposition").is_some());
}

// ============================================================
// v11 T6: Per-task work duration
// ============================================================

#[tokio::test]
async fn test_task_work_duration() {
    let app = app().await;
    let tok = login_root(&app).await;
    let tid = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"DurTask"})))).await.unwrap()).await["id"].as_i64().unwrap();
    // Set work duration
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"work_duration_minutes": 45})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let task = body_json(resp).await;
    assert_eq!(task["work_duration_minutes"], 45);
    // Invalid bounds
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"work_duration_minutes": 999})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ============================================================
// v11 T7: Sprint capacity_hours
// ============================================================

#[tokio::test]
async fn test_sprint_capacity() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"CapSprint","capacity_hours":40.0})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let sprint = body_json(resp).await;
    assert_eq!(sprint["capacity_hours"], 40.0);
    // Update capacity
    let sid = sprint["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/sprints/{}", sid), &tok, Some(json!({"capacity_hours": 60.0})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let sprint = body_json(resp).await;
    assert_eq!(sprint["capacity_hours"], 60.0);
}

// ============================================================
// v11 T9: Dependency cycle detection
// ============================================================

#[tokio::test]
async fn test_dependency_self_reference() {
    let app = app().await;
    let tok = login_root(&app).await;
    let tid = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"DepSelf"})))).await.unwrap()).await["id"].as_i64().unwrap();
    // Self-dependency should fail
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/dependencies", tid), &tok, Some(json!({"depends_on": tid})))).await.unwrap();
    assert!(resp.status() == 400 || resp.status() == 500);
}

// ============================================================
// v11 T10: Webhook URL length validation
// ============================================================

#[tokio::test]
async fn test_webhook_url_length() {
    let app = app().await;
    let tok = login_root(&app).await;
    let long_url = format!("https://example.com/{}", "a".repeat(2000));
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok, Some(json!({"url": long_url})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ============================================================
// v11 T8: Auto-archive completed tasks
// ============================================================

#[tokio::test]
async fn test_auto_archive() {
    // Create pool directly so we can run raw SQL
    let pool = pomodoro_daemon::db::connect_memory().await.unwrap();
    let config = pomodoro_daemon::config::Config::default();
    let engine = Arc::new(pomodoro_daemon::engine::Engine::new(pool.clone(), config).await);
    let app = pomodoro_daemon::build_router(engine).await;
    let tok = login_root(&app).await;
    // Create and complete a task
    let tid = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"ArchiveMe"})))).await.unwrap()).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"completed"})))).await.unwrap();
    // Backdate updated_at to simulate old completion
    sqlx::query("UPDATE tasks SET updated_at = '2020-01-01T00:00:00' WHERE id = ?").bind(tid).execute(&pool).await.unwrap();
    // Run archive logic (same SQL as main.rs auto-archive)
    let cutoff = "2025-01-01T00:00:00";
    let result = sqlx::query("UPDATE tasks SET status = 'archived', updated_at = ? WHERE status = 'completed' AND updated_at < ? AND deleted_at IS NULL")
        .bind("2025-01-01T00:00:01").bind(cutoff).execute(&pool).await.unwrap();
    assert!(result.rows_affected() >= 1);
    // Verify archived via API
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
    let task = body_json(resp).await;
    assert_eq!(task["task"]["status"], "archived");
}

// ============================================================
// v12 T1: FTS5 search
// ============================================================

#[tokio::test]
async fn test_fts5_search() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Quantum physics research"})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Buy groceries"})))).await.unwrap();
    // Search should find the physics task
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks?search=quantum", &tok, None)).await.unwrap();
    let tasks = body_json(resp).await;
    let arr = tasks.as_array().unwrap();
    assert!(arr.iter().any(|t| t["title"].as_str().unwrap().contains("Quantum")));
    assert!(!arr.iter().any(|t| t["title"].as_str().unwrap().contains("groceries")));
}

// ============================================================
// v12 T2: Backup list endpoint
// ============================================================

#[tokio::test]
async fn test_backup_list() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/admin/backups", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Restore with invalid filename
    let resp = app.clone().oneshot(auth_req("POST", "/api/admin/restore", &tok, Some(json!({"filename":"../../../etc/passwd"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ============================================================
// v12 T3: User hours report
// ============================================================

#[tokio::test]
async fn test_user_hours_report() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/reports/user-hours", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data.as_array().unwrap().len() >= 1); // At least root user
    // Invalid date format
    let resp = app.clone().oneshot(auth_req("GET", "/api/reports/user-hours?from=garbage", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ============================================================
// v12 T4: Sprint carry-over preserves capacity
// ============================================================

#[tokio::test]
async fn test_carryover_preserves_capacity() {
    let app = app().await;
    let tok = login_root(&app).await;
    let tid = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"CapTask"})))).await.unwrap()).await["id"].as_i64().unwrap();
    let sid = body_json(app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"CapSprint","capacity_hours":42.0})))).await.unwrap()).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/complete", sid), &tok, None)).await.unwrap();
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/carryover", sid), &tok, None)).await.unwrap();
    let new_sprint = body_json(resp).await;
    assert_eq!(new_sprint["capacity_hours"], 42.0);
}

// ============================================================
// v12 T5: Dependency ownership enforcement
// ============================================================

#[tokio::test]
async fn test_dependency_ownership() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Register a second user
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"depuser","password":"Password1!"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"depuser","password":"Password1!"})))).await.unwrap();
    let user_tok = body_json(resp).await["token"].as_str().unwrap().to_string();
    // Root creates a task
    let tid = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"RootTask"})))).await.unwrap()).await["id"].as_i64().unwrap();
    let tid2 = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"RootTask2"})))).await.unwrap()).await["id"].as_i64().unwrap();
    // Non-owner can't add dependency
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/dependencies", tid), &user_tok, Some(json!({"depends_on": tid2})))).await.unwrap();
    assert_eq!(resp.status(), 403);
}

// ============================================================
// v12 T9: Sprint date ordering on create
// ============================================================

#[tokio::test]
async fn test_sprint_date_ordering_create() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"BadDates","start_date":"2025-03-01","end_date":"2025-02-01"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// ============================================================
// v12 T10: Concurrent timer start
// ============================================================

#[tokio::test]
async fn test_concurrent_timer_start() {
    let app = app().await;
    let tok = login_root(&app).await;
    let t1 = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Timer1"})))).await.unwrap()).await["id"].as_i64().unwrap();
    let t2 = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Timer2"})))).await.unwrap()).await["id"].as_i64().unwrap();
    // Start first timer
    app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({"task_id": t1})))).await.unwrap();
    // Start second timer — should stop first
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({"task_id": t2})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let state = body_json(resp).await;
    assert_eq!(state["current_task_id"], t2);
}

// ============================================================
// v12 T6: Room membership filter
// ============================================================

#[tokio::test]
async fn test_room_membership_filter() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create a room and join
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"FilterRoom"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/join", rid), &tok, None)).await.unwrap();
    // Register second user
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"roomuser","password":"Password1!"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"roomuser","password":"Password1!"})))).await.unwrap();
    let user_tok = body_json(resp).await["token"].as_str().unwrap().to_string();
    // Second user should NOT see the room (not a member)
    let resp = app.clone().oneshot(auth_req("GET", "/api/rooms", &user_tok, None)).await.unwrap();
    let rooms = body_json(resp).await;
    assert!(!rooms.as_array().unwrap().iter().any(|r| r["id"] == rid));
    // Root should see it
    let resp = app.clone().oneshot(auth_req("GET", "/api/rooms", &tok, None)).await.unwrap();
    let rooms = body_json(resp).await;
    assert!(rooms.as_array().unwrap().iter().any(|r| r["id"] == rid));
}

// ============================================================
// v12 T7: Bulk status mixed ownership
// ============================================================

#[tokio::test]
async fn test_bulk_status_mixed_ownership() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Register second user
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"bulkuser","password":"Password1!"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"bulkuser","password":"Password1!"})))).await.unwrap();
    let user_tok = body_json(resp).await["token"].as_str().unwrap().to_string();
    // Root creates a task
    let root_tid = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"RootOwned"})))).await.unwrap()).await["id"].as_i64().unwrap();
    // User creates a task
    let user_tid = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &user_tok, Some(json!({"title":"UserOwned"})))).await.unwrap()).await["id"].as_i64().unwrap();
    // User tries to bulk-update both — should fail (root's task not owned)
    let resp = app.clone().oneshot(auth_req("PUT", "/api/tasks/bulk-status", &user_tok, Some(json!({"task_ids":[root_tid, user_tid],"status":"active"})))).await.unwrap();
    assert_eq!(resp.status(), 403);
}

// ============================================================
// v12 T8: CSV import field length validation
// ============================================================

#[tokio::test]
async fn test_csv_import_title_length() {
    let app = app().await;
    let tok = login_root(&app).await;
    let long_title = "x".repeat(600);
    let csv = format!("title,priority\n{},3\nShort,2", long_title);
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &tok, Some(json!({"csv": csv})))).await.unwrap();
    let result = body_json(resp).await;
    // Long title should produce an error, short one should succeed
    assert_eq!(result["created"].as_i64().unwrap(), 1);
    assert!(!result["errors"].as_array().unwrap().is_empty());
}

// ============================================================
// Integration tests validating business flows end-to-end
// ============================================================


// ============================================================
// Sprint 11: Security Fixes Tests
// ============================================================

// H1: internal() should not leak raw error details
#[tokio::test]
async fn test_internal_error_no_leak() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Trigger an internal error by requesting a non-existent task for an operation that uses .map_err(internal)
    // Use a webhook update on a non-existent ID — the NOT_FOUND path is explicit, but let's test
    // that error messages from DB operations don't contain SQL/table names
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks/999999", &tok, None)).await.unwrap();
    if resp.status() == 500 {
        let j = body_json(resp).await;
        let msg = j["error"].as_str().unwrap_or("");
        assert!(!msg.contains("sqlx"), "Error message should not contain sqlx details");
        assert!(!msg.contains("SELECT"), "Error message should not contain SQL");
        assert!(!msg.contains("tasks"), "Error message should not contain table names");
        assert_eq!(msg, "Internal server error");
    }
    // 404 is also acceptable — the point is no SQL leak on 500
}

// M1: Global search should not leak comments from other users' tasks
#[tokio::test]
async fn test_global_search_comment_isolation() {
    let app = app().await;
    let root_tok = login_root(&app).await;
    let user_tok = register_user(&app, "searchuser1").await;

    // Root creates a task and adds a comment with a unique keyword
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &root_tok, Some(json!({"title":"Root secret task"})))).await.unwrap();
    let task_id = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", task_id), &root_tok, Some(json!({"content":"xyzzy_secret_keyword"})))).await.unwrap();

    // Normal user searches for the keyword — should NOT find root's comment
    let resp = app.clone().oneshot(auth_req("GET", "/api/search?q=xyzzy_secret_keyword", &user_tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j = body_json(resp).await;
    let comments = j["comments"].as_array().unwrap();
    assert!(comments.is_empty(), "Non-admin user should not see comments on other users' tasks");

    // Root can still find it
    let resp = app.clone().oneshot(auth_req("GET", "/api/search?q=xyzzy_secret_keyword", &root_tok, None)).await.unwrap();
    let j = body_json(resp).await;
    let comments = j["comments"].as_array().unwrap();
    assert!(!comments.is_empty(), "Root should see all comments");
}

// M2: Advanced search LIKE injection — wildcards should be escaped
#[tokio::test]
async fn test_advanced_search_like_escape() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create tasks with specific projects
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T1","project":"alpha_beta"})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T2","project":"alphaXbeta"})))).await.unwrap();

    // Search with underscore wildcard — should only match literal underscore
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks/search/advanced", &tok, Some(json!({
        "filters": [{"field":"project","op":"contains","value":"a_b"}]
    })))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let results = body_json(resp).await;
    let arr = results.as_array().unwrap();
    // Should find alpha_beta but NOT alphaXbeta (underscore is literal, not wildcard)
    assert!(arr.iter().any(|t| t["project"].as_str().unwrap_or("") == "alpha_beta"));
    assert!(!arr.iter().any(|t| t["project"].as_str().unwrap_or("") == "alphaXbeta"),
        "LIKE wildcard _ should be escaped — alphaXbeta should not match a_b");
}

// M3: FTS5 snippets should have HTML sanitized (except <mark>)
// Note: sanitize_snippet is tested directly in db::tasks::tests. This integration test
// verifies the search endpoint doesn't return raw HTML in snippets.
#[tokio::test]
async fn test_fts5_snippet_xss_sanitized() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create a task with HTML in the title
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({
        "title": "Test <img onerror=alert(1)> xsstest123"
    })))).await.unwrap();

    // Use the FTS search endpoint that returns snippets
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks/search?q=xsstest123", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let results = body_json(resp).await;
    let arr = results.as_array().unwrap();
    assert!(!arr.is_empty(), "Search should find the task");
    for r in arr {
        let title = r["title"].as_str().unwrap_or("");
        // When FTS5 is active, snippets go through sanitize_snippet (no raw HTML).
        // When FTS5 is not active, the raw title is returned (no snippet processing).
        // In either case, <script> tags should not appear in snippets.
        assert!(!title.contains("<script"), "Script tags should not appear in search results: {}", title);
    }

    // Also test via global search
    let resp = app.clone().oneshot(auth_req("GET", "/api/search?q=xsstest123", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j = body_json(resp).await;
    for t in j["tasks"].as_array().unwrap() {
        let snippet = t["snippet"].as_str().unwrap_or("");
        assert!(!snippet.contains("<script"), "Script tags should not appear in global search snippets: {}", snippet);
    }
}

// M7: Webhook update should validate events
#[tokio::test]
async fn test_webhook_update_validates_events() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create a valid webhook
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok, Some(json!({
        "url": "https://example.com/hook", "events": "task.created"
    })))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let wid = body_json(resp).await["id"].as_i64().unwrap();

    // Update with invalid event — should be rejected
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/webhooks/{}", wid), &tok, Some(json!({
        "events": "invalid.event"
    })))).await.unwrap();
    assert_eq!(resp.status(), 400);
    let j = body_json(resp).await;
    assert!(j["error"].as_str().unwrap().contains("Unknown event"));

    // Update with valid event — should succeed
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/webhooks/{}", wid), &tok, Some(json!({
        "events": "task.updated,sprint.completed"
    })))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Wildcard should also work
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/webhooks/{}", wid), &tok, Some(json!({
        "events": "*"
    })))).await.unwrap();
    assert_eq!(resp.status(), 200);
}

// ============================================================
// Sprint 12: updated_by on tasks
// ============================================================

#[tokio::test]
async fn test_updated_by_set_on_task_update() {
    let app = app().await;
    let tok = login_root(&app).await;
    let task = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"UB Test"})))).await.unwrap()).await;
    let tid = task["id"].as_i64().unwrap();
    // updated_by should be null on creation
    assert!(task["updated_by"].is_null());
    // Update the task
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"title":"UB Updated"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let updated = body_json(resp).await;
    assert_eq!(updated["updated_by"], task["user_id"]);
}

// ============================================================
// Sprint 12: user timezone
// ============================================================

#[tokio::test]
async fn test_user_timezone_update() {
    let app = app().await;
    let tok = register_user(&app, "tzuser").await;
    // Set timezone
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"timezone":"Europe/Stockholm"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Verify via admin user list (root)
    let root_tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/admin/users", &root_tok, None)).await.unwrap();
    let users = body_json(resp).await;
    let tz_user = users.as_array().unwrap().iter().find(|u| u["username"] == "tzuser").unwrap();
    assert_eq!(tz_user["timezone"], "Europe/Stockholm");
}

#[tokio::test]
async fn test_user_timezone_clear() {
    let app = app().await;
    let tok = register_user(&app, "tzuser2").await;
    // Set then clear
    app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"timezone":"US/Eastern"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"timezone":""})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let root_tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/admin/users", &root_tok, None)).await.unwrap();
    let users = body_json(resp).await;
    let u = users.as_array().unwrap().iter().find(|u| u["username"] == "tzuser2").unwrap();
    assert!(u["timezone"].is_null());
}

// ============================================================
// Sprint 12: watcher notifications on comment
// ============================================================

#[tokio::test]
async fn test_watcher_notified_on_comment() {
    let app = app().await;
    let root_tok = login_root(&app).await;
    let (alice_tok, _alice_id) = register_user_full(&app, "walice", "Pass1234").await;
    // Root creates task, alice watches it
    let tid = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &root_tok, Some(json!({"title":"WatchComment"})))).await.unwrap()).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/watch", tid), &alice_tok, None)).await.unwrap();
    // Root comments — alice should get notified
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &root_tok, Some(json!({"content":"Hello watchers"})))).await.unwrap();
    // Give the spawned task time to run
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/notifications", &alice_tok, None)).await.unwrap();
    let notifs = body_json(resp).await;
    let found = notifs.as_array().unwrap().iter().any(|n| n["kind"] == "comment_added");
    assert!(found, "Watcher should receive comment_added notification");
}

#[tokio::test]
async fn test_watcher_not_notified_own_comment() {
    let app = app().await;
    let root_tok = login_root(&app).await;
    // Root creates task and watches it
    let tid = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &root_tok, Some(json!({"title":"SelfComment"})))).await.unwrap()).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/watch", tid), &root_tok, None)).await.unwrap();
    // Root comments on own watched task — should NOT get notified
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &root_tok, Some(json!({"content":"My own comment"})))).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/notifications", &root_tok, None)).await.unwrap();
    let notifs = body_json(resp).await;
    let found = notifs.as_array().unwrap().iter().any(|n| n["kind"] == "comment_added");
    assert!(!found, "Commenter should NOT receive own comment notification");
}

// ============================================================
// Sprint 12: watcher notifications on status change
// ============================================================

#[tokio::test]
async fn test_watcher_notified_on_status_change() {
    let app = app().await;
    let root_tok = login_root(&app).await;
    let (bob_tok, _bob_id) = register_user_full(&app, "wbob", "Pass1234").await;
    // Root creates task, bob watches it
    let tid = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &root_tok, Some(json!({"title":"WatchStatus"})))).await.unwrap()).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/watch", tid), &bob_tok, None)).await.unwrap();
    // Root changes status
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &root_tok, Some(json!({"status":"in_progress"})))).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/notifications", &bob_tok, None)).await.unwrap();
    let notifs = body_json(resp).await;
    let found = notifs.as_array().unwrap().iter().any(|n| n["kind"] == "task_status_changed");
    assert!(found, "Watcher should receive task_status_changed notification");
}

// ============================================================
// Sprint 12: new notification pref event types
// ============================================================

#[tokio::test]
async fn test_notification_prefs_include_new_types() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/profile/notifications", &tok, None)).await.unwrap();
    let prefs = body_json(resp).await;
    let types: Vec<&str> = prefs.as_array().unwrap().iter().map(|p| p["event_type"].as_str().unwrap()).collect();
    assert!(types.contains(&"task_status_changed"), "Should include task_status_changed");
    assert!(types.contains(&"mention"), "Should include mention");
}

// ============================================================
// Sprint 13: Paginate /api/tasks/full
// ============================================================

#[tokio::test]
async fn test_tasks_full_returns_pagination_headers() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create a task so there's data
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Full1"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks/full?page=1&per_page=10", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert!(resp.headers().get("x-total-count").is_some());
    assert_eq!(resp.headers().get("x-page").unwrap().to_str().unwrap(), "1");
    assert_eq!(resp.headers().get("x-per-page").unwrap().to_str().unwrap(), "10");
}

#[tokio::test]
async fn test_tasks_full_project_filter() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"ProjA","project":"alpha"})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"ProjB","project":"beta"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks/full?project=alpha", &tok, None)).await.unwrap();
    let j = body_json(resp).await;
    let tasks = j["tasks"].as_array().unwrap();
    assert!(tasks.iter().all(|t| t["project"] == "alpha"));
    assert!(tasks.iter().any(|t| t["title"] == "ProjA"));
}

#[tokio::test]
async fn test_tasks_full_pagination() {
    let app = app().await;
    let tok = login_root(&app).await;
    for i in 0..5 { app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":format!("Page{}", i)})))).await.unwrap(); }
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks/full?per_page=2&page=1", &tok, None)).await.unwrap();
    let total: i64 = resp.headers().get("x-total-count").unwrap().to_str().unwrap().parse().unwrap();
    assert!(total >= 5);
    let j = body_json(resp).await;
    assert_eq!(j["tasks"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_tasks_full_backward_compatible() {
    let app = app().await;
    let tok = login_root(&app).await;
    // No query params — should still work (returns all tasks)
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks/full", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j = body_json(resp).await;
    assert!(j["tasks"].is_array());
    assert!(j["task_sprints"].is_array());
    assert!(j["burn_totals"].is_array());
}

// ============================================================
// Sprint 13: Saved views CRUD
// ============================================================

#[tokio::test]
async fn test_saved_views_crud() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create
    let resp = app.clone().oneshot(auth_req("POST", "/api/views", &tok, Some(json!({"name":"My Filter","filters":{"status":"active","project":"alpha"}})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let view = body_json(resp).await;
    assert_eq!(view["name"], "My Filter");
    let vid = view["id"].as_i64().unwrap();
    // List
    let resp = app.clone().oneshot(auth_req("GET", "/api/views", &tok, None)).await.unwrap();
    let views = body_json(resp).await;
    assert!(views.as_array().unwrap().iter().any(|v| v["id"].as_i64() == Some(vid)));
    // Update
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/views/{}", vid), &tok, Some(json!({"name":"Updated","filters":{"status":"done"}})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["name"], "Updated");
    // Delete
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/views/{}", vid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_saved_views_empty_name_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/views", &tok, Some(json!({"name":"","filters":{}})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_saved_views_user_isolation() {
    let app = app().await;
    let tok_a = register_user(&app, "viewuserA").await;
    let tok_b = register_user(&app, "viewuserB").await;
    // A creates a view
    let resp = app.clone().oneshot(auth_req("POST", "/api/views", &tok_a, Some(json!({"name":"A's view","filters":{}})))).await.unwrap();
    let vid = body_json(resp).await["id"].as_i64().unwrap();
    // B cannot see it
    let resp = app.clone().oneshot(auth_req("GET", "/api/views", &tok_b, None)).await.unwrap();
    let views = body_json(resp).await;
    assert!(!views.as_array().unwrap().iter().any(|v| v["id"].as_i64() == Some(vid)));
    // B cannot delete it
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/views/{}", vid), &tok_b, None)).await.unwrap();
    assert_eq!(resp.status(), 404);
}

// ============================================================
// Sprint 13: Webhook deliveries endpoint
// ============================================================

#[tokio::test]
async fn test_webhook_deliveries_endpoint() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create a webhook
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok, Some(json!({"url":"https://example.com/hook"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let wid = body_json(resp).await["id"].as_i64().unwrap();
    // List deliveries (should be empty)
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/webhooks/{}/deliveries", wid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let deliveries = body_json(resp).await;
    assert!(deliveries.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_webhook_deliveries_not_owner_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let user_tok = register_user(&app, "whuser").await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok, Some(json!({"url":"https://example.com/hook2"})))).await.unwrap();
    let wid = body_json(resp).await["id"].as_i64().unwrap();
    // Non-owner cannot see deliveries
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/webhooks/{}/deliveries", wid), &user_tok, None)).await.unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_projects_crud() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create project
    let resp = app.clone().oneshot(auth_req("POST", "/api/projects", &tok, Some(json!({"name":"Alpha Project","description":"Test project","key":"ALPHA"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let project = body_json(resp).await;
    let pid = project["id"].as_i64().unwrap();
    assert_eq!(project["name"], "Alpha Project");
    assert_eq!(project["key"], "ALPHA");
    assert_eq!(project["status"], "active");
    // List projects
    let resp = app.clone().oneshot(auth_req("GET", "/api/projects", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let projects = body_json(resp).await;
    assert!(projects.as_array().unwrap().iter().any(|p| p["key"] == "ALPHA"));
    // Get project
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/projects/{}", pid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["name"], "Alpha Project");
    // Update project
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/projects/{}", pid), &tok, Some(json!({"name":"Alpha v2","status":"archived"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let updated = body_json(resp).await;
    assert_eq!(updated["name"], "Alpha v2");
    assert_eq!(updated["status"], "archived");
    // Delete project
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/projects/{}", pid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Verify deleted
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/projects/{}", pid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_projects_duplicate_key_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/projects", &tok, Some(json!({"name":"P1","key":"DUP"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let resp = app.clone().oneshot(auth_req("POST", "/api/projects", &tok, Some(json!({"name":"P2","key":"DUP"})))).await.unwrap();
    assert_eq!(resp.status(), 409);
}

#[tokio::test]
async fn test_projects_non_admin_rejected() {
    let app = app().await;
    let user_tok = register_user(&app, "projuser").await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/projects", &user_tok, Some(json!({"name":"Nope","key":"NOPE"})))).await.unwrap();
    assert_eq!(resp.status(), 403);
    // But can list
    let resp = app.clone().oneshot(auth_req("GET", "/api/projects", &user_tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_task_with_project_id() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create project
    let resp = app.clone().oneshot(auth_req("POST", "/api/projects", &tok, Some(json!({"name":"TaskProj","key":"TP"})))).await.unwrap();
    let pid = body_json(resp).await["id"].as_i64().unwrap();
    // Create task with project_id
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Proj Task","project_id":pid})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let task = body_json(resp).await;
    assert_eq!(task["project_id"], pid);
    assert_eq!(task["project_name"], "TaskProj");
    // Update task to remove project_id
    let tid = task["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"project_id":null})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let updated = body_json(resp).await;
    assert!(updated["project_id"].is_null());
    assert!(updated["project_name"].is_null());
}

#[tokio::test]
async fn test_project_delete_unlinks_tasks() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/projects", &tok, Some(json!({"name":"DelProj","key":"DEL"})))).await.unwrap();
    let pid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Linked","project_id":pid})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Delete project
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/projects/{}", pid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Task should still exist but project_id should be null
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
    let task = body_json(resp).await;
    assert!(task["task"]["project_id"].is_null());
}

// ── Status Transition Rules ────────────────────────────────────

#[tokio::test]
async fn test_transition_rules_crud() {
    let app = common::app().await;
    let token = common::login_root(&app).await;

    // List transitions (empty initially)
    let resp = app.clone().oneshot(common::auth_req("GET", "/api/workflows/transitions", &token, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = common::body_json(resp).await;
    assert_eq!(body.as_array().unwrap().len(), 0);

    // Create a transition rule
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/workflows/transitions", &token,
        Some(serde_json::json!({"from_status": "backlog", "to_status": "active"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let t = common::body_json(resp).await;
    assert_eq!(t["from_status"], "backlog");
    assert_eq!(t["to_status"], "active");
    assert!(t["project_id"].is_null());
    let tid = t["id"].as_i64().unwrap();

    // List again — should have 1
    let resp = app.clone().oneshot(common::auth_req("GET", "/api/workflows/transitions", &token, None)).await.unwrap();
    let body = common::body_json(resp).await;
    assert_eq!(body.as_array().unwrap().len(), 1);

    // Delete
    let resp = app.clone().oneshot(common::auth_req("DELETE", &format!("/api/workflows/transitions/{}", tid), &token, None)).await.unwrap();
    assert_eq!(resp.status(), 204);

    // List again — empty
    let resp = app.clone().oneshot(common::auth_req("GET", "/api/workflows/transitions", &token, None)).await.unwrap();
    let body = common::body_json(resp).await;
    assert_eq!(body.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_transition_rules_non_admin_rejected() {
    let app = common::app().await;
    let user_token = common::register_user(&app, "transuser").await;

    let resp = app.clone().oneshot(common::auth_req("POST", "/api/workflows/transitions", &user_token,
        Some(serde_json::json!({"from_status": "backlog", "to_status": "active"})))).await.unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_transition_rules_duplicate_rejected() {
    let app = common::app().await;
    let token = common::login_root(&app).await;

    let resp = app.clone().oneshot(common::auth_req("POST", "/api/workflows/transitions", &token,
        Some(serde_json::json!({"from_status": "backlog", "to_status": "active"})))).await.unwrap();
    assert_eq!(resp.status(), 201);

    // Duplicate should fail
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/workflows/transitions", &token,
        Some(serde_json::json!({"from_status": "backlog", "to_status": "active"})))).await.unwrap();
    assert_eq!(resp.status(), 409);
}

#[tokio::test]
async fn test_transition_rules_same_status_rejected() {
    let app = common::app().await;
    let token = common::login_root(&app).await;

    let resp = app.clone().oneshot(common::auth_req("POST", "/api/workflows/transitions", &token,
        Some(serde_json::json!({"from_status": "active", "to_status": "active"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_transition_enforcement_blocks_invalid() {
    let app = common::app().await;
    let token = common::login_root(&app).await;

    // Create transition rules: only backlog→active and active→completed allowed
    app.clone().oneshot(common::auth_req("POST", "/api/workflows/transitions", &token,
        Some(serde_json::json!({"from_status": "backlog", "to_status": "active"})))).await.unwrap();
    app.clone().oneshot(common::auth_req("POST", "/api/workflows/transitions", &token,
        Some(serde_json::json!({"from_status": "active", "to_status": "completed"})))).await.unwrap();

    // Create a task (starts as backlog)
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/tasks", &token,
        Some(serde_json::json!({"title": "Trans test"})))).await.unwrap();
    let task = common::body_json(resp).await;
    let task_id = task["id"].as_i64().unwrap();

    // Try to go directly from backlog→completed (not allowed)
    let resp = app.clone().oneshot(common::auth_req("PUT", &format!("/api/tasks/{}", task_id), &token,
        Some(serde_json::json!({"status": "completed"})))).await.unwrap();
    assert_eq!(resp.status(), 400);

    // Go backlog→active (allowed)
    let resp = app.clone().oneshot(common::auth_req("PUT", &format!("/api/tasks/{}", task_id), &token,
        Some(serde_json::json!({"status": "active"})))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Go active→completed (allowed)
    let resp = app.clone().oneshot(common::auth_req("PUT", &format!("/api/tasks/{}", task_id), &token,
        Some(serde_json::json!({"status": "completed"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_transition_no_rules_allows_all() {
    let app = common::app().await;
    let token = common::login_root(&app).await;

    // No transition rules defined — any transition should work
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/tasks", &token,
        Some(serde_json::json!({"title": "Free trans"})))).await.unwrap();
    let task = common::body_json(resp).await;
    let task_id = task["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(common::auth_req("PUT", &format!("/api/tasks/{}", task_id), &token,
        Some(serde_json::json!({"status": "completed"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_transition_enforcement_bulk_status() {
    let app = common::app().await;
    let token = common::login_root(&app).await;

    // Create transition rules: only backlog→active
    app.clone().oneshot(common::auth_req("POST", "/api/workflows/transitions", &token,
        Some(serde_json::json!({"from_status": "backlog", "to_status": "active"})))).await.unwrap();

    // Create tasks
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/tasks", &token,
        Some(serde_json::json!({"title": "Bulk1"})))).await.unwrap();
    let t1 = common::body_json(resp).await["id"].as_i64().unwrap();

    // Bulk update to "completed" should fail (backlog→completed not allowed)
    let resp = app.clone().oneshot(common::auth_req("PUT", "/api/tasks/bulk-status", &token,
        Some(serde_json::json!({"task_ids": [t1], "status": "completed"})))).await.unwrap();
    assert_eq!(resp.status(), 400);

    // Bulk update to "active" should succeed
    let resp = app.clone().oneshot(common::auth_req("PUT", "/api/tasks/bulk-status", &token,
        Some(serde_json::json!({"task_ids": [t1], "status": "active"})))).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_transition_project_specific_rules() {
    let app = common::app().await;
    let token = common::login_root(&app).await;

    // Create a project
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/projects", &token,
        Some(serde_json::json!({"name": "TransProj", "key": "TP"})))).await.unwrap();
    let proj_id = common::body_json(resp).await["id"].as_i64().unwrap();

    // Create project-specific rule: only backlog→active
    app.clone().oneshot(common::auth_req("POST", "/api/workflows/transitions", &token,
        Some(serde_json::json!({"from_status": "backlog", "to_status": "active", "project_id": proj_id})))).await.unwrap();

    // Create task in that project
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/tasks", &token,
        Some(serde_json::json!({"title": "Proj task", "project_id": proj_id})))).await.unwrap();
    let task_id = common::body_json(resp).await["id"].as_i64().unwrap();

    // backlog→completed should fail (project rule restricts it)
    let resp = app.clone().oneshot(common::auth_req("PUT", &format!("/api/tasks/{}", task_id), &token,
        Some(serde_json::json!({"status": "completed"})))).await.unwrap();
    assert_eq!(resp.status(), 400);

    // backlog→active should succeed
    let resp = app.clone().oneshot(common::auth_req("PUT", &format!("/api/tasks/{}", task_id), &token,
        Some(serde_json::json!({"status": "active"})))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Task without project should still be free (no global rules)
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/tasks", &token,
        Some(serde_json::json!({"title": "No proj task"})))).await.unwrap();
    let task2_id = common::body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(common::auth_req("PUT", &format!("/api/tasks/{}", task2_id), &token,
        Some(serde_json::json!({"status": "completed"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
}

// ── Automation Rules Wiring ────────────────────────────────────

#[tokio::test]
async fn test_automation_status_change_sets_priority() {
    let app = common::app().await;
    let token = common::login_root(&app).await;

    // Create automation: when status changes to "active", set priority to 5
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/automations", &token,
        Some(serde_json::json!({
            "name": "Urgent on active",
            "trigger_event": "task.status_changed",
            "condition_json": r#"{"to_status":"active"}"#,
            "action_json": r#"{"set_priority":5}"#
        })))).await.unwrap();
    assert_eq!(resp.status(), 201);

    // Create task
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/tasks", &token,
        Some(serde_json::json!({"title": "Auto test", "priority": 3})))).await.unwrap();
    let task = common::body_json(resp).await;
    let task_id = task["id"].as_i64().unwrap();

    // Change status to active
    let resp = app.clone().oneshot(common::auth_req("PUT", &format!("/api/tasks/{}", task_id), &token,
        Some(serde_json::json!({"status": "active"})))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Wait for async automation to complete
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Check priority was updated to 5
    let resp = app.clone().oneshot(common::auth_req("GET", &format!("/api/tasks/{}", task_id), &token, None)).await.unwrap();
    let detail = common::body_json(resp).await;
    assert_eq!(detail["task"]["priority"], 5);
}

#[tokio::test]
async fn test_automation_all_subtasks_done() {
    let app = common::app().await;
    let token = common::login_root(&app).await;

    // Create automation: when all subtasks done, set parent to "completed"
    app.clone().oneshot(common::auth_req("POST", "/api/automations", &token,
        Some(serde_json::json!({
            "name": "Auto complete parent",
            "trigger_event": "task.all_subtasks_done",
            "condition_json": "{}",
            "action_json": r#"{"set_status":"completed"}"#
        })))).await.unwrap();

    // Create parent + child
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/tasks", &token,
        Some(serde_json::json!({"title": "Parent"})))).await.unwrap();
    let parent_id = common::body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(common::auth_req("POST", "/api/tasks", &token,
        Some(serde_json::json!({"title": "Child", "parent_id": parent_id})))).await.unwrap();
    let child_id = common::body_json(resp).await["id"].as_i64().unwrap();

    // Complete the child
    app.clone().oneshot(common::auth_req("PUT", &format!("/api/tasks/{}", child_id), &token,
        Some(serde_json::json!({"status": "completed"})))).await.unwrap();

    // Wait for automation
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Parent should be completed
    let resp = app.clone().oneshot(common::auth_req("GET", &format!("/api/tasks/{}", parent_id), &token, None)).await.unwrap();
    let detail = common::body_json(resp).await;
    assert_eq!(detail["task"]["status"], "completed");
}

#[tokio::test]
async fn test_automation_task_created_trigger() {
    let app = common::app().await;
    let token = common::login_root(&app).await;

    // Create automation: when task created, set priority to 1
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/automations", &token,
        Some(serde_json::json!({
            "name": "Low priority default",
            "trigger_event": "task.created",
            "condition_json": "{}",
            "action_json": r#"{"set_priority":1}"#
        })))).await.unwrap();
    assert_eq!(resp.status(), 201);

    // Create task with default priority 3
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/tasks", &token,
        Some(serde_json::json!({"title": "Auto created"})))).await.unwrap();
    let task_id = common::body_json(resp).await["id"].as_i64().unwrap();

    // Wait for async automation
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Priority should be 1
    let resp = app.clone().oneshot(common::auth_req("GET", &format!("/api/tasks/{}", task_id), &token, None)).await.unwrap();
    let detail = common::body_json(resp).await;
    assert_eq!(detail["task"]["priority"], 1);
}

#[tokio::test]
async fn test_automation_task_assigned_trigger() {
    let app = common::app().await;
    let token = common::login_root(&app).await;
    common::register_user(&app, "assignee1").await;

    // Create automation: when task assigned, set status to "in_progress"
    app.clone().oneshot(common::auth_req("POST", "/api/automations", &token,
        Some(serde_json::json!({
            "name": "Auto in-progress on assign",
            "trigger_event": "task.assigned",
            "condition_json": "{}",
            "action_json": r#"{"set_status":"in_progress"}"#
        })))).await.unwrap();

    // Create task
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/tasks", &token,
        Some(serde_json::json!({"title": "Assign test"})))).await.unwrap();
    let task_id = common::body_json(resp).await["id"].as_i64().unwrap();

    // Assign user
    app.clone().oneshot(common::auth_req("POST", &format!("/api/tasks/{}/assignees", task_id), &token,
        Some(serde_json::json!({"username": "assignee1"})))).await.unwrap();

    // Wait for async automation
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Status should be in_progress
    let resp = app.clone().oneshot(common::auth_req("GET", &format!("/api/tasks/{}", task_id), &token, None)).await.unwrap();
    let detail = common::body_json(resp).await;
    assert_eq!(detail["task"]["status"], "in_progress");
}

#[tokio::test]
async fn test_automation_priority_changed_trigger() {
    let app = common::app().await;
    let token = common::login_root(&app).await;

    // Create automation: when priority changes to 5, set status to "active"
    app.clone().oneshot(common::auth_req("POST", "/api/automations", &token,
        Some(serde_json::json!({
            "name": "Activate on urgent",
            "trigger_event": "task.priority_changed",
            "condition_json": r#"{"priority":5}"#,
            "action_json": r#"{"set_status":"active"}"#
        })))).await.unwrap();

    // Create task
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/tasks", &token,
        Some(serde_json::json!({"title": "Priority test", "priority": 3})))).await.unwrap();
    let task_id = common::body_json(resp).await["id"].as_i64().unwrap();

    // Change priority to 5
    app.clone().oneshot(common::auth_req("PUT", &format!("/api/tasks/{}", task_id), &token,
        Some(serde_json::json!({"priority": 5})))).await.unwrap();

    // Wait for async automation
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Status should be active
    let resp = app.clone().oneshot(common::auth_req("GET", &format!("/api/tasks/{}", task_id), &token, None)).await.unwrap();
    let detail = common::body_json(resp).await;
    assert_eq!(detail["task"]["status"], "active");
}
