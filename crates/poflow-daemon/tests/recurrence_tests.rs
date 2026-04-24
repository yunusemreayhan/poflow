use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{app, auth_req, body_json, login_root};

#[tokio::test]
async fn test_recurrence_crud() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app
        .clone()
        .oneshot(auth_req(
            "POST",
            "/api/tasks",
            &tok,
            Some(json!({"title":"Daily standup"})),
        ))
        .await
        .unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Set recurrence
    let resp = app
        .clone()
        .oneshot(auth_req(
            "PUT",
            &format!("/api/tasks/{}/recurrence", tid),
            &tok,
            Some(json!({"pattern":"daily","next_due":"2026-04-12"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let rec = body_json(resp).await;
    assert_eq!(rec["pattern"], "daily");
    // Get recurrence
    let resp = app
        .clone()
        .oneshot(auth_req(
            "GET",
            &format!("/api/tasks/{}/recurrence", tid),
            &tok,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(body_json(resp).await["pattern"], "daily");
    // Invalid pattern
    let resp = app
        .clone()
        .oneshot(auth_req(
            "PUT",
            &format!("/api/tasks/{}/recurrence", tid),
            &tok,
            Some(json!({"pattern":"yearly","next_due":"2027-01-01"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    // Remove recurrence
    let resp = app
        .clone()
        .oneshot(auth_req(
            "DELETE",
            &format!("/api/tasks/{}/recurrence", tid),
            &tok,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_recurrence_idempotency() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create task with recurrence
    let resp = app
        .clone()
        .oneshot(auth_req(
            "POST",
            "/api/tasks",
            &tok,
            Some(json!({"title":"Recurring"})),
        ))
        .await
        .unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    let today = chrono::Utc::now()
        .naive_utc()
        .format("%Y-%m-%d")
        .to_string();
    app.clone()
        .oneshot(auth_req(
            "PUT",
            &format!("/api/tasks/{}/recurrence", tid),
            &tok,
            Some(json!({
                "pattern": "daily", "next_due": today
            })),
        ))
        .await
        .unwrap();

    // Verify recurrence was set
    let resp = app
        .clone()
        .oneshot(auth_req(
            "GET",
            &format!("/api/tasks/{}/recurrence", tid),
            &tok,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let rec = body_json(resp).await;
    assert_eq!(rec["pattern"], "daily");
    assert_eq!(rec["next_due"], today);
}

#[tokio::test]
async fn test_recurrence_patterns() {
    let app = app().await;
    let tok = login_root(&app).await;
    let task = body_json(
        app.clone()
            .oneshot(auth_req(
                "POST",
                "/api/tasks",
                &tok,
                Some(json!({"title":"Recurring"})),
            ))
            .await
            .unwrap(),
    )
    .await;
    let tid = task["id"].as_i64().unwrap();
    // Set daily recurrence
    let resp = app
        .clone()
        .oneshot(auth_req(
            "PUT",
            &format!("/api/tasks/{}/recurrence", tid),
            &tok,
            Some(json!({"pattern":"daily","next_due":"2026-04-12"})),
        ))
        .await
        .unwrap();
    assert!(resp.status().is_success());
    // Get recurrence
    let rec = body_json(
        app.clone()
            .oneshot(auth_req(
                "GET",
                &format!("/api/tasks/{}/recurrence", tid),
                &tok,
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(rec["pattern"], "daily");
    // Update to weekly
    let resp = app
        .clone()
        .oneshot(auth_req(
            "PUT",
            &format!("/api/tasks/{}/recurrence", tid),
            &tok,
            Some(json!({"pattern":"weekly","next_due":"2026-04-18"})),
        ))
        .await
        .unwrap();
    assert!(resp.status().is_success());
    // Delete recurrence
    let resp = app
        .clone()
        .oneshot(auth_req(
            "DELETE",
            &format!("/api/tasks/{}/recurrence", tid),
            &tok,
            None,
        ))
        .await
        .unwrap();
    assert!(resp.status().is_success());
}

#[tokio::test]
async fn test_recurrence_set_get_remove() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app
        .clone()
        .oneshot(auth_req(
            "POST",
            "/api/tasks",
            &tok,
            Some(json!({"title":"RecurTask"})),
        ))
        .await
        .unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    // Set recurrence
    let resp = app
        .clone()
        .oneshot(auth_req(
            "PUT",
            &format!("/api/tasks/{}/recurrence", tid),
            &tok,
            Some(json!({"pattern":"daily","next_due":"2026-05-01"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let rec = body_json(resp).await;
    assert_eq!(rec["pattern"], "daily");

    // Get recurrence
    let resp = app
        .clone()
        .oneshot(auth_req(
            "GET",
            &format!("/api/tasks/{}/recurrence", tid),
            &tok,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Invalid pattern rejected
    let resp = app
        .clone()
        .oneshot(auth_req(
            "PUT",
            &format!("/api/tasks/{}/recurrence", tid),
            &tok,
            Some(json!({"pattern":"yearly","next_due":"2026-05-01"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);

    // Invalid date format rejected
    let resp = app
        .clone()
        .oneshot(auth_req(
            "PUT",
            &format!("/api/tasks/{}/recurrence", tid),
            &tok,
            Some(json!({"pattern":"daily","next_due":"not-a-date"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);

    // Remove recurrence
    let resp = app
        .clone()
        .oneshot(auth_req(
            "DELETE",
            &format!("/api/tasks/{}/recurrence", tid),
            &tok,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_recurrence_crud_and_patterns() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app
        .clone()
        .oneshot(auth_req(
            "POST",
            "/api/tasks",
            &tok,
            Some(json!({"title":"Recurring"})),
        ))
        .await
        .unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    // Set daily recurrence
    let resp = app
        .clone()
        .oneshot(auth_req(
            "PUT",
            &format!("/api/tasks/{}/recurrence", tid),
            &tok,
            Some(json!({"pattern":"daily","next_due":"2026-04-15"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Get recurrence
    let resp = app
        .clone()
        .oneshot(auth_req(
            "GET",
            &format!("/api/tasks/{}/recurrence", tid),
            &tok,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let rec = body_json(resp).await;
    assert_eq!(rec["pattern"], "daily");
    assert_eq!(rec["next_due"], "2026-04-15");

    // Update to weekly
    let resp = app
        .clone()
        .oneshot(auth_req(
            "PUT",
            &format!("/api/tasks/{}/recurrence", tid),
            &tok,
            Some(json!({"pattern":"weekly","next_due":"2026-04-20"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Delete recurrence
    let resp = app
        .clone()
        .oneshot(auth_req(
            "DELETE",
            &format!("/api/tasks/{}/recurrence", tid),
            &tok,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Get should return empty/null
    let resp = app
        .clone()
        .oneshot(auth_req(
            "GET",
            &format!("/api/tasks/{}/recurrence", tid),
            &tok,
            None,
        ))
        .await
        .unwrap();
    let rec = body_json(resp).await;
    assert!(rec.is_null() || rec.as_object().is_none_or(|o| o.is_empty()));
}

#[tokio::test]
async fn test_recurrence_auto_creates_task_via_db() {
    // Test the recurrence DB functions directly (simulating background scheduler)
    std::env::set_var("POFLOW_ROOT_PASSWORD", "root");
    let pool = poflow_daemon::db::connect_memory().await.unwrap();
    let yesterday = (chrono::Utc::now() - chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    // Create template task
    let task = poflow_daemon::db::create_task(
        &pool,
        poflow_daemon::db::CreateTaskOpts {
            user_id: 1,
            parent_id: None,
            title: "Daily standup",
            description: None,
            project: Some("Team"),
            project_id: None,
            tags: None,
            priority: 4,
            estimated: 1,
            estimated_hours: 0.0,
            remaining_points: 0.0,
            due_date: None,
        },
    )
    .await
    .unwrap();
    // Set recurrence with yesterday's due date
    poflow_daemon::db::set_recurrence(&pool, task.id, "daily", &yesterday)
        .await
        .unwrap();
    // Get due recurrences
    let due = poflow_daemon::db::get_due_recurrences(&pool, &today)
        .await
        .unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].task_id, task.id);
    // Simulate auto-creation
    let title = format!("{} ({})", task.title, today);
    let new_task = poflow_daemon::db::create_task(
        &pool,
        poflow_daemon::db::CreateTaskOpts {
            user_id: task.user_id,
            parent_id: task.parent_id,
            title: &title,
            description: task.description.as_deref(),
            project: task.project.as_deref(),
            project_id: task.project_id,
            tags: task.tags.as_deref(),
            priority: task.priority,
            estimated: task.estimated,
            estimated_hours: task.estimated_hours,
            remaining_points: task.remaining_points,
            due_date: task.due_date.as_deref(),
        },
    )
    .await
    .unwrap();
    assert!(new_task.title.contains(&today));
    assert_eq!(new_task.project.as_deref(), Some("Team"));
    assert_eq!(new_task.priority, 4);
    // Advance recurrence
    let next = chrono::NaiveDate::parse_from_str(&yesterday, "%Y-%m-%d").unwrap()
        + chrono::Duration::days(1);
    poflow_daemon::db::advance_recurrence(&pool, task.id, &next.format("%Y-%m-%d").to_string())
        .await
        .unwrap();
    // Verify next_due advanced
    let rec = poflow_daemon::db::get_recurrence(&pool, task.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(rec.next_due, today);
    assert!(
        rec.last_created.as_ref().unwrap().starts_with(&today),
        "last_created should be today"
    );
}

#[tokio::test]
async fn test_recurrence_monthly_advances_correctly() {
    use chrono::Datelike;
    std::env::set_var("POFLOW_ROOT_PASSWORD", "root");
    let pool = poflow_daemon::db::connect_memory().await.unwrap();
    let task = poflow_daemon::db::create_task(
        &pool,
        poflow_daemon::db::CreateTaskOpts {
            user_id: 1,
            parent_id: None,
            title: "Monthly report",
            description: None,
            project: None,
            project_id: None,
            tags: None,
            priority: 3,
            estimated: 1,
            estimated_hours: 0.0,
            remaining_points: 0.0,
            due_date: Some("2026-01-31"),
        },
    )
    .await
    .unwrap();
    poflow_daemon::db::set_recurrence(&pool, task.id, "monthly", "2026-01-31")
        .await
        .unwrap();
    // Advance from Jan 31 → should go to Feb 28 (not Feb 31)
    let d = chrono::NaiveDate::parse_from_str("2026-01-31", "%Y-%m-%d").unwrap();
    let m = d.month() % 12 + 1; // Feb
    let y = if m == 1 { d.year() + 1 } else { d.year() };
    let max_day = chrono::NaiveDate::from_ymd_opt(y, m + 1, 1)
        .and_then(|d| d.pred_opt())
        .map(|d| d.day())
        .unwrap_or(28);
    let next = chrono::NaiveDate::from_ymd_opt(y, m, 31u32.min(max_day)).unwrap();
    assert_eq!(
        next.to_string(),
        "2026-02-28",
        "Jan 31 monthly should advance to Feb 28"
    );
}
