use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{app, auth_req, body_json, login_root, register_user};

#[tokio::test]
async fn test_teams_crud() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create team
    let resp = app
        .clone()
        .oneshot(auth_req(
            "POST",
            "/api/teams",
            &tok,
            Some(json!({"name":"Alpha"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let team = body_json(resp).await;
    let tid = team["id"].as_i64().unwrap();
    assert_eq!(team["name"], "Alpha");

    // List teams
    let resp = app
        .clone()
        .oneshot(auth_req("GET", "/api/teams", &tok, None))
        .await
        .unwrap();
    let teams = body_json(resp).await;
    assert_eq!(teams.as_array().unwrap().len(), 1);

    // Get team detail
    let resp = app
        .clone()
        .oneshot(auth_req("GET", &format!("/api/teams/{}", tid), &tok, None))
        .await
        .unwrap();
    let detail = body_json(resp).await;
    assert_eq!(detail["team"]["name"], "Alpha");
    assert_eq!(detail["members"].as_array().unwrap().len(), 1); // creator auto-added

    // My teams
    let resp = app
        .clone()
        .oneshot(auth_req("GET", "/api/me/teams", &tok, None))
        .await
        .unwrap();
    let my = body_json(resp).await;
    assert_eq!(my.as_array().unwrap().len(), 1);

    // Delete team (root only)
    let resp = app
        .clone()
        .oneshot(auth_req(
            "DELETE",
            &format!("/api/teams/{}", tid),
            &tok,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_team_members_and_root_tasks() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create team + task
    let resp = app
        .clone()
        .oneshot(auth_req(
            "POST",
            "/api/teams",
            &tok,
            Some(json!({"name":"Beta"})),
        ))
        .await
        .unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app
        .clone()
        .oneshot(auth_req(
            "POST",
            "/api/tasks",
            &tok,
            Some(json!({"title":"Root Task"})),
        ))
        .await
        .unwrap();
    let task_id = body_json(resp).await["id"].as_i64().unwrap();

    // Add root task
    let resp = app
        .clone()
        .oneshot(auth_req(
            "POST",
            &format!("/api/teams/{}/roots", tid),
            &tok,
            Some(json!({"task_ids":[task_id]})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Get scope
    let resp = app
        .clone()
        .oneshot(auth_req(
            "GET",
            &format!("/api/teams/{}/scope", tid),
            &tok,
            None,
        ))
        .await
        .unwrap();
    let scope = body_json(resp).await;
    assert!(scope.as_array().unwrap().contains(&json!(task_id)));

    // Remove root task
    let resp = app
        .clone()
        .oneshot(auth_req(
            "DELETE",
            &format!("/api/teams/{}/roots/{}", tid, task_id),
            &tok,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_team_member_add_remove() {
    let app = app().await;
    let root_tok = login_root(&app).await;
    let _user_tok = register_user(&app, "teamUser1").await;
    // Create team as root
    let team = body_json(
        app.clone()
            .oneshot(auth_req(
                "POST",
                "/api/teams",
                &root_tok,
                Some(json!({"name":"TestTeam"})),
            ))
            .await
            .unwrap(),
    )
    .await;
    let tid = team["id"].as_i64().unwrap();
    // Get user id
    let users = body_json(
        app.clone()
            .oneshot(auth_req("GET", "/api/admin/users", &root_tok, None))
            .await
            .unwrap(),
    )
    .await;
    let uid = users
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["username"] == "teamUser1")
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    // Add member
    let resp = app
        .clone()
        .oneshot(auth_req(
            "POST",
            &format!("/api/teams/{}/members", tid),
            &root_tok,
            Some(json!({"user_id":uid,"role":"member"})),
        ))
        .await
        .unwrap();
    assert!(resp.status().is_success());
    // Verify member in team detail
    let detail = body_json(
        app.clone()
            .oneshot(auth_req(
                "GET",
                &format!("/api/teams/{}", tid),
                &root_tok,
                None,
            ))
            .await
            .unwrap(),
    )
    .await;
    let members = detail["members"].as_array().unwrap();
    assert!(members.iter().any(|m| m["username"] == "teamUser1"));
    // Remove member
    let resp = app
        .clone()
        .oneshot(auth_req(
            "DELETE",
            &format!("/api/teams/{}/members/{}", tid, uid),
            &root_tok,
            None,
        ))
        .await
        .unwrap();
    assert!(resp.status().is_success());
}

#[tokio::test]
async fn test_my_teams() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create team (auto-adds creator as admin)
    app.clone()
        .oneshot(auth_req(
            "POST",
            "/api/teams",
            &tok,
            Some(json!({"name":"MyTeam1"})),
        ))
        .await
        .unwrap();
    let resp = app
        .clone()
        .oneshot(auth_req("GET", "/api/me/teams", &tok, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let teams = body_json(resp).await;
    assert!(teams
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["name"] == "MyTeam1"));
}

#[tokio::test]
async fn test_team_scope() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create team
    let resp = app
        .clone()
        .oneshot(auth_req(
            "POST",
            "/api/teams",
            &tok,
            Some(json!({"name":"Alpha"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let team_id = body_json(resp).await["id"].as_i64().unwrap();

    // Create tasks
    let resp = app
        .clone()
        .oneshot(auth_req(
            "POST",
            "/api/tasks",
            &tok,
            Some(json!({"title":"TeamTask"})),
        ))
        .await
        .unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    // Add root task to team
    let resp = app
        .clone()
        .oneshot(auth_req(
            "POST",
            &format!("/api/teams/{}/roots", team_id),
            &tok,
            Some(json!({"task_ids":[tid]})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Query tasks scoped to team
    let resp = app
        .clone()
        .oneshot(auth_req(
            "GET",
            &format!("/api/tasks?team_id={}", team_id),
            &tok,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let tasks = body_json(resp).await;
    assert!(tasks.as_array().unwrap().iter().any(|t| t["id"] == tid));

    // Remove root task
    let resp = app
        .clone()
        .oneshot(auth_req(
            "DELETE",
            &format!("/api/teams/{}/roots/{}", team_id, tid),
            &tok,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
}
