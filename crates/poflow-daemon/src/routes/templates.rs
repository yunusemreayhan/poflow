use super::*;

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateTemplateRequest { pub name: String, pub data: serde_json::Value }

#[utoipa::path(get, path = "/api/templates", responses((status = 200)), security(("bearer" = [])))]
pub async fn list_templates(State(engine): State<AppState>, claims: Claims) -> ApiResult<Vec<db::TaskTemplate>> {
    db::list_templates(&engine.pool, claims.user_id).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/templates", responses((status = 201)), security(("bearer" = [])))]
pub async fn create_template(State(engine): State<AppState>, claims: Claims, Json(req): Json<CreateTemplateRequest>) -> Result<(StatusCode, Json<db::TaskTemplate>), ApiError> {
    if req.name.trim().is_empty() { return Err(err(StatusCode::BAD_REQUEST, "Name required")); }
    if req.name.len() > 200 { return Err(err(StatusCode::BAD_REQUEST, "Name too long (max 200 chars)")); }
    let data = serde_json::to_string(&req.data).map_err(internal)?;
    if data.len() > 65536 { return Err(err(StatusCode::BAD_REQUEST, "Template data too large (max 64KB)")); }
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM task_templates WHERE user_id = ?")
        .bind(claims.user_id).fetch_one(&engine.pool).await.map_err(internal)?;
    if count.0 >= 100 { return Err(err(StatusCode::BAD_REQUEST, "Template limit reached (max 100)")); }
    let t = db::create_template(&engine.pool, claims.user_id, req.name.trim(), &data).await.map_err(internal)?;
    Ok((StatusCode::CREATED, Json(t)))
}

#[utoipa::path(delete, path = "/api/templates/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_template(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    // S9: Verify ownership — templates are per-user
    let tmpl: (i64,) = sqlx::query_as("SELECT user_id FROM task_templates WHERE id = ?")
        .bind(id).fetch_one(&engine.pool).await.map_err(|_| err(StatusCode::NOT_FOUND, "Template not found"))?;
    if !is_owner_or_root(tmpl.0, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    db::delete_template(&engine.pool, id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

// V32-16: Update template name/data
#[utoipa::path(put, path = "/api/templates/{id}", responses((status = 200)), security(("bearer" = [])))]
pub async fn update_template(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<CreateTemplateRequest>) -> ApiResult<db::TaskTemplate> {
    let tmpl: (i64,) = sqlx::query_as("SELECT user_id FROM task_templates WHERE id = ?")
        .bind(id).fetch_one(&engine.pool).await.map_err(|_| err(StatusCode::NOT_FOUND, "Template not found"))?;
    if !is_owner_or_root(tmpl.0, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    if req.name.trim().is_empty() { return Err(err(StatusCode::BAD_REQUEST, "Name required")); }
    if req.name.len() > 200 { return Err(err(StatusCode::BAD_REQUEST, "Name too long (max 200 chars)")); }
    let data = serde_json::to_string(&req.data).map_err(internal)?;
    if data.len() > 65536 { return Err(err(StatusCode::BAD_REQUEST, "Template data too large (max 64KB)")); }
    sqlx::query("UPDATE task_templates SET name = ?, data = ? WHERE id = ?")
        .bind(req.name.trim()).bind(&data).bind(id).execute(&engine.pool).await.map_err(internal)?;
    let updated = sqlx::query_as::<_, db::TaskTemplate>("SELECT * FROM task_templates WHERE id = ?")
        .bind(id).fetch_one(&engine.pool).await.map_err(internal)?;
    Ok(Json(updated))
}

// F11: Instantiate template with variable resolution + checklists/labels/custom fields
#[utoipa::path(post, path = "/api/templates/{id}/instantiate", responses((status = 201)), security(("bearer" = [])))]
pub async fn instantiate_template(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<(StatusCode, Json<db::Task>), ApiError> {
    let tmpl = db::get_template(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Template not found"))?;
    if !is_owner_or_root(tmpl.user_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    let data: serde_json::Value = serde_json::from_str(&tmpl.data).map_err(internal)?;
    let today = chrono::Utc::now().naive_utc().format("%Y-%m-%d").to_string();
    let resolve = |s: &str| s.replace("{{today}}", &today).replace("{{username}}", &claims.username);
    let title = resolve(data["title"].as_str().unwrap_or(&tmpl.name));
    let desc = data["description"].as_str().map(&resolve);
    let project = data["project"].as_str().map(|s| s.to_string());
    let priority = data["priority"].as_i64().unwrap_or(3).clamp(1, 5);
    let estimated = data["estimated"].as_i64().unwrap_or(0).max(0);
    let tags = data["tags"].as_str().map(&resolve);
    let due_date = data["due_date"].as_str().map(resolve);
    let estimated_hours = data["estimated_hours"].as_f64().unwrap_or(0.0).max(0.0);
    let t = db::create_task(&engine.pool, db::CreateTaskOpts {
        user_id: claims.user_id, parent_id: None, title: &title, description: desc.as_deref(),
        project: project.as_deref(), project_id: None, tags: tags.as_deref(), priority, estimated,
        estimated_hours, remaining_points: 0.0, due_date: due_date.as_deref(),
    }).await.map_err(internal)?;

    // Copy checklist items from template data
    if let Some(items) = data["checklist"].as_array() {
        for (i, item) in items.iter().enumerate() {
            if let Some(title) = item.as_str().or_else(|| item["title"].as_str()) {
                db::add_checklist_item(&engine.pool, t.id, title, i as i64).await.ok();
            }
        }
    }

    // Copy labels from template data
    if let Some(labels) = data["labels"].as_array() {
        for label in labels {
            if let Some(name) = label.as_str().or_else(|| label["name"].as_str()) {
                let lid: Option<(i64,)> = sqlx::query_as("SELECT id FROM labels WHERE name = ?")
                    .bind(name).fetch_optional(&engine.pool).await.unwrap_or(None);
                if let Some((lid,)) = lid { db::add_task_label(&engine.pool, t.id, lid).await.ok(); }
            }
        }
    }

    // Copy custom field values from template data
    if let Some(fields) = data["custom_fields"].as_object() {
        for (field_name, value) in fields {
            let fid: Option<(i64,)> = sqlx::query_as("SELECT id FROM custom_fields WHERE name = ?")
                .bind(field_name).fetch_optional(&engine.pool).await.unwrap_or(None);
            if let Some((fid,)) = fid {
                db::set_task_field_value(&engine.pool, t.id, fid, value.as_str()).await.ok();
            }
        }
    }

    engine.notify(ChangeEvent::Tasks);
    Ok((StatusCode::CREATED, Json(t)))
}

// Save an existing task as a template (captures checklist, labels, custom fields)
#[utoipa::path(post, path = "/api/tasks/{id}/save-as-template", responses((status = 201)), security(("bearer" = [])))]
pub async fn save_task_as_template(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<(StatusCode, Json<db::TaskTemplate>), ApiError> {
    let task = db::get_task(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Task not found"))?;
    if !is_owner_or_root(task.user_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }

    // Gather checklist items
    let checklist = db::list_checklist(&engine.pool, id).await.unwrap_or_default();
    let checklist_titles: Vec<&str> = checklist.iter().map(|c| c.title.as_str()).collect();

    // Gather labels
    let labels: Vec<(String,)> = sqlx::query_as("SELECT l.name FROM task_labels tl JOIN labels l ON l.id = tl.label_id WHERE tl.task_id = ?")
        .bind(id).fetch_all(&engine.pool).await.unwrap_or_default();
    let label_names: Vec<&str> = labels.iter().map(|(n,)| n.as_str()).collect();

    // Gather custom field values
    let cf_values = db::get_task_field_values(&engine.pool, id).await.unwrap_or_default();
    let mut custom_fields = serde_json::Map::new();
    for fv in &cf_values {
        if let Some(v) = &fv.value { custom_fields.insert(fv.field_name.clone(), serde_json::Value::String(v.clone())); }
    }

    let data = serde_json::json!({
        "title": task.title,
        "description": task.description,
        "project": task.project,
        "priority": task.priority,
        "estimated": task.estimated,
        "estimated_hours": task.estimated_hours,
        "tags": task.tags,
        "checklist": checklist_titles,
        "labels": label_names,
        "custom_fields": custom_fields,
    });

    let data_str = serde_json::to_string(&data).map_err(internal)?;
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM task_templates WHERE user_id = ?")
        .bind(claims.user_id).fetch_one(&engine.pool).await.map_err(internal)?;
    if count.0 >= 100 { return Err(err(StatusCode::BAD_REQUEST, "Template limit reached (max 100)")); }
    let tmpl = db::create_template(&engine.pool, claims.user_id, &format!("Template: {}", task.title), &data_str).await.map_err(internal)?;
    Ok((StatusCode::CREATED, Json(tmpl)))
}
