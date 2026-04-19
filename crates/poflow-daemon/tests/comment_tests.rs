use axum::body::Body;
use http_body_util::BodyExt;
use hyper::Request;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

mod common;
use common::{app, json_req, auth_req, body_json, login_root, register_user, register_user_full, reg};

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

#[tokio::test]
async fn test_threaded_comments() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"thread task"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Add root comment
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &tok, Some(json!({"content":"root comment"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let root_id = body_json(resp).await["id"].as_i64().unwrap();
    // Add reply
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &tok, Some(json!({"content":"reply","parent_id":root_id})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let reply = body_json(resp).await;
    assert_eq!(reply["parent_id"], root_id);
    // List comments — both should be there
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/comments", tid), &tok, None)).await.unwrap();
    let comments = body_json(resp).await;
    assert_eq!(comments.as_array().unwrap().len(), 2);
    let reply_comment = comments.as_array().unwrap().iter().find(|c| c["parent_id"].as_i64() == Some(root_id)).unwrap();
    assert_eq!(reply_comment["content"], "reply");
}