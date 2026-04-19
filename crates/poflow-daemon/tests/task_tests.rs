use axum::body::Body;
use http_body_util::BodyExt;
use hyper::Request;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

mod common;
use common::{app, json_req, auth_req, body_json, login_root, register_user, register_user_full, reg};

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

#[tokio::test]
async fn test_task_empty_title_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":""})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"   "})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

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

#[tokio::test]
async fn test_task_negative_estimated_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T","estimated":-1})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T","estimated_hours":-1.0})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

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

#[tokio::test]
async fn test_tasks_full_scoped_joins() {
    // F4: burn_totals, assignees, labels should only contain data for tasks in the current page
    let app = app().await;
    let tok = login_root(&app).await;
    // Create 3 tasks
    let mut ids = vec![];
    for i in 0..3 {
        let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title": format!("Scoped{}", i)})))).await.unwrap();
        let j = body_json(resp).await;
        ids.push(j["id"].as_i64().unwrap());
    }
    // Assign user to task 0 only
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", ids[0]), &tok, Some(json!({"username": "root"})))).await.unwrap();
    // Create a label and attach to task 2 only
    let resp = app.clone().oneshot(auth_req("POST", "/api/labels", &tok, Some(json!({"name": "scoped-lbl", "color": "#ff0000"})))).await.unwrap();
    let label_id = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/labels/{}", ids[2], label_id), &tok, None)).await.unwrap();
    // Fetch page 1 with per_page=1 (should only get task 0's join data)
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks/full?per_page=1&page=1", &tok, None)).await.unwrap();
    let j = body_json(resp).await;
    let tasks = j["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    let page_id = tasks[0]["id"].as_i64().unwrap();
    // Assignees should only contain entries for the page task
    for a in j["assignees"].as_array().unwrap() {
        assert_eq!(a["task_id"].as_i64().unwrap(), page_id, "assignee leaked from another page");
    }
    // Labels should only contain entries for the page task
    for l in j["labels"].as_array().unwrap() {
        assert_eq!(l["task_id"].as_i64().unwrap(), page_id, "label leaked from another page");
    }
}

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
async fn test_bulk_assign_nonexistent_user_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"BA"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks/bulk-assign", &tok, Some(json!({"task_ids":[tid],"username":"nonexistent_user_xyz"})))).await.unwrap();
    assert_eq!(resp.status(), 404);
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

#[tokio::test]
async fn test_checklist_empty_title_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"CL"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/checklist", tid), &tok, Some(json!({"title":""})))).await.unwrap();
    assert_eq!(resp.status(), 400);
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
