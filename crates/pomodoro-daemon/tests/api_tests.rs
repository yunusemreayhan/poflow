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
        .header("authorization", format!("Bearer {}", token));
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
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"alice","password":"pass123"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let resp = app.oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"alice","password":"pass123"})))).await.unwrap();
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
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"bob","password":"pass123"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"bob","password":"pass123"})))).await.unwrap();
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
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"eve","password":"pass123"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"eve","password":"pass123"})))).await.unwrap();
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
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"dan","password":"pass123"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"dan","password":"pass123"})))).await.unwrap();
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
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"noob","password":"pass123"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"noob","password":"pass123"})))).await.unwrap();
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
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"bob","password":"pass123"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"bob","password":"pass123"})))).await.unwrap();
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
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"alice2","password":"pass123"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"alice2","password":"pass123"})))).await.unwrap();
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
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"roomuser","password":"pass123"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"roomuser","password":"pass123"})))).await.unwrap();
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
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"sprintuser","password":"pass123"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"sprintuser","password":"pass123"})))).await.unwrap();
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
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"timeruser","password":"pass123"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"timeruser","password":"pass123"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Root starts timer
    app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({})))).await.unwrap();

    // Other user cannot pause
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/pause", &tok2, None)).await.unwrap();
    assert_eq!(resp.status(), 403);

    // Other user cannot stop
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/stop", &tok2, None)).await.unwrap();
    assert_eq!(resp.status(), 403);

    // Root can stop own timer
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/stop", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
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
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"assignuser","password":"pass123"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"assignuser","password":"pass123"})))).await.unwrap();
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
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"delme","password":"pass123"})))).await.unwrap();
    let uid = body_json(resp).await["user_id"].as_i64().unwrap();

    // Create task as delme
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"delme","password":"pass123"})))).await.unwrap();
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
    // Should be 5.0 (remaining_points only), NOT 8.0 (5+3)
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
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"sprintuser2","password":"pass123"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"sprintuser2","password":"pass123"})))).await.unwrap();
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
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"burnuser","password":"pass123"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"burnuser","password":"pass123"})))).await.unwrap();
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
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"viewer","password":"pass123"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"viewer","password":"pass123"})))).await.unwrap();
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
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"unique1","password":"pass123"})))).await.unwrap();

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
