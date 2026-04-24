use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{app, auth_req, body_json, login_root};

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
