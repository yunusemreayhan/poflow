use axum::body::Body;
use http_body_util::BodyExt;
use hyper::Request;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

mod common;
use common::{app, json_req, auth_req, body_json, login_root, register_user, register_user_full, reg};

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