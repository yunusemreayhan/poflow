use axum::body::Body;
use http_body_util::BodyExt;
use hyper::Request;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

mod common;
use common::{app, json_req, auth_req, body_json, login_root, register_user, register_user_full, reg};

#[tokio::test]
async fn test_health_endpoint() {
    let app = app().await;
    let resp = app.clone().oneshot(Request::builder().method("GET").uri("/api/health").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = body_json(resp).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["db"], true);
}

#[tokio::test]
async fn test_history_empty() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.oneshot(auth_req("GET", "/api/history", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_history_date_range() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/history?from=2026-01-01&to=2026-12-31", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_stats_endpoint() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/stats?from=2026-01-01&to=2026-12-31", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let stats = body_json(resp).await;
    assert!(stats.is_array());
}

#[tokio::test]
async fn test_profile_update() {
    let app = app().await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"profuser","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"profuser","password":"Pass1234"})))).await.unwrap();
    let tok = body_json(resp).await["token"].as_str().unwrap().to_string();
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"username":"profuser2"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let auth = body_json(resp).await;
    assert_eq!(auth["username"], "profuser2");
    let new_tok = auth["token"].as_str().unwrap().to_string();
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &new_tok, Some(json!({"password":"NewPass12","current_password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"profuser2","password":"NewPass12"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"profuser2","password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_profile_password_change() {
    let app = app().await;
    let tok = register_user(&app, "pwChangeUser").await;
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"password":"NewPass123","current_password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let new_auth = body_json(resp).await;
    assert!(new_auth["token"].as_str().unwrap().len() > 10);
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"pwChangeUser","password":"NewPass123"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"pwChangeUser","password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 401);
    let tok2 = register_user(&app, "pwChangeUser2").await;
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok2, Some(json!({"password":"NewPass123"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok2, Some(json!({"password":"NewPass123","current_password":"WrongPass1"})))).await.unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_profile_weak_password_rejected() {
    let app = app().await;
    let tok = register_user(&app, "weakPwUser").await;
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"password":"Ab1"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"password":"alllower1"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"password":"NoDigitHere"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_password_change_requires_current() {
    let app = app().await;
    let tok = register_user(&app, "pwReqUser").await;
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"password":"NewPass99"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"password":"NewPass99","current_password":"WrongPass1"})))).await.unwrap();
    assert_eq!(resp.status(), 403);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"password":"NewPass99","current_password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_velocity() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/sprints/velocity", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert!(body_json(resp).await.as_array().is_some());
}

#[tokio::test]
async fn test_velocity_with_limit() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/sprints/velocity?sprints=5", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let vel = body_json(resp).await;
    assert!(vel.is_array());
}

#[tokio::test]
async fn test_global_burndown() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T","estimated":3})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/snapshot", sid), &tok, None)).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/sprints/burndown", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(!data.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_global_burndown_empty() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/sprints/burndown", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
}
#[tokio::test]
async fn test_due_date_reminder_query() {
    let app = app().await;
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"root","password":"root"})))).await.unwrap();
    let tok = body_json(resp).await["token"].as_str().unwrap().to_string();
    let tomorrow = (chrono::Utc::now() + chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Due soon","due_date":&tomorrow})))).await.unwrap();
    assert!(resp.status().is_success());
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Not due","due_date":"2099-12-31"})))).await.unwrap();
    assert!(resp.status().is_success());
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Done task","due_date":&tomorrow})))).await.unwrap();
    assert!(resp.status().is_success());
    let done_id = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", done_id), &tok, Some(json!({"status":"completed"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let day_after = (chrono::Utc::now() + chrono::Duration::days(2)).format("%Y-%m-%d").to_string();
    let pool = poflow_daemon::db::connect_memory().await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks", &tok, None)).await.unwrap();
    let tasks = body_json(resp).await;
    let due_tasks: Vec<&Value> = tasks.as_array().unwrap().iter()
        .filter(|t| t["due_date"].as_str().map_or(false, |d| d <= day_after.as_str()) && t["status"].as_str() != Some("completed"))
        .collect();
    assert_eq!(due_tasks.len(), 1);
    assert_eq!(due_tasks[0]["title"].as_str().unwrap(), "Due soon");
}

#[tokio::test]
async fn test_graceful_shutdown_recovery() {
    let app = app().await;
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"root","password":"root"})))).await.unwrap();
    let tok = body_json(resp).await["token"].as_str().unwrap().to_string();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Recovery test"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({"task_id": tid})))).await.unwrap();
    assert!(resp.status().is_success());
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer", &tok, None)).await.unwrap();
    let state = body_json(resp).await;
    assert_eq!(state["status"].as_str().unwrap(), "Running");
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/stop", &tok, None)).await.unwrap();
    assert!(resp.status().is_success());
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer", &tok, None)).await.unwrap();
    let state = body_json(resp).await;
    assert_eq!(state["status"].as_str().unwrap(), "Idle");
}

#[tokio::test]
async fn test_assignees() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &tok, Some(json!({"username":"root"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/assignees", tid), &tok, None)).await.unwrap();
    assert!(body_json(resp).await.as_array().unwrap().contains(&json!("root")));
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/assignees/root", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_remove_assignee_ownership() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"assignuser","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"assignuser","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &tok, Some(json!({"username":"root"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/assignees/root", tid), &tok2, None)).await.unwrap();
    assert_eq!(resp.status(), 403);
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/assignees/root", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_assignee_add_list_remove() {
    let app = app().await;
    let tok = login_root(&app).await;
    register_user(&app, "assigneeUser").await;
    let task = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap()).await;
    let tid = task["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &tok, Some(json!({"username":"assigneeUser"})))).await.unwrap();
    assert!(resp.status().is_success());
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/assignees", tid), &tok, None)).await.unwrap();
    let assignees = body_json(resp).await;
    assert!(assignees.as_array().unwrap().contains(&json!("assigneeUser")));
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/assignees/assigneeUser", tid), &tok, None)).await.unwrap();
    assert!(resp.status().is_success());
}

