use super::*;
use axum::body::Bytes;

const MAX_ATTACHMENT_SIZE: usize = 10 * 1024 * 1024; // 10MB

#[utoipa::path(get, path = "/api/tasks/{id}/attachments", responses((status = 200)), security(("bearer" = [])))]
pub async fn list_attachments(
    State(engine): State<AppState>,
    _claims: Claims,
    Path(id): Path<i64>,
) -> ApiResult<Vec<db::Attachment>> {
    db::get_task(&engine.pool, id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    db::list_attachments(&engine.pool, id)
        .await
        .map(Json)
        .map_err(internal)
}

#[utoipa::path(post, path = "/api/tasks/{id}/attachments", responses((status = 201)), security(("bearer" = [])),
    request_body(content = String, content_type = "application/octet-stream"))]
pub async fn upload_attachment(
    State(engine): State<AppState>,
    claims: Claims,
    Path(task_id): Path<i64>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<db::Attachment>), ApiError> {
    if body.len() > MAX_ATTACHMENT_SIZE {
        return Err(err(
            StatusCode::PAYLOAD_TOO_LARGE,
            "File too large (max 10MB)",
        ));
    }
    if body.is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "Empty file"));
    }
    db::get_task(&engine.pool, task_id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;

    let filename = headers
        .get("x-filename")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unnamed");
    // Sanitize filename
    let safe_name: String = filename
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
        .collect();
    let safe_name = safe_name.trim_start_matches('.').to_string();
    let safe_name = if safe_name.is_empty() {
        "file".to_string()
    } else {
        safe_name
    };

    let mime = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream");

    // S3: Block dangerous MIME types that could enable XSS if served inline
    let mime_lower = mime.to_lowercase();
    if mime_lower.contains("html")
        || mime_lower.contains("javascript")
        || mime_lower.contains("svg")
        || mime_lower.contains("xml")
    {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "HTML, JavaScript, SVG, and XML files are not allowed",
        ));
    }

    // Generate unique storage key (portable, collision-resistant)
    let random_hex = {
        let mut buf = [0u8; 8];
        getrandom::fill(&mut buf).map_err(|e| internal(format!("getrandom failed: {e}")))?;
        buf.iter().map(|b| format!("{:02x}", b)).collect::<String>()
    };
    let key = format!("{}_{}", random_hex, &safe_name);
    let path = db::attachments_dir().join(&key);

    tokio::fs::write(&path, &body)
        .await
        .map_err(|e| internal(format!("Failed to write file: {}", e)))?;

    let att = db::create_attachment(
        &engine.pool,
        task_id,
        claims.user_id,
        &safe_name,
        mime,
        body.len() as i64,
        &key,
    )
    .await
    .map_err(internal)?;
    Ok((StatusCode::CREATED, Json(att)))
}

#[utoipa::path(get, path = "/api/attachments/{id}/download", responses((status = 200)), security(("bearer" = [])))]
pub async fn download_attachment(
    State(engine): State<AppState>,
    _claims: Claims,
    Path(id): Path<i64>,
) -> Result<axum::response::Response, ApiError> {
    let att = db::get_attachment(&engine.pool, id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Attachment not found"))?;
    let path = db::attachments_dir().join(&att.storage_key);
    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "File not found on disk"))?;
    let stream = tokio_util::io::ReaderStream::new(file);

    // S3: Force safe content-type to prevent XSS via uploaded HTML/SVG
    let safe_mime = if att.mime_type.starts_with("image/")
        || att.mime_type == "application/pdf"
        || att.mime_type.starts_with("text/plain")
    {
        &att.mime_type
    } else {
        "application/octet-stream"
    };

    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", safe_mime)
        .header(
            "content-disposition",
            format!(
                "attachment; filename=\"{}\"",
                att.filename.replace('"', "_")
            ),
        )
        .header("content-security-policy", "default-src 'none'")
        .header("x-content-type-options", "nosniff")
        .body(axum::body::Body::from_stream(stream))
        .map_err(|e| internal(e.to_string()))
}

#[utoipa::path(delete, path = "/api/attachments/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_attachment(
    State(engine): State<AppState>,
    claims: Claims,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let att = db::get_attachment(&engine.pool, id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Attachment not found"))?;
    if att.user_id != claims.user_id && !auth::is_admin_or_root(&claims) {
        return Err(err(StatusCode::FORBIDDEN, "Not owner"));
    }
    let key = db::delete_attachment(&engine.pool, id)
        .await
        .map_err(internal)?;
    let path = db::attachments_dir().join(&key);
    let _ = tokio::fs::remove_file(&path).await;
    Ok(StatusCode::NO_CONTENT)
}
