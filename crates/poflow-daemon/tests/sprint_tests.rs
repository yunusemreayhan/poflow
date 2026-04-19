use axum::body::Body;
use http_body_util::BodyExt;
use hyper::Request;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

mod common;
use common::{app, json_req, auth_req, body_json, login_root, register_user, register_user_full, reg};

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

#[tokio::test]
async fn test_snapshot_sprint_points_not_double_counted() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Task with remaining_points=5, estimated=3 (poflows)
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok,
        Some(json!({"title":"T","remaining_points":5.0,"estimated":3})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();

    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/snapshot", sid), &tok, None)).await.unwrap();
    let stat = body_json(resp).await;
    // total_points = remaining_points (story points = 5), not estimated (poflows)
    assert_eq!(stat["total_points"], 5.0);
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

#[tokio::test]
async fn test_sprint_empty_name_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":""})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"   "})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

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

#[tokio::test]
async fn test_sprint_cannot_complete_from_planning() {
    let app = app().await;
    let tok = login_root(&app).await;
    let sprint = body_json(app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S2"})))).await.unwrap()).await;
    let sid = sprint["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/complete", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 400);
}

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

#[tokio::test]
async fn test_sprint_date_ordering_create() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"BadDates","start_date":"2025-03-01","end_date":"2025-02-01"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

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