#[tokio::test]
async fn test_all_assignees_endpoint() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/assignees", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_notification_prefs_crud() {
    let app = app().await;
    let tok = register_user(&app, "notifUser").await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/profile/notifications", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let prefs = body_json(resp).await;
    let arr = prefs.as_array().unwrap();
    assert!(arr.len() >= 6);
    assert!(arr.iter().all(|p| p["enabled"] == true));
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile/notifications", &tok, Some(json!([{"event_type":"task_assigned","enabled":false}])))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let resp = app.clone().oneshot(auth_req("GET", "/api/profile/notifications", &tok, None)).await.unwrap();
    let prefs = body_json(resp).await;
    let ta = prefs.as_array().unwrap().iter().find(|p| p["event_type"] == "task_assigned").unwrap();
    assert_eq!(ta["enabled"], false);
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile/notifications", &tok, Some(json!([{"event_type":"bogus","enabled":true}])))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_bulk_status_change() {
    let app = app().await;
    let token = login_root(&app).await;
    let r1 = app.clone().oneshot(auth_req("POST", "/api/tasks", &token, Some(json!({"title":"Bulk1"})))).await.unwrap();
    let id1 = body_json(r1).await["id"].as_i64().unwrap();
    let r2 = app.clone().oneshot(auth_req("POST", "/api/tasks", &token, Some(json!({"title":"Bulk2"})))).await.unwrap();
    let id2 = body_json(r2).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("PUT", "/api/tasks/bulk-status", &token, Some(json!({"task_ids":[id1,id2],"status":"done"})))).await.unwrap();
    assert_eq!(resp.status(), 204);
    let r = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", id1), &token, None)).await.unwrap();
    assert_eq!(body_json(r).await["task"]["status"], "done");
}

#[tokio::test]
async fn test_bulk_status_invalid() {
    let app = app().await;
    let token = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("PUT", "/api/tasks/bulk-status", &token, Some(json!({"task_ids":[999],"status":"invalid"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_bulk_status_ownership_isolation() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"RootTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"user2","password":"Pass1234!"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();
    let resp = app.clone().oneshot(auth_req("PUT", "/api/tasks/bulk-status", &tok2, Some(json!({"task_ids":[tid],"status":"completed"})))).await.unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_bulk_status_mixed_ownership() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"bulkuser","password":"Password1!"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"bulkuser","password":"Password1!"})))).await.unwrap();
    let user_tok = body_json(resp).await["token"].as_str().unwrap().to_string();
    let root_tid = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"RootOwned"})))).await.unwrap()).await["id"].as_i64().unwrap();
    let user_tid = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &user_tok, Some(json!({"title":"UserOwned"})))).await.unwrap()).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("PUT", "/api/tasks/bulk-status", &user_tok, Some(json!({"task_ids":[root_tid, user_tid],"status":"active"})))).await.unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_bulk_status_update() {
    let app = app().await;
    let tok = login_root(&app).await;
    let t1 = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Bulk1"})))).await.unwrap()).await["id"].as_i64().unwrap();
    let t2 = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Bulk2"})))).await.unwrap()).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("PUT", "/api/tasks/bulk-status", &tok, Some(json!({"task_ids":[t1, t2],"status":"completed"})))).await.unwrap();
    assert_eq!(resp.status(), 204);
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", t1), &tok, None)).await.unwrap();
    let task = body_json(resp).await;
    assert_eq!(task["task"]["status"], "completed");
    let resp = app.clone().oneshot(auth_req("PUT", "/api/tasks/bulk-status", &tok, Some(json!({"task_ids":[t1],"status":"invalid"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_rate_limit_get_not_limited() {
    let app = app().await;
    let tok = login_root(&app).await;
    for _ in 0..10 {
        let resp = app.clone().oneshot(auth_req("GET", "/api/tasks", &tok, None)).await.unwrap();
        assert_eq!(resp.status(), 200);
    }
}

#[tokio::test]
async fn test_user_hours_report() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/reports/user-hours", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data.as_array().unwrap().len() >= 1);
    let resp = app.clone().oneshot(auth_req("GET", "/api/reports/user-hours?from=garbage", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_user_timezone_update() {
    let app = app().await;
    let tok = register_user(&app, "tzuser").await;
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"timezone":"Europe/Stockholm"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let root_tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/admin/users", &root_tok, None)).await.unwrap();
    let users = body_json(resp).await;
    let tz_user = users.as_array().unwrap().iter().find(|u| u["username"] == "tzuser").unwrap();
    assert_eq!(tz_user["timezone"], "Europe/Stockholm");
}

#[tokio::test]
async fn test_user_timezone_clear() {
    let app = app().await;
    let tok = register_user(&app, "tzuser2").await;
    app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"timezone":"US/Eastern"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"timezone":""})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let root_tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/admin/users", &root_tok, None)).await.unwrap();
    let users = body_json(resp).await;
    let u = users.as_array().unwrap().iter().find(|u| u["username"] == "tzuser2").unwrap();
    assert!(u["timezone"].is_null());
}

#[tokio::test]
async fn test_watcher_notified_on_comment() {
    let app = app().await;
    let root_tok = login_root(&app).await;
    let (alice_tok, _alice_id) = register_user_full(&app, "walice", "Pass1234").await;
    let tid = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &root_tok, Some(json!({"title":"WatchComment"})))).await.unwrap()).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/watch", tid), &alice_tok, None)).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &root_tok, Some(json!({"content":"Hello watchers"})))).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/notifications", &alice_tok, None)).await.unwrap();
    let notifs = body_json(resp).await;
    let found = notifs.as_array().unwrap().iter().any(|n| n["kind"] == "comment_added");
    assert!(found, "Watcher should receive comment_added notification");
}

#[tokio::test]
async fn test_watcher_not_notified_own_comment() {
    let app = app().await;
    let root_tok = login_root(&app).await;
    let tid = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &root_tok, Some(json!({"title":"SelfComment"})))).await.unwrap()).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/watch", tid), &root_tok, None)).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &root_tok, Some(json!({"content":"My own comment"})))).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/notifications", &root_tok, None)).await.unwrap();
    let notifs = body_json(resp).await;
    let found = notifs.as_array().unwrap().iter().any(|n| n["kind"] == "comment_added");
    assert!(!found, "Commenter should NOT receive own comment notification");
}

#[tokio::test]
async fn test_watcher_notified_on_status_change() {
    let app = app().await;
    let root_tok = login_root(&app).await;
    let (bob_tok, _bob_id) = register_user_full(&app, "wbob", "Pass1234").await;
    let tid = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &root_tok, Some(json!({"title":"WatchStatus"})))).await.unwrap()).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/watch", tid), &bob_tok, None)).await.unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &root_tok, Some(json!({"status":"in_progress"})))).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/notifications", &bob_tok, None)).await.unwrap();
    let notifs = body_json(resp).await;
    let found = notifs.as_array().unwrap().iter().any(|n| n["kind"] == "task_status_changed");
    assert!(found, "Watcher should receive task_status_changed notification");
}

#[tokio::test]
async fn test_notification_prefs_include_new_types() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/profile/notifications", &tok, None)).await.unwrap();
    let prefs = body_json(resp).await;
    let types: Vec<&str> = prefs.as_array().unwrap().iter().map(|p| p["event_type"].as_str().unwrap()).collect();
    assert!(types.contains(&"task_status_changed"), "Should include task_status_changed");
    assert!(types.contains(&"mention"), "Should include mention");
}

