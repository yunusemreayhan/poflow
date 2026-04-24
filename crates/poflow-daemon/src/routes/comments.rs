use super::*;

#[utoipa::path(get, path = "/api/tasks/{id}/comments", responses((status = 200, body = Vec<db::Comment>)), security(("bearer" = [])))]
pub async fn list_comments(
    State(engine): State<AppState>,
    _claims: Claims,
    Path(id): Path<i64>,
) -> ApiResult<Vec<db::Comment>> {
    db::get_task(&engine.pool, id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    db::list_comments(&engine.pool, id)
        .await
        .map(Json)
        .map_err(internal)
}

#[utoipa::path(post, path = "/api/tasks/{id}/comments", request_body = AddCommentRequest, responses((status = 201, body = db::Comment)), security(("bearer" = [])))]
pub async fn add_comment(
    State(engine): State<AppState>,
    claims: Claims,
    Path(id): Path<i64>,
    Json(req): Json<AddCommentRequest>,
) -> Result<(StatusCode, Json<db::Comment>), ApiError> {
    if req.content.trim().is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "Comment cannot be empty"));
    }
    if req.content.len() > 10000 {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "Comment too long (max 10000 chars)",
        ));
    }
    // V30-10: Rate limit — max 30 comments per minute per user
    let one_min_ago = (chrono::Utc::now() - chrono::Duration::seconds(60))
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string();
    let (recent,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM comments WHERE user_id = ? AND created_at > ?")
            .bind(claims.user_id)
            .bind(&one_min_ago)
            .fetch_one(&engine.pool)
            .await
            .map_err(internal)?;
    if recent >= 30 {
        return Err(err(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many comments — max 30 per minute",
        ));
    }
    // V7: Validate task exists
    db::get_task(&engine.pool, id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    if let Some(sid) = req.session_id {
        sqlx::query("SELECT 1 FROM sessions WHERE id = ?")
            .bind(sid)
            .fetch_one(&engine.pool)
            .await
            .map_err(|_| err(StatusCode::NOT_FOUND, "Session not found"))?;
    }
    if let Some(pid) = req.parent_id {
        let parent = db::get_comment(&engine.pool, pid)
            .await
            .map_err(|_| err(StatusCode::NOT_FOUND, "Parent comment not found"))?;
        if parent.task_id != id {
            return Err(err(
                StatusCode::BAD_REQUEST,
                "Parent comment belongs to a different task",
            ));
        }
    }
    db::add_comment(
        &engine.pool,
        claims.user_id,
        id,
        req.session_id,
        &req.content,
        req.parent_id,
    )
    .await
    .map(|c| {
        // BL23: Notify @mentioned users
        let pool = engine.pool.clone();
        let content = req.content.clone();
        let task_id = id;
        let commenter_id = claims.user_id;
        let commenter_name = claims.username.clone();
        tokio::spawn(async move {
            // Collect @mentioned user IDs to avoid double-notifying
            let mut mentioned_ids = std::collections::HashSet::new();
            for word in content.split_whitespace() {
                if let Some(username) = word.strip_prefix('@') {
                    let username = username
                        .trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '-');
                    if let Ok(Some(uid)) = db::get_user_id_by_username(&pool, username).await {
                        mentioned_ids.insert(uid);
                        if let Err(e) = db::create_notification(
                            &pool,
                            uid,
                            "mention",
                            &format!("You were mentioned in a comment on task #{}", task_id),
                            Some("task"),
                            Some(task_id),
                        )
                        .await
                        {
                            tracing::warn!(
                                "Failed to create @mention notification for {}: {}",
                                username,
                                e
                            );
                        }
                    }
                }
            }
            // Notify watchers (skip commenter and already-mentioned users)
            if let Ok(watcher_ids) = db::get_task_watcher_ids(&pool, task_id).await {
                for wid in watcher_ids {
                    if wid != commenter_id && !mentioned_ids.contains(&wid) {
                        if let Err(e) = db::create_notification(
                            &pool,
                            wid,
                            "comment_added",
                            &format!("{} commented on task #{}", commenter_name, task_id),
                            Some("task"),
                            Some(task_id),
                        )
                        .await
                        {
                            tracing::warn!("Failed to create watcher comment notification: {}", e);
                        }
                    }
                }
            }
        });
        engine.notify(ChangeEvent::Tasks);
        (StatusCode::CREATED, Json(c))
    })
    .map_err(internal)
}

#[utoipa::path(delete, path = "/api/comments/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_comment(
    State(engine): State<AppState>,
    claims: Claims,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let comment = db::get_comment(&engine.pool, id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Comment not found"))?;
    if !is_owner_or_root(comment.user_id, &claims) {
        return Err(err(StatusCode::FORBIDDEN, "Not owner"));
    }
    db::delete_comment(&engine.pool, id)
        .await
        .map_err(internal)?;
    engine.notify(ChangeEvent::Tasks);
    Ok(StatusCode::NO_CONTENT)
}

// F8: Edit comment (owner only, within 15 minutes)
#[derive(Deserialize, utoipa::ToSchema)]
pub struct EditCommentRequest {
    pub content: String,
}

#[utoipa::path(put, path = "/api/comments/{id}", request_body = EditCommentRequest, responses((status = 200, body = db::Comment)), security(("bearer" = [])))]
pub async fn edit_comment(
    State(engine): State<AppState>,
    claims: Claims,
    Path(id): Path<i64>,
    Json(req): Json<EditCommentRequest>,
) -> ApiResult<db::Comment> {
    if req.content.trim().is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "Comment cannot be empty"));
    }
    if req.content.len() > 10000 {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "Comment too long (max 10000 chars)",
        ));
    }
    let comment = db::get_comment(&engine.pool, id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Comment not found"))?;
    if !is_owner_or_root(comment.user_id, &claims) {
        return Err(err(StatusCode::FORBIDDEN, "Not owner"));
    }
    // B4: 15-minute edit window — reject if parse fails (don't silently skip)
    let created =
        chrono::NaiveDateTime::parse_from_str(&comment.created_at, "%Y-%m-%dT%H:%M:%S%.f")
            .or_else(|_| {
                chrono::NaiveDateTime::parse_from_str(&comment.created_at, "%Y-%m-%dT%H:%M:%S")
            })
            .map_err(|_| {
                err(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Cannot parse comment timestamp",
                )
            })?;
    let elapsed = chrono::Utc::now().naive_utc() - created;
    if elapsed.num_minutes() > 15 {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "Edit window expired (15 minutes)",
        ));
    }
    sqlx::query("UPDATE comments SET content = ? WHERE id = ?")
        .bind(&req.content)
        .bind(id)
        .execute(&engine.pool)
        .await
        .map_err(internal)?;
    let updated = db::get_comment(&engine.pool, id).await.map_err(internal)?;
    engine.notify(ChangeEvent::Tasks);
    Ok(Json(updated))
}

// --- Task Time/Burns ---
