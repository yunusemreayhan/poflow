use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{app, json_req, auth_req, body_json, login_root, reg};

#[tokio::test]
async fn test_room_create_and_list() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok,
        Some(json!({"name":"Sprint 1","estimation_unit":"points"})))).await.unwrap();
    assert!(resp.status().is_success());
    let room = body_json(resp).await;
    assert_eq!(room["name"], "Sprint 1");
    assert_eq!(room["status"], "lobby");

    let resp = app.oneshot(auth_req("GET", "/api/rooms", &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_room_full_voting_flow() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create task + room
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"Story"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok,
        Some(json!({"name":"R","estimation_unit":"points"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();

    // Start voting
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/start-voting", rid), &tok,
        Some(json!({"task_id":tid})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let r = body_json(resp).await;
    assert_eq!(r["status"], "voting");

    // Cast vote
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &tok,
        Some(json!({"value":8})))).await.unwrap();
    assert!(resp.status().is_success());

    // Reveal
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/reveal", rid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let r = body_json(resp).await;
    assert_eq!(r["status"], "revealed");

    // Accept
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/accept", rid), &tok,
        Some(json!({"value":8})))).await.unwrap();
    assert_eq!(resp.status(), 200);
    let task = body_json(resp).await;
    assert_eq!(task["estimated"], 8);
    assert_eq!(task["status"], "estimated");

    // Task votes endpoint
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/tasks/{}/votes", tid), &tok, None)).await.unwrap();
    let votes = body_json(resp).await;
    assert_eq!(votes.as_array().unwrap().len(), 1);
    assert_eq!(votes[0]["value"], 8.0);
}

#[tokio::test]
async fn test_room_join_leave_kick() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Register second user
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"eve","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"eve","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok,
        Some(json!({"name":"R","estimation_unit":"hours"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();

    // Eve joins
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/join", rid), &tok2, None)).await.unwrap();

    // Check members via state
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/rooms/{}", rid), &tok, None)).await.unwrap();
    let state = body_json(resp).await;
    assert_eq!(state["members"].as_array().unwrap().len(), 2);

    // Kick eve
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/rooms/{}/members/eve", rid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);

    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/rooms/{}", rid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await["members"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_room_role_promotion() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"dan","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"dan","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok,
        Some(json!({"name":"R","estimation_unit":"points"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();

    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/join", rid), &tok2, None)).await.unwrap();

    // Promote dan to admin
    let resp = app.clone().oneshot(auth_req("PUT", &format!("/api/rooms/{}/role", rid), &tok,
        Some(json!({"username":"dan","role":"admin"})))).await.unwrap();
    assert!(resp.status().is_success());

    // Dan can now start voting (admin action)
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"X"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/start-voting", rid), &tok2,
        Some(json!({"task_id":tid})))).await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_room_non_admin_cannot_start_voting() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"noob","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"noob","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok,
        Some(json!({"name":"R","estimation_unit":"points"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/join", rid), &tok2, None)).await.unwrap();

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"X"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/start-voting", rid), &tok2,
        Some(json!({"task_id":tid})))).await.unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_room_close() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok,
        Some(json!({"name":"R","estimation_unit":"points"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/close", rid), &tok, None)).await.unwrap();
    assert!(resp.status().is_success());

    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/rooms/{}", rid), &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await["room"]["status"], "closed");
}

#[tokio::test]
async fn test_room_delete() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok,
        Some(json!({"name":"R","estimation_unit":"points"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/rooms/{}", rid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);

    let resp = app.clone().oneshot(auth_req("GET", "/api/rooms", &tok, None)).await.unwrap();
    assert_eq!(body_json(resp).await.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_room_accept_hours() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"H"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok,
        Some(json!({"name":"R","estimation_unit":"hours"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();

    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/start-voting", rid), &tok, Some(json!({"task_id":tid})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &tok, Some(json!({"value":4})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/reveal", rid), &tok, None)).await.unwrap();

    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/accept", rid), &tok, Some(json!({"value":4})))).await.unwrap();
    let task = body_json(resp).await;
    assert_eq!(task["estimated_hours"], 4.0);
    assert_eq!(task["status"], "estimated");
}

#[tokio::test]
async fn test_room_auto_advance() {
    let app = app().await;
    let tok = login_root(&app).await;

    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"A"})))).await.unwrap();
    let t1 = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"B"})))).await.unwrap();
    let t2 = body_json(resp).await["id"].as_i64().unwrap();

    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok,
        Some(json!({"name":"R","estimation_unit":"points"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();

    // Vote on first task
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/start-voting", rid), &tok, Some(json!({"task_id":t1})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &tok, Some(json!({"value":5})))).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/reveal", rid), &tok, None)).await.unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/accept", rid), &tok, Some(json!({"value":5})))).await.unwrap();

    // Should auto-advance to task B
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/rooms/{}", rid), &tok, None)).await.unwrap();
    let state = body_json(resp).await;
    assert_eq!(state["room"]["status"], "voting");
    assert_eq!(state["room"]["current_task_id"], t2);
}

#[tokio::test]
async fn test_get_room_no_auto_join() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"viewer","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"viewer","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"R"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();

    // Viewer GETs room state — should be forbidden (not a member, S2 fix)
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/rooms/{}", rid), &tok2, None)).await.unwrap();
    assert_eq!(resp.status(), 403);
    // Creator can still view
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/rooms/{}", rid), &tok, None)).await.unwrap();
    let state = body_json(resp).await;
    assert_eq!(state["members"].as_array().unwrap().len(), 1);
    assert_eq!(state["members"][0]["username"], "root");
}