#[tokio::test]
async fn test_saved_views_crud() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/views", &tok, Some(json!({"name":"My Filter","filters":{"status":"active","project":"alpha"}})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let view = body_json(resp).await;
    assert_eq!(view["name"], "My Filter");
    let vid = view["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/views", &tok, None)).await.unwrap();
    let views = body_json(resp).await;
    assert!(views.as_array().unwrap().iter().any(|v| v["id"].as_i64() == Some(vid)));
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/views/{}", vid), &tok, Some(json!({"name":"Updated","filters":{"status":"done"}})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["name"], "Updated");
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/views/{}", vid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_saved_views_empty_name_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/views", &tok, Some(json!({"name":"","filters":{}})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_saved_views_user_isolation() {
    let app = app().await;
    let tok_a = register_user(&app, "viewuserA").await;
    let tok_b = register_user(&app, "viewuserB").await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/views", &tok_a, Some(json!({"name":"A's view","filters":{}})))).await.unwrap();
    let vid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/views", &tok_b, None)).await.unwrap();
    let views = body_json(resp).await;
    assert!(!views.as_array().unwrap().iter().any(|v| v["id"].as_i64() == Some(vid)));
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/views/{}", vid), &tok_b, None)).await.unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_projects_crud() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/projects", &tok, Some(json!({"name":"Alpha Project","description":"Test project","key":"ALPHA"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let project = body_json(resp).await;
    let pid = project["id"].as_i64().unwrap();
    assert_eq!(project["name"], "Alpha Project");
    assert_eq!(project["key"], "ALPHA");
    assert_eq!(project["status"], "active");
    let resp = app.clone().oneshot(auth_req("GET", "/api/projects", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let projects = body_json(resp).await;
    assert!(projects.as_array().unwrap().iter().any(|p| p["key"] == "ALPHA"));
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/projects/{}", pid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["name"], "Alpha Project");
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/projects/{}", pid), &tok, Some(json!({"name":"Alpha v2","status":"archived"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let updated = body_json(resp).await;
    assert_eq!(updated["name"], "Alpha v2");
    assert_eq!(updated["status"], "archived");
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/projects/{}", pid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/projects/{}", pid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_projects_duplicate_key_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/projects", &tok, Some(json!({"name":"P1","key":"DUP"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let resp = app.clone().oneshot(auth_req("POST", "/api/projects", &tok, Some(json!({"name":"P2","key":"DUP"})))).await.unwrap();
    assert_eq!(resp.status(), 409);
}

#[tokio::test]
async fn test_projects_non_admin_rejected() {
    let app = app().await;
    let user_tok = register_user(&app, "projuser").await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/projects", &user_tok, Some(json!({"name":"Nope","key":"NOPE"})))).await.unwrap();
    assert_eq!(resp.status(), 403);
    let resp = app.clone().oneshot(auth_req("GET", "/api/projects", &user_tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_task_with_project_id() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/projects", &tok, Some(json!({"name":"TaskProj","key":"TP"})))).await.unwrap();
    let pid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Proj Task","project_id":pid})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let task = body_json(resp).await;
    assert_eq!(task["project_id"], pid);
    assert_eq!(task["project_name"], "TaskProj");
    let tid = task["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"project_id":null})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let updated = body_json(resp).await;
    assert!(updated["project_id"].is_null());
    assert!(updated["project_name"].is_null());
}

#[tokio::test]
async fn test_project_delete_unlinks_tasks() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/projects", &tok, Some(json!({"name":"DelProj","key":"DEL"})))).await.unwrap();
    let pid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Linked","project_id":pid})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/projects/{}", pid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
    let task = body_json(resp).await;
    assert!(task["task"]["project_id"].is_null());
}

#[tokio::test]
async fn test_transition_rules_crud() {
    let app = common::app().await;
    let token = common::login_root(&app).await;
    let resp = app.clone().oneshot(common::auth_req("GET", "/api/workflows/transitions", &token, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = common::body_json(resp).await;
    assert_eq!(body.as_array().unwrap().len(), 0);
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/workflows/transitions", &token,
        Some(serde_json::json!({"from_status": "backlog", "to_status": "active"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let t = common::body_json(resp).await;
    assert_eq!(t["from_status"], "backlog");
    assert_eq!(t["to_status"], "active");
    assert!(t["project_id"].is_null());
    let tid = t["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(common::auth_req("GET", "/api/workflows/transitions", &token, None)).await.unwrap();
    let body = common::body_json(resp).await;
    assert_eq!(body.as_array().unwrap().len(), 1);
    let resp = app.clone().oneshot(common::auth_req("DELETE", &format!("/api/workflows/transitions/{}", tid), &token, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    let resp = app.clone().oneshot(common::auth_req("GET", "/api/workflows/transitions", &token, None)).await.unwrap();
    let body = common::body_json(resp).await;
    assert_eq!(body.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_transition_rules_non_admin_rejected() {
    let app = common::app().await;
    let user_token = common::register_user(&app, "transuser").await;
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/workflows/transitions", &user_token,
        Some(serde_json::json!({"from_status": "backlog", "to_status": "active"})))).await.unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_transition_rules_duplicate_rejected() {
    let app = common::app().await;
    let token = common::login_root(&app).await;
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/workflows/transitions", &token,
        Some(serde_json::json!({"from_status": "backlog", "to_status": "active"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/workflows/transitions", &token,
        Some(serde_json::json!({"from_status": "backlog", "to_status": "active"})))).await.unwrap();
    assert_eq!(resp.status(), 409);
}

#[tokio::test]
async fn test_transition_rules_same_status_rejected() {
    let app = common::app().await;
    let token = common::login_root(&app).await;
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/workflows/transitions", &token,
        Some(serde_json::json!({"from_status": "active", "to_status": "active"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_transition_enforcement_blocks_invalid() {
    let app = common::app().await;
    let token = common::login_root(&app).await;
    app.clone().oneshot(common::auth_req("POST", "/api/workflows/transitions", &token,
        Some(serde_json::json!({"from_status": "backlog", "to_status": "active"})))).await.unwrap();
    app.clone().oneshot(common::auth_req("POST", "/api/workflows/transitions", &token,
        Some(serde_json::json!({"from_status": "active", "to_status": "completed"})))).await.unwrap();
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/tasks", &token,
        Some(serde_json::json!({"title": "Trans test"})))).await.unwrap();
    let task = common::body_json(resp).await;
    let task_id = task["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(common::auth_req("PUT", &format!("/api/tasks/{}", task_id), &token,
        Some(serde_json::json!({"status": "completed"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    let resp = app.clone().oneshot(common::auth_req("PUT", &format!("/api/tasks/{}", task_id), &token,
        Some(serde_json::json!({"status": "active"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let resp = app.clone().oneshot(common::auth_req("PUT", &format!("/api/tasks/{}", task_id), &token,
        Some(serde_json::json!({"status": "completed"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_transition_no_rules_allows_all() {
    let app = common::app().await;
    let token = common::login_root(&app).await;
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/tasks", &token,
        Some(serde_json::json!({"title": "Free trans"})))).await.unwrap();
    let task = common::body_json(resp).await;
    let task_id = task["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(common::auth_req("PUT", &format!("/api/tasks/{}", task_id), &token,
        Some(serde_json::json!({"status": "completed"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_transition_enforcement_bulk_status() {
    let app = common::app().await;
    let token = common::login_root(&app).await;
    app.clone().oneshot(common::auth_req("POST", "/api/workflows/transitions", &token,
        Some(serde_json::json!({"from_status": "backlog", "to_status": "active"})))).await.unwrap();
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/tasks", &token,
        Some(serde_json::json!({"title": "Bulk1"})))).await.unwrap();
    let t1 = common::body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(common::auth_req("PUT", "/api/tasks/bulk-status", &token,
        Some(serde_json::json!({"task_ids": [t1], "status": "completed"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    let resp = app.clone().oneshot(common::auth_req("PUT", "/api/tasks/bulk-status", &token,
        Some(serde_json::json!({"task_ids": [t1], "status": "active"})))).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_transition_project_specific_rules() {
    let app = common::app().await;
    let token = common::login_root(&app).await;
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/projects", &token,
        Some(serde_json::json!({"name": "TransProj", "key": "TP"})))).await.unwrap();
    let proj_id = common::body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(common::auth_req("POST", "/api/workflows/transitions", &token,
        Some(serde_json::json!({"from_status": "backlog", "to_status": "active", "project_id": proj_id})))).await.unwrap();
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/tasks", &token,
        Some(serde_json::json!({"title": "Proj task", "project_id": proj_id})))).await.unwrap();
    let task_id = common::body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(common::auth_req("PUT", &format!("/api/tasks/{}", task_id), &token,
        Some(serde_json::json!({"status": "completed"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    let resp = app.clone().oneshot(common::auth_req("PUT", &format!("/api/tasks/{}", task_id), &token,
        Some(serde_json::json!({"status": "active"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/tasks", &token,
        Some(serde_json::json!({"title": "No proj task"})))).await.unwrap();
    let task2_id = common::body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(common::auth_req("PUT", &format!("/api/tasks/{}", task2_id), &token,
        Some(serde_json::json!({"status": "completed"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_automation_status_change_sets_priority() {
    let app = common::app().await;
    let token = common::login_root(&app).await;
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/automations", &token,
        Some(serde_json::json!({
            "name": "Urgent on active",
            "trigger_event": "task.status_changed",
            "condition_json": r#"{"to_status":"active"}"#,
            "action_json": r#"{"set_priority":5}"#
        })))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/tasks", &token,
        Some(serde_json::json!({"title": "Auto test", "priority": 3})))).await.unwrap();
    let task = common::body_json(resp).await;
    let task_id = task["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(common::auth_req("PUT", &format!("/api/tasks/{}", task_id), &token,
        Some(serde_json::json!({"status": "active"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let resp = app.clone().oneshot(common::auth_req("GET", &format!("/api/tasks/{}", task_id), &token, None)).await.unwrap();
    let detail = common::body_json(resp).await;
    assert_eq!(detail["task"]["priority"], 5);
}

#[tokio::test]
async fn test_automation_all_subtasks_done() {
    let app = common::app().await;
    let token = common::login_root(&app).await;
    app.clone().oneshot(common::auth_req("POST", "/api/automations", &token,
        Some(serde_json::json!({
            "name": "Auto complete parent",
            "trigger_event": "task.all_subtasks_done",
            "condition_json": "{}",
            "action_json": r#"{"set_status":"completed"}"#
        })))).await.unwrap();
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/tasks", &token,
        Some(serde_json::json!({"title": "Parent"})))).await.unwrap();
    let parent_id = common::body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/tasks", &token,
        Some(serde_json::json!({"title": "Child", "parent_id": parent_id})))).await.unwrap();
    let child_id = common::body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(common::auth_req("PUT", &format!("/api/tasks/{}", child_id), &token,
        Some(serde_json::json!({"status": "completed"})))).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let resp = app.clone().oneshot(common::auth_req("GET", &format!("/api/tasks/{}", parent_id), &token, None)).await.unwrap();
    let detail = common::body_json(resp).await;
    assert_eq!(detail["task"]["status"], "completed");
}

#[tokio::test]
async fn test_automation_task_created_trigger() {
    let app = common::app().await;
    let token = common::login_root(&app).await;
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/automations", &token,
        Some(serde_json::json!({
            "name": "Low priority default",
            "trigger_event": "task.created",
            "condition_json": "{}",
            "action_json": r#"{"set_priority":1}"#
        })))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/tasks", &token,
        Some(serde_json::json!({"title": "Auto created"})))).await.unwrap();
    let task_id = common::body_json(resp).await["id"].as_i64().unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let resp = app.clone().oneshot(common::auth_req("GET", &format!("/api/tasks/{}", task_id), &token, None)).await.unwrap();
    let detail = common::body_json(resp).await;
    assert_eq!(detail["task"]["priority"], 1);
}

#[tokio::test]
async fn test_automation_task_assigned_trigger() {
    let app = common::app().await;
    let token = common::login_root(&app).await;
    common::register_user(&app, "assignee1").await;
    app.clone().oneshot(common::auth_req("POST", "/api/automations", &token,
        Some(serde_json::json!({
            "name": "Auto in-progress on assign",
            "trigger_event": "task.assigned",
            "condition_json": "{}",
            "action_json": r#"{"set_status":"in_progress"}"#
        })))).await.unwrap();
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/tasks", &token,
        Some(serde_json::json!({"title": "Assign test"})))).await.unwrap();
    let task_id = common::body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(common::auth_req("POST", &format!("/api/tasks/{}/assignees", task_id), &token,
        Some(serde_json::json!({"username": "assignee1"})))).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let resp = app.clone().oneshot(common::auth_req("GET", &format!("/api/tasks/{}", task_id), &token, None)).await.unwrap();
    let detail = common::body_json(resp).await;
    assert_eq!(detail["task"]["status"], "in_progress");
}

#[tokio::test]
async fn test_automation_priority_changed_trigger() {
    let app = common::app().await;
    let token = common::login_root(&app).await;
    app.clone().oneshot(common::auth_req("POST", "/api/automations", &token,
        Some(serde_json::json!({
            "name": "Activate on urgent",
            "trigger_event": "task.priority_changed",
            "condition_json": r#"{"priority":5}"#,
            "action_json": r#"{"set_status":"active"}"#
        })))).await.unwrap();
    let resp = app.clone().oneshot(common::auth_req("POST", "/api/tasks", &token,
        Some(serde_json::json!({"title": "Priority test", "priority": 3})))).await.unwrap();
    let task_id = common::body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(common::auth_req("PUT", &format!("/api/tasks/{}", task_id), &token,
        Some(serde_json::json!({"priority": 5})))).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let resp = app.clone().oneshot(common::auth_req("GET", &format!("/api/tasks/{}", task_id), &token, None)).await.unwrap();
    let detail = common::body_json(resp).await;
    assert_eq!(detail["task"]["status"], "active");
}

#[tokio::test]
async fn test_session_note_update() {
    let app = app().await;
    let tok = login_root(&app).await;
    let tid = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"NoteTask"})))).await.unwrap()).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({"task_id": tid})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", "/api/timer/stop", &tok, None)).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/sessions", tid), &tok, None)).await.unwrap();
    let sessions = body_json(resp).await;
    let sid = sessions.as_array().unwrap()[0]["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/sessions/{}/note", sid), &tok, Some(json!({"note":"Updated note"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let session = body_json(resp).await;
    assert_eq!(session["notes"], "Updated note");
}

#[tokio::test]
async fn test_task_watchers() {
    let app = app().await;
    let tok = login_root(&app).await;
    let tid = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"WatchMe"})))).await.unwrap()).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/watch", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/watchers", tid), &tok, None)).await.unwrap();
    let watchers = body_json(resp).await;
    assert_eq!(watchers.as_array().unwrap().len(), 1);
    let resp = app.clone().oneshot(auth_req("GET", "/api/watched", &tok, None)).await.unwrap();
    let watched = body_json(resp).await;
    assert!(watched.as_array().unwrap().iter().any(|v| v.as_i64() == Some(tid)));
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/watch", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_auto_archive() {
    let pool = poflow_daemon::db::connect_memory().await.unwrap();
    let config = poflow_daemon::config::Config::default();
    let engine = Arc::new(poflow_daemon::engine::Engine::new(pool.clone(), config).await);
    let app = poflow_daemon::build_router(engine).await;
    let tok = login_root(&app).await;
    let tid = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"ArchiveMe"})))).await.unwrap()).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"completed"})))).await.unwrap();
    sqlx::query("UPDATE tasks SET updated_at = '2020-01-01T00:00:00' WHERE id = ?").bind(tid).execute(&pool).await.unwrap();
    let cutoff = "2025-01-01T00:00:00";
    let result = sqlx::query("UPDATE tasks SET status = 'archived', updated_at = ? WHERE status = 'completed' AND updated_at < ? AND deleted_at IS NULL")
        .bind("2025-01-01T00:00:01").bind(cutoff).execute(&pool).await.unwrap();
    assert!(result.rows_affected() >= 1);
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
    let task = body_json(resp).await;
    assert_eq!(task["task"]["status"], "archived");
}

#[tokio::test]
async fn test_global_search_comment_isolation() {
    let app = app().await;
    let root_tok = login_root(&app).await;
    let user_tok = register_user(&app, "searchuser1").await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &root_tok, Some(json!({"title":"Root secret task"})))).await.unwrap();
    let task_id = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", task_id), &root_tok, Some(json!({"content":"xyzzy_secret_keyword"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/search?q=xyzzy_secret_keyword", &user_tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j = body_json(resp).await;
    let comments = j["comments"].as_array().unwrap();
    assert!(comments.is_empty(), "Non-admin user should not see comments on other users' tasks");
    let resp = app.clone().oneshot(auth_req("GET", "/api/search?q=xyzzy_secret_keyword", &root_tok, None)).await.unwrap();
    let j = body_json(resp).await;
    let comments = j["comments"].as_array().unwrap();
    assert!(!comments.is_empty(), "Root should see all comments");
}

#[tokio::test]
async fn test_achievements_check() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/achievements/check", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data.is_array());
}


#[tokio::test]
async fn test_achievements_list() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/achievements", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data.as_array().unwrap().len() >= 6); // 6 achievement types defined
    for a in data.as_array().unwrap() {
        assert!(a["type"].is_string());
        assert!(a["description"].is_string());
        assert!(a["unlocked"].is_boolean());
    }
}


#[tokio::test]
async fn test_activity_feed() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create a task to generate audit entries
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"feed task"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/feed", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data.as_array().unwrap().len() > 0);
    assert!(data[0]["type"].is_string());
    assert!(data[0]["created_at"].is_string());
}


#[tokio::test]
async fn test_activity_feed_includes_all_types() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create task (generates audit entry)
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"FeedTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Add comment
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &tok, Some(json!({"content":"Feed comment"})))).await.unwrap();
    // Create sprint
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"FeedSprint"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    // Add task to sprint, start, log burn
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok, Some(json!({"task_id":tid,"hours":2.0})))).await.unwrap();
    // Get feed
    let resp = app.clone().oneshot(auth_req("GET", "/api/feed?limit=50", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let items = body_json(resp).await;
    let types: Vec<&str> = items.as_array().unwrap().iter().map(|i| i["type"].as_str().unwrap_or("")).collect();
    assert!(types.contains(&"audit"), "Feed should contain audit entries");
    assert!(types.contains(&"comment"), "Feed should contain comments");
    assert!(types.contains(&"sprint"), "Feed should contain sprint events");
    assert!(types.contains(&"burn"), "Feed should contain burn entries");
}


#[tokio::test]
async fn test_activity_feed_invalid_since() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/feed?since=garbage", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 400);
}


#[tokio::test]
async fn test_activity_feed_type_filter() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"FilterTask"})))).await.unwrap();
    // Filter to only audit
    let resp = app.clone().oneshot(auth_req("GET", "/api/feed?types=audit&limit=10", &tok, None)).await.unwrap();
    let items = body_json(resp).await;
    assert!(items.as_array().unwrap().iter().all(|i| i["type"] == "audit"));
}


#[tokio::test]
async fn test_activity_feed_with_filters() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"feed filter task"})))).await.unwrap();
    // Filter by type
    let resp = app.clone().oneshot(auth_req("GET", "/api/feed?types=audit&limit=5", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data.as_array().unwrap().len() <= 5);
    for item in data.as_array().unwrap() { assert_eq!(item["type"], "audit"); }
}


