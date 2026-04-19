use axum::body::Body;
use http_body_util::BodyExt;
use hyper::Request;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

mod common;
use common::{app, json_req, auth_req, body_json, login_root, register_user, register_user_full, reg};

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

#[tokio::test]
async fn test_password_min_length() {
    let app = app().await;
    let resp = app.oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"short","password":"abc"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
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
async fn test_auth_rate_limiting() {
    let app = app().await;
    if std::env::var("POFLOW_NO_RATE_LIMIT").is_ok() { return; }
    poflow_daemon::routes::auth_limiter().reset();
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

#[tokio::test]
async fn test_register_duplicate_username() {
    let app = app().await;
    register_user(&app, "dupUser").await;
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"dupUser","password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 409);
}

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

#[tokio::test]
async fn test_auth_rate_limit_threshold() {
    let app = app().await;
    if std::env::var("POFLOW_NO_RATE_LIMIT").is_ok() { return; }
    poflow_daemon::routes::auth_limiter().reset();

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

#[tokio::test]
async fn test_auth_rate_limit_blocks_after_threshold() {
    let app = app().await;
    if std::env::var("POFLOW_NO_RATE_LIMIT").is_ok() { return; }
    poflow_daemon::routes::auth_limiter().reset();
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

#[tokio::test]
async fn test_read_rate_limiter_enforced() {
    // F3: GET requests are rate-limited at 1000/min (not unlimited)
    if std::env::var("POFLOW_NO_RATE_LIMIT").is_ok() { return; }
    let limiter = poflow_daemon::routes::read_limiter();
    limiter.reset();
    let ip = "10.88.88.88";
    for _ in 0..1000 {
        assert!(limiter.check_and_record(ip));
    }
    assert!(!limiter.check_and_record(ip), "GET rate limiter should reject after 1000 requests");
}

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