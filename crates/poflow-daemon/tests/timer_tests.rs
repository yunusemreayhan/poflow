use axum::body::Body;
use http_body_util::BodyExt;
use hyper::Request;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

mod common;
use common::{app, json_req, auth_req, body_json, login_root, register_user, register_user_full, reg};

#[tokio::test]
async fn test_timer_state() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.oneshot(auth_req("GET", "/api/timer", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let s = body_json(resp).await;
    assert_eq!(s["status"], "Idle");
}

#[tokio::test]
async fn test_timer_user_isolation() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"timeruser","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"timeruser","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Root starts timer
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let root_state = body_json(resp).await;
    assert_eq!(root_state["status"], "Running");

    // Other user sees their own idle timer, not root's
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer", &tok2, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let user_state = body_json(resp).await;
    assert_eq!(user_state["status"], "Idle");

    // Other user can start their own timer independently
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok2, Some(json!({})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let user_state = body_json(resp).await;
    assert_eq!(user_state["status"], "Running");

    // Root's timer is still running
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer", &tok, None)).await.unwrap();
    let root_state = body_json(resp).await;
    assert_eq!(root_state["status"], "Running");

    // Root can stop own timer
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/stop", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["status"], "Idle");

    // User's timer still running
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer", &tok2, None)).await.unwrap();
    assert_eq!(body_json(resp).await["status"], "Running");
}

#[tokio::test]
async fn test_timer_full_lifecycle() {
    let app = app().await;
    let tok = register_user(&app, "timerUser").await;
    // Start
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let state = body_json(resp).await;
    assert_eq!(state["status"], "Running");
    assert_eq!(state["phase"], "Work");
    // Pause
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/pause", &tok, Some(json!({})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["status"], "Paused");
    // Resume
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/resume", &tok, Some(json!({})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["status"], "Running");
    // Stop
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/stop", &tok, Some(json!({})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["status"], "Idle");
}

#[tokio::test]
async fn test_timer_start_with_task() {
    let app = app().await;
    let tok = register_user(&app, "timerTaskUser").await;
    let task = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Focus"})))).await.unwrap()).await;
    let tid = task["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({"task_id":tid})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let state = body_json(resp).await;
    assert_eq!(state["current_task_id"], tid);
}

#[tokio::test]
async fn test_timer_skip_from_idle() {
    let app = app().await;
    let tok = register_user(&app, "skipIdleUser").await;
    // Skip from idle — should still return a valid state
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/skip", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let state = body_json(resp).await;
    assert_eq!(state["status"], "Idle");
}

#[tokio::test]
async fn test_timer_start_break() {
    let app = app().await;
    let tok = register_user(&app, "breakUser").await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({"phase":"short_break"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let state = body_json(resp).await;
    assert_eq!(state["phase"], "ShortBreak");
    assert_eq!(state["status"], "Running");
}

#[tokio::test]
async fn test_skip_advances_phase() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Start work
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({})))).await.unwrap();
    assert!(resp.status().is_success());
    let state = body_json(resp).await;
    assert_eq!(state["phase"], "Work");
    // Skip
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/skip", &tok, None)).await.unwrap();
    assert!(resp.status().is_success());
    let state = body_json(resp).await;
    assert_eq!(state["status"], "Idle");
    assert!(state["phase"] == "ShortBreak" || state["phase"] == "LongBreak");
}

#[tokio::test]
async fn test_concurrent_timer_start_stop() {
    let app = app().await;
    let tok = login_root(&app).await;
    let start_body = Some(json!({}));
    // Start timer
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, start_body.clone())).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Pause then stop
    let _ = app.clone().oneshot(auth_req("POST", "/api/timer/pause", &tok, None)).await.unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/stop", &tok, None)).await.unwrap();
    assert!(resp.status().is_success());
    // Start again — should succeed (not stuck)
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, start_body)).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Verify state is running
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer", &tok, None)).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["status"].as_str().unwrap(), "Running");
}

#[tokio::test]
async fn test_concurrent_timer_start() {
    let app = app().await;
    let tok = login_root(&app).await;
    let t1 = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Timer1"})))).await.unwrap()).await["id"].as_i64().unwrap();
    let t2 = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Timer2"})))).await.unwrap()).await["id"].as_i64().unwrap();
    // Start first timer
    app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({"task_id": t1})))).await.unwrap();
    // Start second timer — should stop first
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({"task_id": t2})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let state = body_json(resp).await;
    assert_eq!(state["current_task_id"], t2);
}

#[tokio::test]
async fn test_timer_persist_and_restore() {
    use poflow_daemon::{db, config::Config, engine::Engine};
    let pool = db::connect_memory().await.unwrap();
    let config = Config::default();
    let engine = std::sync::Arc::new(Engine::new(pool.clone(), config.clone()).await);

    // Create a user
    db::create_user(&pool, "persist_user", "$2b$04$LJ0fRCDPiLe/gkz0.Ey3/.dummy.hash.value", "user").await.unwrap();

    // Start a timer
    let state = engine.start(1, None, None).await.unwrap();
    assert_eq!(state.status, poflow_daemon::engine::TimerStatus::Running);
    assert_eq!(state.phase, poflow_daemon::engine::TimerPhase::Work);

    // Verify persisted to DB
    let rows = db::load_timer_states(&pool).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].user_id, 1);
    assert_eq!(rows[0].phase, "Work");
    assert_eq!(rows[0].status, "Running");

    // Simulate restart: create new engine, restore
    let engine2 = std::sync::Arc::new(Engine::new(pool.clone(), config).await);
    engine2.restore_states().await;
    let restored = engine2.get_state(1).await;
    assert_eq!(restored.phase, poflow_daemon::engine::TimerPhase::Work);
    assert_eq!(restored.status, poflow_daemon::engine::TimerStatus::Paused); // Restored as Paused
    assert_eq!(restored.duration_s, state.duration_s);
}

#[tokio::test]
async fn test_timer_persist_cleared_on_stop() {
    use poflow_daemon::{db, config::Config, engine::Engine};
    let pool = db::connect_memory().await.unwrap();
    let config = Config::default();
    let engine = std::sync::Arc::new(Engine::new(pool.clone(), config).await);

    db::create_user(&pool, "stop_user", "$2b$04$LJ0fRCDPiLe/gkz0.Ey3/.dummy.hash.value", "user").await.unwrap();

    engine.start(1, None, None).await.unwrap();
    assert_eq!(db::load_timer_states(&pool).await.unwrap().len(), 1);

    engine.stop(1).await.unwrap();
    assert_eq!(db::load_timer_states(&pool).await.unwrap().len(), 0);
}

#[tokio::test]
async fn test_timer_persist_pause_resume() {
    use poflow_daemon::{db, config::Config, engine::Engine};
    let pool = db::connect_memory().await.unwrap();
    let config = Config::default();
    let engine = std::sync::Arc::new(Engine::new(pool.clone(), config).await);

    db::create_user(&pool, "pause_user", "$2b$04$LJ0fRCDPiLe/gkz0.Ey3/.dummy.hash.value", "user").await.unwrap();

    engine.start(1, None, None).await.unwrap();
    engine.pause(1).await.unwrap();

    let rows = db::load_timer_states(&pool).await.unwrap();
    assert_eq!(rows[0].status, "Paused");

    engine.resume(1).await.unwrap();
    let rows = db::load_timer_states(&pool).await.unwrap();
    assert_eq!(rows[0].status, "Running");
}