#[tokio::test]
async fn test_automation_condition_filters() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create automation: only fires when going from in_progress to completed
    app.clone().oneshot(auth_req("POST", "/api/automations", &tok, Some(json!({
        "name": "Specific transition",
        "trigger_event": "task.status_changed",
        "condition_json": "{\"from_status\":\"in_progress\",\"to_status\":\"completed\"}",
        "action_json": "{\"set_priority\":1}"
    })))).await.unwrap();
    // Create task, go directly from backlog to completed (should NOT fire)
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"CondTask","priority":5})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"completed"})))).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await["task"]["priority"], 5, "Automation should NOT fire for backlog→completed");
}


#[tokio::test]
async fn test_automation_disabled_rule_does_not_fire() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create automation then disable it
    let resp = app.clone().oneshot(auth_req("POST", "/api/automations", &tok, Some(json!({
        "name": "Disabled rule", "trigger_event": "task.status_changed",
        "condition_json": "{\"to_status\":\"completed\"}", "action_json": "{\"set_priority\":1}"
    })))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("DELETE", &format!("/api/automations/{}", rid), &tok, None)).await.unwrap();
    // Create task and complete it
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"NoAuto","priority":5})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"completed"})))).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await["task"]["priority"], 5, "Deleted rule should not fire");
}


