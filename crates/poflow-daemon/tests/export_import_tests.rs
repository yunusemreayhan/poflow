use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{app, auth_req, body_json, login_root};

#[tokio::test]
async fn test_export_tasks_csv() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"ExportMe"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/tasks?format=csv", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let csv = String::from_utf8_lossy(&bytes);
    assert!(csv.contains("ExportMe"));
    assert!(csv.starts_with("id,"));
}

#[tokio::test]
async fn test_export_tasks_json() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Export Me"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/tasks?format=json", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("application/json"));
    let tasks = body_json(resp).await;
    assert!(!tasks.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_export_sessions_csv() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/sessions?format=csv", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("text/csv"));
    let body = String::from_utf8(resp.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert!(body.starts_with("id,task_id,user,session_type,status,started_at,ended_at,duration_s,task_path"));
}

#[tokio::test]
async fn test_export_sessions_json() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/sessions?format=json", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("application/json"));
}

#[tokio::test]
async fn test_export_sessions_date_range() {
    let app = app().await;
    let token = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/sessions?format=json&from=2020-01-01&to=2020-12-31", &token, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = body_json(resp).await;
    assert!(body.is_array());
    assert_eq!(body.as_array().unwrap().len(), 0); // no sessions in that range
}

#[tokio::test]
async fn test_csv_import_tasks() {
    let app = app().await;
    let token = login_root(&app).await;
    let csv = "title,priority,estimated,project\nImported Task,2,5,myproj\nAnother,3,0,";
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &token, Some(json!({"csv": csv})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = body_json(resp).await;
    assert_eq!(body["created"], 2);
}

#[tokio::test]
async fn test_csv_import_quoted_fields() {
    let app = app().await;
    let tok = login_root(&app).await;
    let csv = "title,priority,estimated,project\n\"Task with, comma\",3,2,\"Project A\"\n\"Normal task\",1,1,";
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &tok, Some(json!({"csv": csv})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = body_json(resp).await;
    assert_eq!(body["created"], 2);
    // Verify the comma-containing title was imported correctly
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks", &tok, None)).await.unwrap();
    let tasks = body_json(resp).await;
    let titles: Vec<&str> = tasks.as_array().unwrap().iter().map(|t| t["title"].as_str().unwrap()).collect();
    assert!(titles.contains(&"Task with, comma"));
}

#[tokio::test]
async fn test_csv_import_malformed() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Empty CSV (header only)
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &tok, Some(json!({"csv":"title,priority\n"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = body_json(resp).await;
    assert_eq!(body["created"], 0);

    // Missing columns — should still create with defaults
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &tok, Some(json!({"csv":"title\nJustTitle\n"})))).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["created"], 1);

    // Extra columns — should ignore extras
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &tok, Some(json!({"csv":"title,priority,estimated,project,extra1,extra2\nT,1,2,proj,x,y\n"})))).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["created"], 1);

    // Special characters in title
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &tok, Some(json!({"csv":"title\n\"Quoted, with comma\"\n=formula\n"})))).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["created"], 2);

    // Empty title lines should be skipped
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &tok, Some(json!({"csv":"title\n\n  \nReal Task\n"})))).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["created"], 1);

    // Too large CSV
    let big = "title\n".to_string() + &"x".repeat(2_000_000);
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &tok, Some(json!({"csv": big})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_csv_import_quoted_fields_with_commas() {
    let app = app().await;
    let tok = login_root(&app).await;
    let csv = "title,priority,estimated,project\n\"Task with, comma\",3,2,\"My Project\"\n\"Task \"\"quoted\"\"\",1,1,\n";
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &tok, Some(json!({"csv": csv})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let result = body_json(resp).await;
    assert_eq!(result["created"], 2);
}

#[tokio::test]
async fn test_csv_import_empty_lines_skipped() {
    let app = app().await;
    let tok = login_root(&app).await;
    let csv = "title,priority\n\n\nActual Task,3\n\n";
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &tok, Some(json!({"csv": csv})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let result = body_json(resp).await;
    assert_eq!(result["created"], 1);
}

#[tokio::test]
async fn test_csv_import_size_limit() {
    let app = app().await;
    let tok = login_root(&app).await;
    let csv = "a".repeat(1_048_577); // > 1MB
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &tok, Some(json!({"csv": csv})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_csv_import_title_length() {
    let app = app().await;
    let tok = login_root(&app).await;
    let long_title = "x".repeat(600);
    let csv = format!("title,priority\n{},3\nShort,2", long_title);
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks", &tok, Some(json!({"csv": csv})))).await.unwrap();
    let result = body_json(resp).await;
    // Long title should produce an error, short one should succeed
    assert_eq!(result["created"].as_i64().unwrap(), 1);
    assert!(!result["errors"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_csv_export() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create some tasks
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"CSV1","project":"P1"})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"CSV2","project":"P1"})))).await.unwrap();

    // Export CSV
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/tasks?format=csv", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.contains("csv") || ct.contains("text"));
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let csv = String::from_utf8(body.to_vec()).unwrap();
    assert!(csv.contains("CSV1"));
    assert!(csv.contains("CSV2"));
}