#[tokio::test]
async fn test_delete_room_ownership() {
    let app = app().await;
    let tok = login_root(&app).await;
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"roomuser","password":"Pass1234"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"roomuser","password":"Pass1234"})))).await.unwrap();
    let tok2 = body_json(resp).await["token"].as_str().unwrap().to_string();

    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"R"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();

    // Non-owner cannot delete
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/rooms/{}", rid), &tok2, None)).await.unwrap();
    assert_eq!(resp.status(), 403);

    // Owner can delete
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/rooms/{}", rid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_room_ws_auth() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create a room
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"WSRoom"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let rid = body_json(resp).await["id"].as_i64().unwrap();
    // SSE ticket exchange works
    let resp = app.clone().oneshot(auth_req("POST", "/api/timer/ticket", &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let ticket = body_json(resp).await["ticket"].as_str().unwrap().to_string();
    assert!(!ticket.is_empty());
    // Room state accessible after creation
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/rooms/{}", rid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let state = body_json(resp).await;
    assert_eq!(state["room"]["name"], "WSRoom");
    // Members include creator
    assert!(!state["members"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_room_vote_without_active_task() {
    let app = app().await;
    let tok = login_root(&app).await;
    let room = body_json(app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"R"})))).await.unwrap()).await;
    let rid = room["id"].as_i64().unwrap();
    // Try to vote without starting voting — should fail
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &tok, Some(json!({"value":5.0})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_room_vote_range_validation() {
    let app = app().await;
    let tok = login_root(&app).await;
    let room = body_json(app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"R2"})))).await.unwrap()).await;
    let rid = room["id"].as_i64().unwrap();
    let task = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap()).await;
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/start-voting", rid), &tok, Some(json!({"task_id":task["id"]})))).await.unwrap();
    // Negative vote
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &tok, Some(json!({"value":-1.0})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // Over 1000
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &tok, Some(json!({"value":1001.0})))).await.unwrap();
    assert_eq!(resp.status(), 400);
    // Valid vote
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &tok, Some(json!({"value":5.0})))).await.unwrap();
    assert!(resp.status().is_success());
}

#[tokio::test]
async fn test_room_invalid_estimation_unit() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"R","estimation_unit":"bananas"})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_room_empty_name_rejected() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":""})))).await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_room_reveal_without_votes() {
    let app = app().await;
    let tok = login_root(&app).await;
    let room = body_json(app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"RevealRoom"})))).await.unwrap()).await;
    let rid = room["id"].as_i64().unwrap();
    let task = body_json(app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"T"})))).await.unwrap()).await;
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/start-voting", rid), &tok, Some(json!({"task_id":task["id"]})))).await.unwrap();
    // Reveal without any votes
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/reveal", rid), &tok, None)).await.unwrap();
    assert!(resp.status().is_success());
}

#[tokio::test]
async fn test_room_mandays_estimation() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"MandayRoom","estimation_unit":"mandays"})))).await.unwrap();
    assert_eq!(resp.status(), 201);
    let room = body_json(resp).await;
    assert_eq!(room["estimation_unit"], "mandays");
}