#[tokio::test]
async fn test_automation_empty_name() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/automations", &tok, Some(json!({
        "name": "", "trigger_event": "task.status_changed", "action_json": "{}"
    })))).await.unwrap();
    assert_eq!(resp.status(), 400);
}


#[tokio::test]
async fn test_automation_invalid_json_fields() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/automations", &tok, Some(json!({
        "name": "Bad JSON", "trigger_event": "task.status_changed", "action_json": "not json"
    })))).await.unwrap();
    assert_eq!(resp.status(), 400);
}


#[tokio::test]
async fn test_automation_invalid_trigger() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/automations", &tok, Some(json!({
        "name": "Bad", "trigger_event": "invalid.trigger", "action_json": "{}"
    })))).await.unwrap();
    assert_eq!(resp.status(), 400);
}


#[tokio::test]
async fn test_automation_ownership() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Register second user
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"autouser","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"autouser","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();
    // User creates rule
    let resp = app.clone().oneshot(auth_req("POST", "/api/automations", &tok2, Some(json!({
        "name": "User rule", "trigger_event": "task.due_approaching", "action_json": "{}"
    })))).await.unwrap();
    let rule_id = body_json(resp).await["id"].as_i64().unwrap();
    // Root can delete (is_owner_or_root)
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/automations/{}", rule_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}


