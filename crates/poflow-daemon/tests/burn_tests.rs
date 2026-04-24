use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{app, json_req, auth_req, body_json, login_root, register_user_full};

#[tokio::test]
async fn test_burn_log_and_cancel() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Setup: task + sprint
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    // BL7: Sprint must be active to log burns
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();

    // Log a burn
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok,
        Some(json!({"task_id":tid,"points":5.0,"hours":2.0,"note":"Did stuff"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let burn = body_json(resp).await;
    assert_eq!(burn["points"], 5.0);
    assert_eq!(burn["hours"], 2.0);
    assert_eq!(burn["username"], "root");
    assert_eq!(burn["source"], "manual");
    assert_eq!(burn["cancelled"], 0);
    let bid = burn["id"].as_i64().unwrap();

    // List burns
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/burns", sid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 1);

    // Summary
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/burn-summary", sid), &tok, None)).await.unwrap();
    let summary = body_json(resp).await;
    assert_eq!(summary[0]["points"], 5.0);
    assert_eq!(summary[0]["username"], "root");

    // Cancel
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/sprints/{}/burns/{}", sid, bid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let burn = body_json(resp).await;
    assert_eq!(burn["cancelled"], 1);
    assert_eq!(burn["cancelled_by"], "root");

    // Summary should be empty after cancel
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/burn-summary", sid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 0);

    // But list still shows the cancelled entry
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/burns", sid), &tok, None)).await.unwrap();
    let burns = body_json(resp).await;
    assert_eq!(burns.as_array().unwrap().len(), 1);
    assert_eq!(burns[0]["cancelled"], 1);
}

#[tokio::test]
async fn test_burn_multi_user_summary() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"bob","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"bob","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();

    // Assign Bob to the task so he can log burns
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &tok, Some(json!({"username":"bob"})))).await.unwrap();

    // Root burns 3 pts, Bob burns 5 pts
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok, Some(json!({"task_id":tid,"points":3.0})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok2, Some(json!({"task_id":tid,"points":5.0})))).await.unwrap();

    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/burn-summary", sid), &tok, None)).await.unwrap();
    let summary = body_json(resp).await;
    let arr = summary.as_array().unwrap();
    assert_eq!(arr.len(), 2); // two users
    let total: f64 = arr.iter().map(|e| e["points"].as_f64().unwrap()).sum();
    assert_eq!(total, 8.0);
}

#[tokio::test]
async fn test_burn_log_requires_authorization() {
    let app = app().await;
    let tok = login_root(&app).await;
    let (user_tok, _) = register_user_full(&app, "burnnoauth", "BurnNo111").await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"BurnTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"BurnSprint"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();
    // Unrelated user cannot log burns
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &user_tok, Some(json!({"task_id":tid,"points":5.0})))).await.unwrap();
    assert_eq!(resp.status(), 403, "Unrelated user should not be able to log burns");
    // Sprint owner can
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok, Some(json!({"task_id":tid,"points":3.0})))).await.unwrap();
    assert_eq!(resp.status(), 201);
}

