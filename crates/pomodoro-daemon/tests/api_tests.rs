use axum::body::Body;
use http_body_util::BodyExt;
use hyper::Request;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

async fn app() -> axum::Router {
    let pool = pomodoro_daemon::db::connect_memory().await.unwrap();
    let config = pomodoro_daemon::config::Config::default();
    let engine = Arc::new(pomodoro_daemon::engine::Engine::new(pool, config).await);
    pomodoro_daemon::build_router(engine)
}

fn json_req(method: &str, uri: &str, body: Option<Value>) -> Request<Body> {
    let b = Request::builder().method(method).uri(uri).header("content-type", "application/json");
    if let Some(v) = body {
        b.body(Body::from(serde_json::to_vec(&v).unwrap())).unwrap()
    } else {
        b.body(Body::empty()).unwrap()
    }
}

fn auth_req(method: &str, uri: &str, token: &str, body: Option<Value>) -> Request<Body> {
    let b = Request::builder().method(method).uri(uri)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .header("x-requested-with", "test");
    if let Some(v) = body {
        b.body(Body::from(serde_json::to_vec(&v).unwrap())).unwrap()
    } else {
        b.body(Body::empty()).unwrap()
    }
}

async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

async fn login_root(app: &axum::Router) -> String {
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"root","password":"root"})))).await.unwrap();
    body_json(resp).await["token"].as_str().unwrap().to_string()
}

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
    assert_eq!(resp.status(), 200);

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
    assert_eq!(resp.status(), 200);

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
    assert_eq!(resp.status(), 200);

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
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok, Some(json!({"task_id":tid,"points":5.0})))).await.unwrap();

    // Delete task — should clean sprint_tasks and burn_log
    app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();

    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/tasks", sid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 0);
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/burns", sid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 0);
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
    // total_points = estimated (story points = 3), not remaining_points
    assert_eq!(stat["total_points"], 3.0);
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

    // Viewer GETs room state — should NOT auto-join
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/rooms/{}", rid), &tok2, None)).await.unwrap();
    let state = body_json(resp).await;
    // Only root (creator) should be a member
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

    // Change password
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &new_tok, Some(json!({"password":"NewPass12"})))).await.unwrap();
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