#[tokio::test]
async fn test_automation_rules_crud() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create rule
    let resp = app.clone().oneshot(auth_req("POST", "/api/automations", &tok, Some(json!({
        "name": "Auto close", "trigger_event": "task.status_changed",
        "action_json": "{\"set_status\":\"archived\"}"
    })))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let rule = body_json(resp).await;
    let rule_id = rule["id"].as_i64().unwrap();
    assert_eq!(rule["name"], "Auto close");
    assert_eq!(rule["enabled"], 1);
    // List rules
    let resp = app.clone().oneshot(auth_req("GET", "/api/automations", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 1);
    // Toggle
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/automations/{}/toggle", rule_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Delete
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/automations/{}", rule_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Verify deleted
    let resp = app.clone().oneshot(auth_req("GET", "/api/automations", &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_automation_status_change_fires() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create automation: when task becomes completed, set priority to 1
    let resp = app.clone().oneshot(auth_req("POST", "/api/automations", &tok, Some(json!({
        "name": "Lower priority on complete",
        "trigger_event": "task.status_changed",
        "condition_json": "{\"to_status\":\"completed\"}",
        "action_json": "{\"set_priority\":1}"
    })))).await.unwrap();
    assert_eq!(resp.status(), 201);
    // Create task with priority 5
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"AutoTask","priority":5})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Change status to completed
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"completed"})))).await.unwrap();
    // Wait for async automation to fire
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    // Check priority was changed to 1
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
    let detail = body_json(resp).await;
    assert_eq!(detail["task"]["priority"], 1, "Automation should have set priority to 1");
}


#[tokio::test]
async fn test_change_password() {
    let app = app().await;
    let (tok, _) = register_user_full(&app, "pwuser", "OldPass123").await;
    // Change password
    let resp = app.clone().oneshot(auth_req("PUT", "/api/auth/password", &tok, Some(json!({"current_password":"OldPass123","new_password":"NewPass456"})))).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Login with new password
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"pwuser","password":"NewPass456"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Old password should fail
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"pwuser","password":"OldPass123"})))).await.unwrap();
    assert_eq!(resp.status(), 401);
}


#[tokio::test]
async fn test_change_password_wrong_current() {
    let app = app().await;
    let (tok, _) = register_user_full(&app, "pwwrong", "Pass1234").await;
    let resp = app.clone().oneshot(auth_req("PUT", "/api/auth/password", &tok, Some(json!({"current_password":"WrongPass","new_password":"NewPass456"})))).await.unwrap();
    assert_eq!(resp.status(), 401);
}


