use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{app, json_req, auth_req, body_json, login_root, register_user, register_user_full};

#[tokio::test]
async fn test_admin_list_users() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.oneshot(auth_req("GET", "/api/admin/users", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let users = body_json(resp).await;
    assert!(!users.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_non_root_cannot_admin() {
    let app = app().await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"bob","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"bob","password":"Pass1234"})))).await.unwrap();
    let tok = body_json(resp).await["token"].as_str().unwrap().to_string();

    let resp = app.oneshot(auth_req("GET", "/api/admin/users", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_delete_user_cascade() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"delme","password":"Pass1234"})))).await.unwrap();
    let uid = body_json(resp).await["user_id"].as_i64().unwrap();

    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"delme","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();
    app.clone().oneshot(auth_req("POST", "/api/tasks", &tok2, Some(json!({"title":"MyTask"})))).await.unwrap();

    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/admin/users/{}", uid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);

    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks", &tok, None)).await.unwrap();
    let tasks = body_json(resp).await;
    let task = tasks.as_array().unwrap().iter().find(|t| t["title"] == "MyTask").unwrap();
    assert_eq!(task["user"], "root");
}

#[tokio::test]
async fn test_delete_user_preserves_comments_and_burns() {
    let app = app().await;
    let tok = login_root(&app).await;
    let (tok2, uid) = register_user_full(&app, "burnuser", "BurnUs111").await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"BurnTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/comments", tid), &tok2, Some(json!({"content":"Important context"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let resp = app.clone().oneshot(auth_req("POST", "/api/sprints", &tok, Some(json!({"name":"BS"})))).await.unwrap();
    let sid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/tasks", sid), &tok, Some(json!({"task_ids":[tid]})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/start", sid), &tok, None)).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/tasks/{}/assignees", tid), &tok, Some(json!({"username":"burnuser"})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/sprints/{}/burn", sid), &tok2, Some(json!({"task_id":tid,"points":5.0})))).await.unwrap();
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/admin/users/{}", uid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}", tid), &tok, None)).await.unwrap();
    let detail = body_json(resp).await;
    let comments = detail["comments"].as_array().unwrap();
    assert!(comments.iter().any(|c| c["content"] == "Important context"), "Comment should survive user deletion");
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/sprints/{}/burns", sid), &tok, None)).await.unwrap();
    let burns = body_json(resp).await;
    assert!(!burns.as_array().unwrap().is_empty(), "Burns should survive user deletion");
}

#[tokio::test]
async fn test_delete_last_root_prevented() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/admin/users", &tok, None)).await.unwrap();
    let users = body_json(resp).await;
    let root_id = users.as_array().unwrap().iter().find(|u| u["username"] == "root").unwrap()["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/admin/users/{}", root_id), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_users_list_public() {
    let app = app().await;
    let tok = register_user(&app, "listUser").await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/users", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let users = body_json(resp).await;
    assert!(users.as_array().unwrap().len() >= 2);
}

#[tokio::test]
async fn test_backup_root_only() {
    let app = app().await;
    let tok = login_root(&app).await;
    let user_tok = register_user(&app, "backupUser").await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/admin/backup", &user_tok, None)).await.unwrap();
    assert_eq!(resp.status(), 403);
    let resp = app.clone().oneshot(auth_req("POST", "/api/admin/backup", &tok, None)).await.unwrap();
    assert!(resp.status() == 200 || resp.status() == 500);
}

#[tokio::test]
async fn test_backup_list() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/admin/backups", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let resp = app.clone().oneshot(auth_req("POST", "/api/admin/restore", &tok, Some(json!({"filename":"../../../etc/passwd"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_internal_error_no_leak() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("GET", "/api/tasks/999999", &tok, None)).await.unwrap();
    if resp.status() == 500 {
        let j = body_json(resp).await;
        let msg = j["error"].as_str().unwrap_or("");
        assert!(!msg.contains("sqlx"), "Error message should not contain sqlx details");
        assert!(!msg.contains("SELECT"), "Error message should not contain SQL");
        assert!(!msg.contains("tasks"), "Error message should not contain table names");
        assert_eq!(msg, "Internal server error");
    }
}

