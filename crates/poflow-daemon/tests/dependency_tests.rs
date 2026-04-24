use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{app, json_req, auth_req, body_json, login_root};

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
    assert!(!body_json(resp).await.as_array().unwrap().is_empty());
    // Remove dependency
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/dependencies/{}", b, a), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Self-dependency should fail
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/dependencies", a), &tok, Some(json!({"depends_on": a})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

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
    assert!(!all.as_array().unwrap().is_empty());
    // Remove dependency
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/dependencies/{}", id1, id2), &tok, None)).await.unwrap();
    assert!(resp.status().is_success());
}

#[tokio::test]
async fn test_dependency_self_reference() {
    let app = app().await;
    let tok = login_root(&app).await;
    let tid = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"DepSelf"})))).await.unwrap()).await["id"].as_i64().unwrap();
    // Self-dependency should fail
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/dependencies", tid), &tok, Some(json!({"depends_on": tid})))).await.unwrap();
    assert!(resp.status() == 400 || resp.status() == 500);
}

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