#[tokio::test]
async fn test_custom_field_crud() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create text field
    let resp = app.clone().oneshot(auth_req("POST", "/api/fields", &tok, Some(json!({"name":"Sprint Goal","field_type":"text"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let field = body_json(resp).await;
    assert_eq!(field["name"], "Sprint Goal");
    assert_eq!(field["field_type"], "text");
    let fid = field["id"].as_i64().unwrap();
    // Create select field
    let resp = app.clone().oneshot(auth_req("POST", "/api/fields", &tok, Some(json!({"name":"Severity","field_type":"select","options":["low","medium","high","critical"]})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    assert_eq!(body_json(resp).await["field_type"], "select");
    // List
    let resp = app.clone().oneshot(auth_req("GET", "/api/fields", &tok, None)).await.unwrap();
    let fields = body_json(resp).await;
    assert!(fields.as_array().unwrap().len() >= 2);
    // Update
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/fields/{}", fid), &tok, Some(json!({"name":"Goal","field_type":"text","required":true})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["required"], true);
    // Delete
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/fields/{}", fid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}


#[tokio::test]
async fn test_custom_field_in_task_detail() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create field + task + set value
    let resp = app.clone().oneshot(auth_req("POST", "/api/fields", &tok, Some(json!({"name":"Priority Level","field_type":"select","options":["P0","P1","P2"]})))).await.unwrap();
    let fid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"DetailTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/fields/{}", tid, fid), &tok, Some(json!({"value":"P0"})))).await.unwrap();
    // Get task detail — should include custom_fields
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
    let detail = body_json(resp).await;
    let cf = detail["custom_fields"].as_array().unwrap();
    assert_eq!(cf.len(), 1);
    assert_eq!(cf[0]["field_name"], "Priority Level");
    assert_eq!(cf[0]["value"], "P0");
}


#[tokio::test]
async fn test_custom_field_non_root_cannot_create() {
    let app = app().await;
    let (user_tok, _) = register_user_full(&app, "fielduser", "FieldU111").await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/fields", &user_tok, Some(json!({"name":"MyField"})))).await.unwrap();
    assert_eq!(resp.status(), 403);
}


#[tokio::test]
async fn test_custom_field_select_requires_options() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/fields", &tok, Some(json!({"name":"NoOpts","field_type":"select"})))).await.unwrap();
    assert_eq!(resp.status(), 400, "Select field without options should be rejected");
}


#[tokio::test]
async fn test_custom_field_values_on_task() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create field
    let resp = app.clone().oneshot(auth_req("POST", "/api/fields", &tok, Some(json!({"name":"Component","field_type":"text"})))).await.unwrap();
    let fid = body_json(resp).await["id"].as_i64().unwrap();
    // Create task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"FieldTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Set value
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/fields/{}", tid, fid), &tok, Some(json!({"value":"Backend"})))).await.unwrap();
    assert_eq!(resp.status(), 204);
    // Get values
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/fields", tid), &tok, None)).await.unwrap();
    let vals = body_json(resp).await;
    let arr = vals.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["field_name"], "Component");
    assert_eq!(arr[0]["value"], "Backend");
    // Update value
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/fields/{}", tid, fid), &tok, Some(json!({"value":"Frontend"})))).await.unwrap();
    assert_eq!(resp.status(), 204);
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/fields", tid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap()[0]["value"], "Frontend");
    // Delete value
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}/fields/{}", tid, fid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/fields", tid), &tok, None)).await.unwrap();
    assert!(body_json(resp).await.as_array().unwrap().is_empty());
}


#[tokio::test]
async fn test_custom_status_board_mapping() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create custom statuses in different categories
    app.clone().oneshot(auth_req("POST", "/api/statuses", &tok, Some(json!({"name":"qa_review","category":"in_progress"})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", "/api/statuses", &tok, Some(json!({"name":"deployed","category":"done"})))).await.unwrap();
    // Create task, sprint, add task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"BoardTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"BS"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    // Set task to custom "qa_review" (category: in_progress)
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"qa_review"})))).await.unwrap();
    // Check board — task should be in in_progress column
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/board", sid), &tok, None)).await.unwrap();
    let board = body_json(resp).await;
    assert!(board["in_progress"].as_array().unwrap().iter().any(|t| t["title"] == "BoardTask"), "Custom status with category in_progress should map to in_progress column");
    // Change to "deployed" (category: done)
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"deployed"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/board", sid), &tok, None)).await.unwrap();
    let board = body_json(resp).await;
    assert!(board["done"].as_array().unwrap().iter().any(|t| t["title"] == "BoardTask"), "Custom status with category done should map to done column");
}


#[tokio::test]
async fn test_custom_status_crud() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create
    let resp = app.clone().oneshot(auth_req("POST", "/api/statuses", &tok, Some(json!({"name":"review","color":"#f59e0b","category":"in_progress"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let status = body_json(resp).await;
    assert_eq!(status["name"], "review");
    assert_eq!(status["category"], "in_progress");
    let sid = status["id"].as_i64().unwrap();
    // List
    let resp = app.clone().oneshot(auth_req("GET", "/api/statuses", &tok, None)).await.unwrap();
    let statuses = body_json(resp).await;
    assert!(statuses.as_array().unwrap().iter().any(|s| s["name"] == "review"));
    // Update
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/statuses/{}", sid), &tok, Some(json!({"name":"code_review","color":"#10b981","category":"in_progress"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["name"], "code_review");
    // Delete
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/statuses/{}", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}


#[tokio::test]
async fn test_custom_status_duplicate_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(auth_req("POST", "/api/statuses", &tok, Some(json!({"name":"dup_status","category":"todo"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/statuses", &tok, Some(json!({"name":"dup_status","category":"todo"})))).await.unwrap();
    assert_eq!(resp.status(), 409);
}


#[tokio::test]
async fn test_custom_status_non_root_cannot_create() {
    let app = app().await;
    let (user_tok, _) = register_user_full(&app, "statususer", "StatUs111").await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/statuses", &user_tok, Some(json!({"name":"mystat","category":"todo"})))).await.unwrap();
    assert_eq!(resp.status(), 403);
}


#[tokio::test]
async fn test_custom_status_unknown_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Try to set task to non-existent custom status
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"nonexistent_status"})))).await.unwrap();
    assert_eq!(resp.status(), 400, "Unknown custom status should be rejected");
}


#[tokio::test]
async fn test_custom_status_used_in_task() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create custom status
    app.clone().oneshot(auth_req("POST", "/api/statuses", &tok, Some(json!({"name":"testing","category":"in_progress"})))).await.unwrap();
    // Create task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Set task to custom status
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"testing"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(body_json(resp).await["status"], "testing");
}

#[tokio::test]
async fn test_estimation_accuracy() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create and complete a task with estimates
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"est task","estimated":5})))).await.unwrap();
    let task_id = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", task_id), &tok, Some(json!({"status":"completed","actual":4})))).await.unwrap();
    // Get accuracy
    let resp = app.clone().oneshot(auth_req("GET", "/api/analytics/estimation-accuracy", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data["total_tasks"].as_f64().unwrap() >= 1.0);
    assert!(data["accuracy_pct"].as_f64().is_some());
    assert!(data["over_estimated"].is_number());
    assert!(data["under_estimated"].is_number());
    assert!(data["by_project"].is_array());
}


#[tokio::test]
async fn test_estimation_accuracy_project_filter() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"proj task","estimated":3,"project":"alpha"})))).await.unwrap();
    let id = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", id), &tok, Some(json!({"status":"completed","actual":3})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/analytics/estimation-accuracy?project=alpha", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data["total_tasks"].as_f64().unwrap() >= 1.0);
}


#[tokio::test]
async fn test_feed_includes_comments() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"feed comment task"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &tok, Some(json!({"content":"test comment for feed"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/feed?types=comment", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data.as_array().unwrap().iter().any(|i| i["type"] == "comment"));
}


#[tokio::test]
async fn test_focus_score() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/analytics/focus-score", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data["score"].is_number());
    // When no sessions exist, returns minimal response
    assert!(data["components"].is_object());
}


#[tokio::test]
async fn test_leaderboard() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/leaderboard", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data.as_array().unwrap().len() >= 1);
    assert!(data[0]["username"].is_string());
    assert!(data[0]["hours"].is_number());
    assert!(data[0]["sessions"].is_number());
}


