use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{app, auth_req, body_json, login_root};

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
async fn test_audit_log_entity_filter() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create a task to generate audit entry
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"AuditTest"})))).await.unwrap();
    // Filter by entity_type
    let resp = app.clone().oneshot(auth_req("GET", "/api/audit?entity_type=task", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let entries = body_json(resp).await;
    assert!(!entries.as_array().unwrap().is_empty());
    for e in entries.as_array().unwrap() {
        assert_eq!(e["entity_type"], "task");
    }
}

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