#[tokio::test]
async fn test_room_voting_flow() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Create room + task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"VoteTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"VoteRoom"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();

    // Join room
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/join", rid), &tok, None)).await.unwrap();
    assert!(resp.status().is_success());

    // Cannot vote in lobby state
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &tok, Some(json!({"value":5.0})))).await.unwrap();
    assert_eq!(resp.status(), 400);

    // Start voting on task
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/start-voting", rid), &tok, Some(json!({"task_id":tid})))).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Cast vote
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &tok, Some(json!({"value":8.0})))).await.unwrap();
    assert_eq!(resp.status(), 204);

    // Reveal votes
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/reveal", rid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    let room = body_json(resp).await;
    assert_eq!(room["status"], "revealed");

    // Accept estimate
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/accept", rid), &tok, Some(json!({"value":8.0})))).await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_concurrent_room_voting() {
    let app = app().await;
    let tok = login_root(&app).await;

    // Register second user
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"voter2","password":"Pass1234"})))).await.unwrap();
    assert_eq!(resp.status(), 200, "Register should succeed");
    let body = body_json(resp).await;
    let tok2 = body["token"].as_str().expect("register should return token").to_string();

    // Create room
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"ConcRoom"})))).await.unwrap();
    assert_eq!(resp.status(), 201, "Room creation should succeed");
    let rid = body_json(resp).await["id"].as_i64().unwrap();

    // Second user joins
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/join", rid), &tok2, None)).await.unwrap();
    assert!(resp.status().is_success(), "Join should succeed");

    // Create task
    let resp = app.clone().oneshot(auth_req("POST", "/api/tasks", &tok, Some(json!({"title":"VoteTask"})))).await.unwrap();
    let tid = body_json(resp).await["id"].as_i64().unwrap();

    // Start voting
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/start-voting", rid), &tok, Some(json!({"task_id":tid})))).await.unwrap();
    assert!(resp.status().is_success(), "Start voting should succeed");

    // Both vote simultaneously
    let (r1, r2) = tokio::join!(
        app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &tok, Some(json!({"value":5})))),
        app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/vote", rid), &tok2, Some(json!({"value":8}))))
    );
    assert!(r1.unwrap().status().is_success());
    assert!(r2.unwrap().status().is_success());

    // Reveal
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/reveal", rid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Fetch room state — both votes should be visible
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/rooms/{}", rid), &tok, None)).await.unwrap();
    let state = body_json(resp).await;
    let votes = state["votes"].as_array().unwrap();
    assert_eq!(votes.len(), 2);
    assert!(votes.iter().all(|v| v["voted"] == true));
}

#[tokio::test]
async fn test_room_membership_filter() {
    let app = app().await;
    let tok = login_root(&app).await;
    // Create a room and join
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"FilterRoom"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/join", rid), &tok, None)).await.unwrap();
    // Register second user
    app.clone().oneshot(json_req("POST", "/api/auth/register", Some(json!({"username":"roomuser","password":"Password1!"})))).await.unwrap();
    let resp = app.clone().oneshot(json_req("POST", "/api/auth/login", Some(json!({"username":"roomuser","password":"Password1!"})))).await.unwrap();
    let user_tok = body_json(resp).await["token"].as_str().unwrap().to_string();
    // Second user should NOT see the room (not a member)
    let resp = app.clone().oneshot(auth_req("GET", "/api/rooms", &user_tok, None)).await.unwrap();
    let rooms = body_json(resp).await;
    assert!(!rooms.as_array().unwrap().iter().any(|r| r["id"] == rid));
    // Root should see it
    let resp = app.clone().oneshot(auth_req("GET", "/api/rooms", &tok, None)).await.unwrap();
    let rooms = body_json(resp).await;
    assert!(rooms.as_array().unwrap().iter().any(|r| r["id"] == rid));
}

#[tokio::test]
async fn test_room_export() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"ExportRoom"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/join", rid), &tok, None)).await.unwrap();
    let resp = app.clone().oneshot(auth_req("GET", &format!("/api/rooms/{}/export", rid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert!(resp.headers().get("content-disposition").is_some());
}


// 5. POST /api/rooms/{id}/leave
#[tokio::test]
async fn test_leave_room() {
    let app = app().await;
    let tok = login_root(&app).await;
    let user_tok = reg(&app, "leaver").await;
    // Create room
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"LeaveRoom"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();
    // User joins
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/join", rid), &user_tok, None)).await.unwrap();
    // User leaves
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/leave", rid), &user_tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn test_leave_room_creator_cannot_leave() {
    let app = app().await;
    let tok = login_root(&app).await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"CreatorRoom"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();
    let resp = app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/leave", rid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 400);
}

// 12. DELETE /api/rooms/{id}/members/{username} — kick member
#[tokio::test]
async fn test_kick_member() {
    let app = app().await;
    let tok = login_root(&app).await;
    let user_tok = reg(&app, "kickee").await;
    let resp = app.clone().oneshot(auth_req("POST", "/api/rooms", &tok, Some(json!({"name":"KickRoom"})))).await.unwrap();
    let rid = body_json(resp).await["id"].as_i64().unwrap();
    app.clone().oneshot(auth_req("POST", &format!("/api/rooms/{}/join", rid), &user_tok, None)).await.unwrap();
    let resp = app.clone().oneshot(auth_req("DELETE", &format!("/api/rooms/{}/members/kickee", rid), &tok, None)).await.unwrap();
    assert_eq!(resp.status(), 204);
}