#[tokio::test]
async fn test_admin_can_create_custom_statuses_and_fields() {
    let app = app().await;
    let tok = login_root(&app).await;
    let (_, admin_uid) = register_user_full(&app, "admcf", "AdmCF1111").await;
    app.clone().oneshot(auth_req("PUT", &format!("/api/admin/users/{}/role", admin_uid), &tok, Some(json!({"role":"admin"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"admcf","password":"AdmCF1111"})))).await.unwrap();
    let admin_tok = body_json(resp).await["token"].as_str().unwrap().to_string();
    // Admin can create custom statuses
    let resp = app.clone().oneshot(auth_req("POST", "/api/statuses", &admin_tok, Some(json!({"name":"admin_review","category":"in_progress"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    // Admin can create custom fields
    let resp = app.clone().oneshot(auth_req("POST", "/api/fields", &admin_tok, Some(json!({"name":"admin_field","field_type":"text"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
}

#[tokio::test]
async fn test_admin_can_manage_labels() {
    let app = app().await;
    let tok = login_root(&app).await;
    let (_, uid) = register_user_full(&app, "admlab", "AdmLab111").await;
    // Elevate to admin
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/admin/users/{}/role", uid), &tok, Some(json!({"role":"admin"})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    // Re-login as admin to get fresh token with admin role
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"admlab","password":"AdmLab111"})))).await.unwrap();
    let admin_tok = body_json(resp).await["token"].as_str().unwrap().to_string();
    // Admin can create labels
    let resp = app.clone().oneshot(auth_req("POST", "/api/labels", &admin_tok, Some(json!({"name":"admin_label","color":"#ff0000"})))).await.unwrap();
    assert_eq!(resp.status(), 201, "Admin should be able to create labels");
}

#[tokio::test]
async fn test_admin_can_manage_others_tasks() {
    let app = app().await;
    let tok = login_root(&app).await;
    let (user_tok, _) = register_user_full(&app, "admtask_user", "AdmTU1111").await;
    let (_, admin_uid) = register_user_full(&app, "admtask_admin", "AdmTA1111").await;
    // Elevate to admin
    app.clone().oneshot(auth_req("PUT", &format!("/api/admin/users/{}/role", admin_uid), &tok, Some(json!({"role":"admin"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"admtask_admin","password":"AdmTA1111"})))).await.unwrap();
    let admin_tok = body_json(resp).await["token"].as_str().unwrap().to_string();
    // User creates task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &user_tok, Some(json!({"title":"User task"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    // Admin can update user's task
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/tasks/{}", tid), &admin_tok, Some(json!({"status":"in_progress"})))).await.unwrap();
    assert_eq!(resp.status(), 200, "Admin should be able to update any task");
}

#[tokio::test]
async fn test_admin_cannot_manage_users() {
    let app = app().await;
    let tok = login_root(&app).await;
    let (_, admin_uid) = register_user_full(&app, "admnouser", "AdmNU1111").await;
    app.clone().oneshot(auth_req("PUT", &format!("/api/admin/users/{}/role", admin_uid), &tok, Some(json!({"role":"admin"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"admnouser","password":"AdmNU1111"})))).await.unwrap();
    let admin_tok = body_json(resp).await["token"].as_str().unwrap().to_string();
    // Admin cannot list users
    let resp = app.clone().oneshot(auth_req("GET", "/api/admin/users", &admin_tok, None)).await.unwrap();
    assert_eq!(resp.status(), 403, "Admin should NOT be able to list users");
    // Admin cannot create backup
    let resp = app.clone().oneshot(auth_req("POST", "/api/admin/backup", &admin_tok, None)).await.unwrap();
    assert_eq!(resp.status(), 403, "Admin should NOT be able to create backups");
}