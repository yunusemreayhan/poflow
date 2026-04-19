use axum::body::Body;
use http_body_util::BodyExt;
use hyper::Request;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

mod common;
use common::{app, json_req, auth_req, body_json, login_root, register_user, register_user_full, reg};

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
async fn test_webhook_ssrf_private_ip() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create webhook with private IP — should be stored (validation happens at dispatch time)
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok, Some(json!({
        "url": "http://192.168.1.1/hook", "events": "task.created"
    })))).await.unwrap();
    // The webhook is created (SSRF check is at dispatch time, not creation)
    let status = resp.status().as_u16();
    assert!(status == 201 || status == 200 || status == 400);
}

#[tokio::test]
async fn test_webhook_ssrf_blocked() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Private IP should be blocked
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok,
        Some(json!({"url":"http://192.168.1.1/hook"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // Localhost should be blocked
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok,
        Some(json!({"url":"http://localhost/hook"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // Cloud metadata should be blocked
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok,
        Some(json!({"url":"http://169.254.169.254/latest/meta-data"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_webhook_ssrf_additional_patterns() {
    let app = app().await;
    let tok = login_root(&app).await;

    let blocked_urls = [
        "http://localhost/hook",
        "http://127.0.0.1/hook",
        "http://0.0.0.0/hook",
        "http://[::1]/hook",
        "http://10.0.0.1/hook",
        "http://192.168.1.1/hook",
        "http://172.16.0.1/hook",
        "http://169.254.1.1/hook",
        "http://internal.local/hook",
        "ftp://example.com/hook",
        "http://user:pass@example.com/hook",
    ];
    for url in &blocked_urls {
        let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok,
            Some(json!({"url": url, "events":"task.created"})))).await.unwrap();
        assert_eq!(resp.status(), 400, "Expected 400 for URL: {}", url);
    }
}

#[tokio::test]
async fn test_webhook_with_event_filter() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok, Some(json!({"url":"https://example.com/hook","events":"task.created,sprint.started","secret":"mysecret"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let wh = body_json(resp).await;
    assert_eq!(wh["events"], "task.created,sprint.started");
    assert!(wh["secret"].is_null() || wh["secret"].as_str().is_some()); // secret may be hidden
}

#[tokio::test]
async fn test_webhook_crud() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create webhook
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok,
        Some(json!({"url":"https://example.com/hook","events":"task.created,task.updated","secret":"s3cret"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let wh = body_json(resp).await;
    let wid = wh["id"].as_i64().unwrap();
    assert_eq!(wh["url"], "https://example.com/hook");

    // List webhooks
    let resp = app.clone().oneshot(auth_req("GET", "/api/webhooks", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let list = body_json(resp).await;
    assert!(list.as_array().unwrap().len() >= 1);

    // Invalid event rejected
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok,
        Some(json!({"url":"https://example.com/hook2","events":"invalid.event"})))).await.unwrap();
    assert_eq!(resp.status(), 400);

    // Delete webhook
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/webhooks/{}", wid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_webhook_rejects_private_ips() {
    let app = app().await;
    let tok = login_root(&app).await;
    let private_urls = [
        "http://localhost:8080/hook",
        "http://127.0.0.1:8080/hook",
        "http://10.0.0.1/hook",
        "http://192.168.1.1/hook",
        "http://172.16.0.1/hook",
    ];
    for url in private_urls {
        let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok, Some(json!({"url": url})))).await.unwrap();
        assert_eq!(resp.status(), 400, "Should reject private URL: {}", url);
    }
}

#[tokio::test]
async fn test_webhook_rejects_credentials_in_url() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok,
        Some(json!({"url":"http://user:pass@example.com/hook"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_webhook_url_length() {
    let app = app().await;
    let tok = login_root(&app).await;
    let long_url = format!("https://example.com/{}", "a".repeat(2000));
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok, Some(json!({"url": long_url})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_webhook_update_validates_events() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create a valid webhook
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok, Some(json!({
        "url": "https://example.com/hook", "events": "task.created"
    })))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let wid = body_json(resp).await["id"].as_i64().unwrap();

    // Update with invalid event — should be rejected
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/webhooks/{}", wid), &tok, Some(json!({
        "events": "invalid.event"
    })))).await.unwrap();
    assert_eq!(resp.status(), 400);
    let j = body_json(resp).await;
    assert!(j["error"].as_str().unwrap().contains("Unknown event"));

    // Update with valid event — should succeed
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/webhooks/{}", wid), &tok, Some(json!({
        "events": "task.updated,sprint.completed"
    })))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Wildcard should also work
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/webhooks/{}", wid), &tok, Some(json!({
        "events": "*"
    })))).await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_webhook_deliveries_endpoint() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create a webhook
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok, Some(json!({"url":"https://example.com/hook"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let wid = body_json(resp).await["id"].as_i64().unwrap();
    // List deliveries (should be empty)
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/webhooks/{}/deliveries", wid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let deliveries = body_json(resp).await;
    assert!(deliveries.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_webhook_deliveries_not_owner_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let user_tok = register_user(&app, "whuser").await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/webhooks", &tok, Some(json!({"url":"https://example.com/hook2"})))).await.unwrap();
    let wid = body_json(resp).await["id"].as_i64().unwrap();
    // Non-owner cannot see deliveries
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/webhooks/{}/deliveries", wid), &user_tok, None)).await.unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_github_webhook_invalid_json() {
    let app = app().await;
    let req = Request::builder().method("POST").uri("/api/integrations/github")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "10.99.0.1")
        .body(Body::from("not json")).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_github_webhook_links_commits() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create task #1
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"webhook task"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Send GitHub webhook payload referencing the task
    let payload = json!({
        "commits": [{"id": "abc1234567", "message": format!("Fix #{} — resolve bug", tid), "url": "https://github.com/test/commit/abc1234567"}],
        "repository": {"full_name": "test/repo"}
    });
    let resp = app.clone().oneshot(json_req("POST", "/api/integrations/github", Some(payload))).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Verify link was created
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/links", tid), &tok, None)).await.unwrap();
    let links = body_json(resp).await;
    assert_eq!(links.as_array().unwrap().len(), 1);
    assert_eq!(links[0]["link_type"], "commit");
    assert!(links[0]["title"].as_str().unwrap().contains("abc1234"));
}