use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{app, auth_req, body_json, login_root};

#[tokio::test]
async fn test_labels_crud() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create label
    let resp = app
        .clone()
        .oneshot(auth_req(
            "POST",
            "/api/labels",
            &tok,
            Some(json!({"name":"urgent","color":"#ff0000"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let label = body_json(resp).await;
    let lid = label["id"].as_i64().unwrap();
    // List labels
    let resp = app
        .clone()
        .oneshot(auth_req("GET", "/api/labels", &tok, None))
        .await
        .unwrap();
    let labels = body_json(resp).await;
    assert!(labels
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l["name"] == "urgent"));
    // Create task and add label
    let resp = app
        .clone()
        .oneshot(auth_req(
            "POST",
            "/api/tasks",
            &tok,
            Some(json!({"title":"Labeled"})),
        ))
        .await
        .unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app
        .clone()
        .oneshot(auth_req(
            "PUT",
            &format!("/api/tasks/{}/labels/{}", tid, lid),
            &tok,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
    // Get task labels
    let resp = app
        .clone()
        .oneshot(auth_req(
            "GET",
            &format!("/api/tasks/{}/labels", tid),
            &tok,
            None,
        ))
        .await
        .unwrap();
    let task_labels = body_json(resp).await;
    assert_eq!(task_labels.as_array().unwrap().len(), 1);
    // Remove label from task
    let resp = app
        .clone()
        .oneshot(auth_req(
            "DELETE",
            &format!("/api/tasks/{}/labels/{}", tid, lid),
            &tok,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
    // Delete label
    let resp = app
        .clone()
        .oneshot(auth_req(
            "DELETE",
            &format!("/api/labels/{}", lid),
            &tok,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_label_task_association() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create label
    let label = body_json(
        app.clone()
            .oneshot(auth_req(
                "POST",
                "/api/labels",
                &tok,
                Some(json!({"name":"urgent","color":"#ff0000"})),
            ))
            .await
            .unwrap(),
    )
    .await;
    let lid = label["id"].as_i64().unwrap();
    // Create task
    let task = body_json(
        app.clone()
            .oneshot(auth_req(
                "POST",
                "/api/tasks",
                &tok,
                Some(json!({"title":"Labeled"})),
            ))
            .await
            .unwrap(),
    )
    .await;
    let tid = task["id"].as_i64().unwrap();
    // Add label to task
    let resp = app
        .clone()
        .oneshot(auth_req(
            "PUT",
            &format!("/api/tasks/{}/labels/{}", tid, lid),
            &tok,
            None,
        ))
        .await
        .unwrap();
    assert!(resp.status().is_success());
    // Get task labels
    let labels = body_json(
        app.clone()
            .oneshot(auth_req(
                "GET",
                &format!("/api/tasks/{}/labels", tid),
                &tok,
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(labels.as_array().unwrap().len(), 1);
    // Remove label
    let resp = app
        .clone()
        .oneshot(auth_req(
            "DELETE",
            &format!("/api/tasks/{}/labels/{}", tid, lid),
            &tok,
            None,
        ))
        .await
        .unwrap();
    assert!(resp.status().is_success());
    let labels = body_json(
        app.clone()
            .oneshot(auth_req(
                "GET",
                &format!("/api/tasks/{}/labels", tid),
                &tok,
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(labels.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_label_filter() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create label
    let resp = app
        .clone()
        .oneshot(auth_req(
            "POST",
            "/api/labels",
            &tok,
            Some(json!({"name":"urgent","color":"#ef4444"})),
        ))
        .await
        .unwrap();
    let lid = body_json(resp).await["id"].as_i64().unwrap();
    // Create tasks
    let resp = app
        .clone()
        .oneshot(auth_req(
            "POST",
            "/api/tasks",
            &tok,
            Some(json!({"title":"Labeled"})),
        ))
        .await
        .unwrap();
    let t1 = body_json(resp).await["id"].as_i64().unwrap();
    app.clone()
        .oneshot(auth_req(
            "POST",
            "/api/tasks",
            &tok,
            Some(json!({"title":"Unlabeled"})),
        ))
        .await
        .unwrap();
    // Add label to t1
    app.clone()
        .oneshot(auth_req(
            "PUT",
            &format!("/api/tasks/{}/labels/{}", t1, lid),
            &tok,
            None,
        ))
        .await
        .unwrap();
    // Filter by label
    let resp = app
        .clone()
        .oneshot(auth_req("GET", "/api/tasks?label=urgent", &tok, None))
        .await
        .unwrap();
    let tasks = body_json(resp).await;
    let arr = tasks.as_array().unwrap();
    assert!(
        arr.iter().all(|t| t["title"] == "Labeled"),
        "Label filter should only return labeled tasks"
    );
    assert!(arr.iter().any(|t| t["title"] == "Labeled"));
}
