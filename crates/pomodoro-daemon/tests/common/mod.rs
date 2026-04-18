use axum::body::Body;
use http_body_util::BodyExt;
use hyper::Request;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

pub async fn app() -> axum::Router {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        std::env::set_var("POMODORO_ROOT_PASSWORD", "root");
        std::env::set_var("POMODORO_NO_RATE_LIMIT", "1");
    });
    let pool = pomodoro_daemon::db::connect_memory().await.unwrap();
    let config = pomodoro_daemon::config::Config::default();
    let engine = Arc::new(pomodoro_daemon::engine::Engine::new(pool, config).await);
    pomodoro_daemon::build_router(engine).await
}

pub fn json_req(method: &str, uri: &str, body: Option<Value>) -> Request<Body> {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let ip = format!("10.{}.{}.{}", (n / 65025 % 255) + 1, (n / 255 % 255) + 1, (n % 255) + 1);
    let b = Request::builder().method(method).uri(uri)
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip);
    if let Some(v) = body {
        b.body(Body::from(serde_json::to_vec(&v).unwrap())).unwrap()
    } else {
        b.body(Body::empty()).unwrap()
    }
}

pub fn auth_req(method: &str, uri: &str, token: &str, body: Option<Value>) -> Request<Body> {
    static AUTH_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(5_000_000);
    let n = AUTH_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let ip = format!("10.{}.{}.{}", (n / 65025 % 255) + 1, (n / 255 % 255) + 1, (n % 255) + 1);
    let b = Request::builder().method(method).uri(uri)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .header("x-requested-with", "test")
        .header("x-forwarded-for", ip);
    if let Some(v) = body {
        b.body(Body::from(serde_json::to_vec(&v).unwrap())).unwrap()
    } else {
        b.body(Body::empty()).unwrap()
    }
}

pub async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

pub async fn login_root(app: &axum::Router) -> String {
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"root","password":"root"})))).await.unwrap();
    body_json(resp).await["token"].as_str().unwrap().to_string()
}

pub async fn register_user(app: &axum::Router, username: &str) -> String {
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username": username, "password": "Pass1234"})))).await.unwrap();
    assert!(resp.status().is_success(), "register {} failed: {}", username, resp.status());
    body_json(resp).await["token"].as_str().unwrap().to_string()
}

pub async fn register_user_full(app: &axum::Router, username: &str, password: &str) -> (String, i64) {
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username": username, "password": password})))).await.unwrap();
    assert_eq!(resp.status(), 200, "register {} failed", username);
    let j = body_json(resp).await;
    (j["token"].as_str().unwrap().to_string(), j["user_id"].as_i64().unwrap())
}

#[allow(dead_code)]
pub async fn reg(app: &axum::Router, username: &str) -> String {
    register_user_full(app, username, "Pass1234").await.0
}
