use axum::body::Body;
use http_body_util::BodyExt;
use hyper::Request;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

mod common;
use common::{app, json_req, auth_req, body_json, login_root, register_user, register_user_full, reg};

#[tokio::test]
async fn test_config_get() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.oneshot(auth_req("GET", "/api/config", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let c = body_json(resp).await;
    assert_eq!(c["work_duration_min"], 25);
}

#[tokio::test]
async fn test_user_config_override() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Get default config
    let resp = app.clone().oneshot(auth_req("GET", "/api/config", &tok, None)).await.unwrap();
    let cfg = body_json(resp).await;
    assert_eq!(cfg["work_duration_min"], 25);

    // Update config (per-user override)
    let mut new_cfg = cfg.clone();
    new_cfg["work_duration_min"] = json!(30);
    new_cfg["daily_goal"] = json!(10);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(new_cfg))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Verify override persists
    let resp = app.clone().oneshot(auth_req("GET", "/api/config", &tok, None)).await.unwrap();
    let cfg = body_json(resp).await;
    assert_eq!(cfg["work_duration_min"], 30);
    assert_eq!(cfg["daily_goal"], 10);
}

#[tokio::test]
async fn test_config_validation_bounds() {
    let app = app().await;
    let tok = login_root(&app).await;
    let base = json!({"work_duration_min":25,"short_break_min":5,"long_break_min":15,"long_break_interval":4,"daily_goal":8,"estimation_mode":"points","auto_start_breaks":false,"auto_start_work":false,"sound_enabled":false,"notification_enabled":false});
    // work_duration_min too high
    let mut bad = base.clone(); bad["work_duration_min"] = json!(999);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(bad))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // daily_goal too high
    let mut bad = base.clone(); bad["daily_goal"] = json!(100);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(bad))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // invalid estimation_mode
    let mut bad = base.clone(); bad["estimation_mode"] = json!("invalid");
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(bad))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_per_user_config_isolation() {
    let app = app().await;
    let root_tok = login_root(&app).await;

    // Register second user
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"configUser","password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"configUser","password":"Pass1234"})))).await.unwrap();
    let user_tok = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Both start with same defaults
    let resp = app.clone().oneshot(auth_req("GET", "/api/config", &root_tok, None)).await.unwrap();
    let root_cfg = body_json(resp).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/config", &user_tok, None)).await.unwrap();
    let user_cfg = body_json(resp).await;
    assert_eq!(root_cfg["work_duration_min"], user_cfg["work_duration_min"]);

    // User changes their config to 15 min
    let mut new_user_cfg = user_cfg.clone();
    new_user_cfg["work_duration_min"] = json!(15);
    new_user_cfg["daily_goal"] = json!(3);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &user_tok, Some(new_user_cfg))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // User sees their override
    let resp = app.clone().oneshot(auth_req("GET", "/api/config", &user_tok, None)).await.unwrap();
    let user_cfg = body_json(resp).await;
    assert_eq!(user_cfg["work_duration_min"], 15, "user should see their override");
    assert_eq!(user_cfg["daily_goal"], 3);

    // Root still sees the global default (unaffected by user's override)
    let resp = app.clone().oneshot(auth_req("GET", "/api/config", &root_tok, None)).await.unwrap();
    let root_cfg = body_json(resp).await;
    assert_eq!(root_cfg["work_duration_min"], 25, "root should still see global default");
    assert_eq!(root_cfg["daily_goal"], 8);
}

#[tokio::test]
async fn test_config_all_bounds() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/config", &tok, None)).await.unwrap();
    let cfg = body_json(resp).await;
    // work_duration_min = 0 should fail
    let mut bad = cfg.clone(); bad["work_duration_min"] = json!(0);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(bad))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // work_duration_min = 241 should fail
    let mut bad = cfg.clone(); bad["work_duration_min"] = json!(241);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(bad))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // short_break_min = 0 should fail
    let mut bad = cfg.clone(); bad["short_break_min"] = json!(0);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(bad))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // long_break_min = 0 should fail
    let mut bad = cfg.clone(); bad["long_break_min"] = json!(0);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(bad))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // daily_goal = 51 should fail
    let mut bad = cfg.clone(); bad["daily_goal"] = json!(51);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(bad))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // estimation_mode = "invalid" should fail
    let mut bad = cfg.clone(); bad["estimation_mode"] = json!("invalid");
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(bad))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_config_rejects_zero_interval() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok,
        Some(json!({"work_duration_min":25,"short_break_min":5,"long_break_min":15,"long_break_interval":0,"daily_goal":8,"auto_start_breaks":false,"auto_start_work":false,"estimation_mode":"points","theme":"dark"})))).await.unwrap();
    assert!(resp.status().as_u16() >= 400);
}

#[tokio::test]
async fn test_config_theme_validation() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Get current config
    let resp = app.clone().oneshot(auth_req("GET", "/api/config", &tok, None)).await.unwrap();
    let mut cfg = body_json(resp).await;

    // Invalid theme
    cfg["theme"] = json!("neon");
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(cfg.clone()))).await.unwrap();
    assert_eq!(resp.status(), 400);

    // Valid theme
    cfg["theme"] = json!("dark");
    let resp = app.clone().oneshot(auth_req("PUT", "/api/config", &tok, Some(cfg))).await.unwrap();
    assert_eq!(resp.status(), 200);
}