#[tokio::test]
async fn test_burn_cascade_on_sprint_delete() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok, Some(json!({"task_id":tid,"points":5.0})))).await.unwrap();

    // Delete sprint — burns should cascade
    app.clone().oneshot(auth_req("DELETE", &format!("/api/sprints/{}", sid), &tok, None)).await.unwrap();

    // Task still exists
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks", &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_cancel_burn_ownership() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"burnuser","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"burnuser","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();

    // Root logs a burn
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok,
        Some(json!({"task_id":tid,"points":5.0})))).await.unwrap();
    let bid = body_json(resp).await["id"].as_i64().unwrap();

    // Non-owner cannot cancel
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/sprints/{}/burns/{}", sid, bid), &tok2, None)).await.unwrap();
    assert_eq!(resp.status(), 403);

    // Owner can cancel
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/sprints/{}/burns/{}", sid, bid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_cancel_burn_validates_sprint() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create task and sprint
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();
    // Log burn
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok,
        Some(json!({"task_id":tid,"points":1.0,"hours":0.5})))).await.unwrap();
    let burn_id = body_json(resp).await["id"].as_i64().unwrap();
    // Cancel with wrong sprint_id
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/sprints/99999/burns/{}", burn_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_burn_negative_values_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create sprint + task
    let sprint = body_json(app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap()).await;
    let task = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap()).await;
    let sid = sprint["id"].as_i64().unwrap();
    let tid = task["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    // Negative points
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok, Some(json!({"task_id":tid,"points":-5.0})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // Negative hours
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok, Some(json!({"task_id":tid,"hours":-1.0})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_time_report_zero_hours_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let task = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap()).await;
    let tid = task["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/time", tid), &tok, Some(json!({"hours":0.0})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_time_reports_auto_assign() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/time", tid), &tok,
        Some(json!({"hours":2.5,"description":"work"})))).await.unwrap();
    assert!(resp.status().is_success());

    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/time", tid), &tok, None)).await.unwrap();
    let reports = body_json(resp).await;
    assert_eq!(reports.as_array().unwrap().len(), 1);
    assert_eq!(reports[0]["hours"], 2.5);
    assert_eq!(reports[0]["source"], "time_report");

    // Burn total
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/burn-total", tid), &tok, None)).await.unwrap();
    let total = body_json(resp).await;
    assert_eq!(total["total_hours"], 2.5);

    // Auto-assigned
    let resp = app.oneshot(auth_req("GET", &format!("/api/tasks/{}/assignees", tid), &tok, None)).await.unwrap();
    let assignees = body_json(resp).await;
    assert!(assignees.as_array().unwrap().contains(&json!("root")));
}

#[tokio::test]
async fn test_time_report_links_to_active_sprint() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"S"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();

    // Add time report — should auto-link to active sprint
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/time", tid), &tok,
        Some(json!({"hours":2.0})))).await.unwrap();
    let burn = body_json(resp).await;
    assert_eq!(burn["sprint_id"], sid, "time report should link to active sprint");

    // Verify it shows in sprint burns
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/burns", sid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_burn_summary_empty() {
    let app = app().await;
    let tok = login_root(&app).await;
    let sprint = body_json(app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"SumSprint"})))).await.unwrap()).await;
    let sid = sprint["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/burn-summary", sid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let summary = body_json(resp).await;
    assert!(summary.is_array());
}

#[tokio::test]
async fn test_burn_totals_endpoint() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/burn-totals", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_export_burns_csv() {
    let app = app().await;
    let token = login_root(&app).await;
    // Create sprint
    let r = app.clone().oneshot(auth_req("POST", "/api/sprints", &token, Some(json!({"name":"BurnExport"})))).await.unwrap();
    let sid = body_json(r).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/export/burns/{}", sid), &token, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let csv = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(csv.starts_with("created_at,task_id,points,hours,username,source,note"));
}


// ============================================================
// Time tracking reports (Jira gap #6)
// ============================================================

#[tokio::test]
async fn test_time_tracking_report_json() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create task + complete a session so there's data
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"TT","project":"Proj1"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", "/api/timer/start", &tok, Some(json!({"task_id": tid})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", "/api/timer/stop", &tok, None)).await.unwrap();
    // Get report
    let resp = app.clone().oneshot(auth_req("GET", "/api/reports/time-tracking", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("json"));
}

#[tokio::test]
async fn test_time_tracking_report_csv() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/reports/time-tracking?format=csv", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("csv"));
    let body = String::from_utf8(resp.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert!(body.starts_with("username,project,week,hours,sessions"));
}

#[tokio::test]
async fn test_time_tracking_non_root_rejected() {
    let app = app().await;
    let (user_tok, _) = register_user_full(&app, "ttuser", "TtUser111").await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/reports/time-tracking", &user_tok, None)).await.unwrap();
    assert_eq!(resp.status(), 403);
}

// 7. GET /api/tasks/{id}/burn-users
#[tokio::test]
async fn test_task_burn_users() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"BurnTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/burn-users", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j = body_json(resp).await;
    assert!(j.is_array());
}

// 10. GET /api/tasks/{id}/time-summary
#[tokio::test]
async fn test_task_time_summary() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"TimeSumTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/time-summary", tid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j = body_json(resp).await;
    assert!(j["task_id"].as_i64().unwrap() == tid);
    assert!(j["total_hours"].is_number());
    assert!(j["by_user"].is_array());
}

// 11. GET /api/timer/active
#[tokio::test]
async fn test_active_timers() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/timer/active", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let j = body_json(resp).await;
    assert!(j.is_array());
}