#[tokio::test]
async fn test_json_import() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks/json", &tok, Some(json!({
        "tasks": [
            {"title": "Parent", "children": [{"title": "Child1"}, {"title": "Child2"}]},
            {"title": "Standalone"}
        ]
    })))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let result = body_json(resp).await;
    assert_eq!(result["created"].as_i64().unwrap(), 4);
    // Empty title rejected
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/tasks/json", &tok, Some(json!({
        "tasks": [{"title": ""}]
    })))).await.unwrap();
    let result = body_json(resp).await;
    assert!(!result["errors"].as_array().unwrap().is_empty());
}


// ========== F4: iCal Export ==========

#[tokio::test]
async fn test_ical_export() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create task with due date
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"ical task","due_date":"2026-06-15"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    // Export iCal
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/ical", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let ical = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(ical.contains("BEGIN:VCALENDAR"));
    assert!(ical.contains("ical task"));
    assert!(ical.contains("20260615"));
    assert!(ical.contains("END:VCALENDAR"));
}

// ========== Cross-feature: iCal excludes deleted tasks ==========

#[tokio::test]
async fn test_ical_excludes_deleted() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"deleted ical","due_date":"2026-12-25"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Delete the task
    app.clone().oneshot(auth_req("DELETE", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
    // Export iCal
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/ical", &tok, None)).await.unwrap();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let ical = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(!ical.contains("deleted ical"));
}

// ============================================================
// Comprehensive project export
// ============================================================

#[tokio::test]
async fn test_project_export_includes_all_data() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create task with comment, label, checklist, custom field
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"ExportTask","project":"ExportProj"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &tok, Some(json!({"content":"Test comment"})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/labels", &tok, Some(json!({"name":"export_label","color":"#ff0000"})))).await.unwrap();
    let lid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}/labels/{}", tid, lid), &tok, None)).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/checklist", tid), &tok, Some(json!({"title":"Check item"})))).await.unwrap();
    // Export
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/project?project=ExportProj", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let export = body_json(resp).await;
    assert_eq!(export["version"], 1);
    assert!(!export["tasks"].as_array().unwrap().is_empty());
    assert!(!export["comments"].as_array().unwrap().is_empty());
    assert!(!export["labels"].as_array().unwrap().is_empty());
    assert!(!export["checklists"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_project_import_roundtrip() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create source data
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"ImportMe","project":"RoundTrip","priority":4})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &tok, Some(json!({"content":"Round trip comment"})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/checklist", tid), &tok, Some(json!({"title":"Check 1"})))).await.unwrap();
    // Export
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/project?project=RoundTrip", &tok, None)).await.unwrap();
    let export = body_json(resp).await;
    // Import into same DB (creates duplicates — that's fine for testing)
    let resp = app.clone().oneshot(auth_req("POST", "/api/import/project", &tok, Some(export.clone()))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let result = body_json(resp).await;
    assert_eq!(result["created_tasks"], 1);
    assert_eq!(result["created_comments"], 1);
    assert_eq!(result["created_checklists"], 1);
}

#[tokio::test]
async fn test_project_export_empty_project() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/export/project?project=NonExistentProject", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let export = body_json(resp).await;
    assert_eq!(export["tasks"].as_array().unwrap().len(), 0);
}
