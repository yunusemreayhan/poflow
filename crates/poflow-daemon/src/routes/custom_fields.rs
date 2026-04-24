use super::*;

const VALID_FIELD_TYPES: &[&str] = &["text", "number", "select", "date", "user"];

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateFieldRequest {
    pub name: String,
    pub field_type: Option<String>,
    pub options: Option<Vec<String>>, // for select type
    pub required: Option<bool>,
    pub sort_order: Option<i64>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetFieldValueRequest {
    pub value: Option<String>,
}

// ── Field definitions CRUD ─────────────────────────────────────

#[utoipa::path(get, path = "/api/fields", responses((status = 200, body = Vec<db::CustomField>)), security(("bearer" = [])))]
pub async fn list_custom_fields(
    State(engine): State<AppState>,
    _claims: Claims,
) -> ApiResult<Vec<db::CustomField>> {
    db::list_custom_fields(&engine.pool)
        .await
        .map(Json)
        .map_err(internal)
}

#[utoipa::path(post, path = "/api/fields", request_body = CreateFieldRequest, responses((status = 201, body = db::CustomField)), security(("bearer" = [])))]
pub async fn create_custom_field(
    State(engine): State<AppState>,
    claims: Claims,
    Json(req): Json<CreateFieldRequest>,
) -> Result<(StatusCode, Json<db::CustomField>), ApiError> {
    if !auth::is_admin_or_root(&claims) {
        return Err(err(StatusCode::FORBIDDEN, "Admin or root required"));
    }
    let name = req.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "Field name must be 1-100 chars",
        ));
    }
    let ft = req.field_type.as_deref().unwrap_or("text");
    if !VALID_FIELD_TYPES.contains(&ft) {
        return Err(err(
            StatusCode::BAD_REQUEST,
            format!(
                "field_type must be one of: {}",
                VALID_FIELD_TYPES.join(", ")
            ),
        ));
    }
    let options = if ft == "select" {
        let opts = req.options.as_ref().ok_or_else(|| {
            err(
                StatusCode::BAD_REQUEST,
                "Select fields require options array",
            )
        })?;
        if opts.is_empty() {
            return Err(err(StatusCode::BAD_REQUEST, "Options cannot be empty"));
        }
        Some(serde_json::to_string(opts).map_err(internal)?)
    } else {
        None
    };
    let field = db::create_custom_field(
        &engine.pool,
        name,
        ft,
        options.as_deref(),
        req.required.unwrap_or(false),
        req.sort_order.unwrap_or(0),
        claims.user_id,
    )
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            err(StatusCode::CONFLICT, "Field name already exists")
        } else {
            internal(e)
        }
    })?;
    Ok((StatusCode::CREATED, Json(field)))
}

#[utoipa::path(put, path = "/api/fields/{id}", request_body = CreateFieldRequest, responses((status = 200, body = db::CustomField)), security(("bearer" = [])))]
pub async fn update_custom_field(
    State(engine): State<AppState>,
    claims: Claims,
    Path(id): Path<i64>,
    Json(req): Json<CreateFieldRequest>,
) -> ApiResult<db::CustomField> {
    if !auth::is_admin_or_root(&claims) {
        return Err(err(StatusCode::FORBIDDEN, "Admin or root required"));
    }
    let name = req.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "Field name must be 1-100 chars",
        ));
    }
    let ft = req.field_type.as_deref().unwrap_or("text");
    if !VALID_FIELD_TYPES.contains(&ft) {
        return Err(err(StatusCode::BAD_REQUEST, "Invalid field_type"));
    }
    let options = if ft == "select" {
        req.options
            .as_ref()
            .map(|o| serde_json::to_string(o).unwrap_or_default())
    } else {
        None
    };
    db::update_custom_field(
        &engine.pool,
        id,
        name,
        ft,
        options.as_deref(),
        req.required.unwrap_or(false),
        req.sort_order.unwrap_or(0),
    )
    .await
    .map(Json)
    .map_err(|e| {
        if e.to_string().contains("not found") {
            err(StatusCode::NOT_FOUND, "Field not found")
        } else {
            internal(e)
        }
    })
}

#[utoipa::path(delete, path = "/api/fields/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_custom_field(
    State(engine): State<AppState>,
    claims: Claims,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    if !auth::is_admin_or_root(&claims) {
        return Err(err(StatusCode::FORBIDDEN, "Admin or root required"));
    }
    db::delete_custom_field(&engine.pool, id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Field not found"))?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Task field values ──────────────────────────────────────────

#[utoipa::path(get, path = "/api/tasks/{id}/fields", responses((status = 200, body = Vec<db::TaskFieldValue>)), security(("bearer" = [])))]
pub async fn get_task_fields(
    State(engine): State<AppState>,
    _claims: Claims,
    Path(id): Path<i64>,
) -> ApiResult<Vec<db::TaskFieldValue>> {
    db::get_task(&engine.pool, id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    db::get_task_field_values(&engine.pool, id)
        .await
        .map(Json)
        .map_err(internal)
}

#[utoipa::path(put, path = "/api/tasks/{task_id}/fields/{field_id}", request_body = SetFieldValueRequest, responses((status = 204)), security(("bearer" = [])))]
pub async fn set_task_field_value(
    State(engine): State<AppState>,
    claims: Claims,
    Path((task_id, field_id)): Path<(i64, i64)>,
    Json(req): Json<SetFieldValueRequest>,
) -> Result<StatusCode, ApiError> {
    let task = db::get_task(&engine.pool, task_id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    if !is_owner_or_root(task.user_id, &claims) {
        let assignees = db::list_assignees(&engine.pool, task_id)
            .await
            .map_err(internal)?;
        if !assignees.contains(&claims.username) {
            return Err(err(StatusCode::FORBIDDEN, "Not owner or assignee"));
        }
    }
    // Verify field exists
    sqlx::query("SELECT 1 FROM custom_fields WHERE id = ?")
        .bind(field_id)
        .fetch_one(&engine.pool)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Field not found"))?;
    db::set_task_field_value(&engine.pool, task_id, field_id, req.value.as_deref())
        .await
        .map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/api/tasks/{task_id}/fields/{field_id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_task_field_value(
    State(engine): State<AppState>,
    claims: Claims,
    Path((task_id, field_id)): Path<(i64, i64)>,
) -> Result<StatusCode, ApiError> {
    let task = db::get_task(&engine.pool, task_id)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    if !is_owner_or_root(task.user_id, &claims) {
        return Err(err(StatusCode::FORBIDDEN, "Not owner"));
    }
    db::delete_task_field_value(&engine.pool, task_id, field_id)
        .await
        .map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}