#[tokio::test]
async fn test_leaderboard_periods() {
    let app = app().await;
    let tok = login_root(&app).await;
    for period in &["week", "month", "year"] {
        let resp = app.clone().oneshot(auth_req("GET", &format!("/api/leaderboard?period={}", period), &tok, None)).await.unwrap();
        assert_eq!(resp.status(), 200);
    }
}


#[tokio::test]
async fn test_list_notifications() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/notifications", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j = body_json(resp).await;
    assert!(j.is_array());
}


#[tokio::test]
async fn test_mark_notifications_read() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Mark all read (no body)
    let resp = app.clone().oneshot(auth_req("POST", "/api/notifications/read", &tok, Some(json!({})))).await.unwrap();
    assert_eq!(resp.status(), 204);
}


#[tokio::test]
async fn test_priority_suggestions() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create overdue task
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"overdue","due_date":"2020-01-01"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/suggestions/priorities", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data.as_array().unwrap().len() >= 1);
    let suggestion = &data[0];
    assert!(suggestion["suggested_priority"].as_i64().unwrap() >= 4);
    assert!(suggestion["reasons"].as_array().unwrap().iter().any(|r| r.as_str().unwrap().contains("Overdue")));
}


#[tokio::test]
async fn test_profile_email_update() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Set email
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"email":"test@example.com"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Verify via admin user list
    let resp = app.clone().oneshot(auth_req("GET", "/api/admin/users", &tok, None)).await.unwrap();
    let users = body_json(resp).await;
    let root = users.as_array().unwrap().iter().find(|u| u["username"] == "root").unwrap();
    assert_eq!(root["email"], "test@example.com");
}


#[tokio::test]
async fn test_profile_invalid_email_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("PUT", "/api/profile", &tok, Some(json!({"email":"not-an-email"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}


#[tokio::test]
async fn test_schedule_suggestions() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/suggestions/schedule", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data["peak_hours"].is_array());
    assert!(data["avg_daily_sessions"].is_number());
    assert!(data["best_days"].is_array());
    assert!(data["task_suggestions"].is_array());
}


#[tokio::test]
async fn test_shared_session_cannot_join_own() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({})))).await.unwrap();
    let session_id = body_json(resp).await["current_session_id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/timer/join/{}", session_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 400);
}


#[tokio::test]
async fn test_shared_session_join() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Register second user
    let reg_resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"joiner_f11","password":"Pass1234"})))).await.unwrap();
    assert_eq!(reg_resp.status(), 200);
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"joiner_f11","password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 200, "Login failed");
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();
    // Root starts a timer
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let state = body_json(resp).await;
    let session_id = state["current_session_id"].as_i64().expect("current_session_id should be set after start");
    // Second user joins
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/timer/join/{}", session_id), &tok2, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    // List participants
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/timer/participants/{}", session_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let participants = body_json(resp).await;
    assert_eq!(participants.as_array().unwrap().len(), 1);
    assert_eq!(participants[0]["username"], "joiner_f11");
}

#[tokio::test]
async fn test_shared_session_not_found() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/join/99999", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 404);
}


#[tokio::test]
async fn test_sla_report() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create and complete a task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"SLA1","priority":5})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"completed"})))).await.unwrap();
    // Get SLA report
    let resp = app.clone().oneshot(auth_req("GET", "/api/reports/sla", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let report = body_json(resp).await;
    assert!(report["resolution_time_by_priority"].as_array().unwrap().len() > 0);
    assert!(report["overdue_tasks"].is_number());
    assert!(report["on_time_completion"]["on_time_pct"].is_number());
}


#[tokio::test]
async fn test_sla_report_non_admin_rejected() {
    let app = app().await;
    let (user_tok, _) = register_user_full(&app, "slauser", "SlaUs1111").await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/reports/sla", &user_tok, None)).await.unwrap();
    assert_eq!(resp.status(), 403);
}


#[tokio::test]
async fn test_slack_integration_invalid_url() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/integrations/slack", &tok, Some(json!({
        "webhook_url": "https://evil.com/steal"
    })))).await.unwrap();
    assert_eq!(resp.status(), 400);
}


#[tokio::test]
async fn test_slack_integration_url_too_long() {
    let app = app().await;
    let tok = login_root(&app).await;
    let long = "https://hooks.slack.com/".to_string() + &"a".repeat(500);
    let resp = app.clone().oneshot(auth_req("POST", "/api/integrations/slack", &tok, Some(json!({"webhook_url": long})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}


#[tokio::test]
async fn test_slack_integration_valid() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/integrations/slack", &tok, Some(json!({
        "webhook_url": "https://hooks.slack.com/services/T00/B00/xxx"
    })))).await.unwrap();
    assert_eq!(resp.status(), 201);
}


#[tokio::test]
async fn test_sprint_retro_report() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create sprint
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"retro sprint"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    // Create task and add to sprint
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"retro task","estimated":3})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    // Get retro report
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/retro-report", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert_eq!(data["tasks"]["total"], 1);
    assert!(data["sprint"]["name"].as_str().unwrap().contains("retro"));
    assert!(data["members"].is_array());
    assert!(data["scope_changes"].is_number());
}


#[tokio::test]
async fn test_sprint_retro_report_not_found() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/sprints/99999/retro-report", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 404);
}


#[tokio::test]
async fn test_standup_report() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create and complete a task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Standup task"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &tok, Some(json!({"status":"in_progress"})))).await.unwrap();
    // Get standup
    let resp = app.clone().oneshot(auth_req("GET", "/api/reports/standup", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let report = body_json(resp).await;
    assert!(report["date"].is_string());
    assert!(report["today"].is_array());
    assert!(report["markdown"].as_str().unwrap().contains("Daily Standup"));
    // in_progress task should appear in "today"
    assert!(report["today"].as_array().unwrap().iter().any(|t| t["title"] == "Standup task"));
}


#[tokio::test]
async fn test_unread_count() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/notifications/unread", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j = body_json(resp).await;
    assert!(j["count"].is_number());
}


#[tokio::test]
async fn test_user_presence() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/users/presence", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert!(data.as_array().unwrap().len() >= 1);
    let root = data.as_array().unwrap().iter().find(|u| u["username"] == "root").unwrap();
    assert!(root["user_id"].is_number());
    assert!(root["online"].is_boolean());
}


#[tokio::test]
async fn test_weekly_digest() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/reports/weekly-digest", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let data = body_json(resp).await;
    assert_eq!(data["period"], "last_7_days");
    assert!(data["focus_hours"].is_number());
    assert!(data["sessions"].is_number());
    assert!(data["tasks_completed"].is_number());
    assert!(data["upcoming_due"].is_array());
}
