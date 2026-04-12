use super::*;

#[derive(Deserialize)]
pub struct ExportQuery { pub format: Option<String>, pub from: Option<String>, pub to: Option<String> }

#[utoipa::path(get, path = "/api/export/tasks", responses((status = 200)), security(("bearer" = [])))]
pub async fn export_tasks(State(engine): State<AppState>, claims: Claims, Query(q): Query<ExportQuery>) -> Result<axum::response::Response, ApiError> {
    let user_filter = if claims.role == "root" { None } else { Some(claims.user_id) };
    let filter = db::TaskFilter { status: None, project: None, search: None, assignee: None, due_before: None, due_after: None, priority: None, team_id: None, user_id: user_filter };
    let tasks = db::list_tasks_paged(&engine.pool, filter, 50000, 0).await.map_err(internal)?;
    let fmt = q.format.as_deref().unwrap_or("json");
    match fmt {
        "csv" => {
            let mut csv = String::from("id,parent_id,title,project,tags,priority,estimated,actual,status,due_date,created_at\n");
            for t in &tasks {
                csv.push_str(&format!("{},{},{},{},{},{},{},{},{},{},{}\n",
                    t.id,
                    t.parent_id.map(|p| p.to_string()).unwrap_or_default(),
                    escape_csv(&t.title),
                    escape_csv(t.project.as_deref().unwrap_or("")),
                    escape_csv(t.tags.as_deref().unwrap_or("")),
                    t.priority, t.estimated, t.actual, t.status,
                    t.due_date.as_deref().unwrap_or(""),
                    t.created_at,
                ));
            }
            Ok(axum::response::Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/csv")
                .header("content-disposition", "attachment; filename=\"tasks.csv\"")
                .body(axum::body::Body::from(csv)).map_err(|e| internal(e.to_string()))?)
        }
        _ => {
            let body = serde_json::to_vec(&tasks).map_err(internal)?;
            Ok(axum::response::Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .header("content-disposition", "attachment; filename=\"tasks.json\"")
                .body(axum::body::Body::from(body)).map_err(|e| internal(e.to_string()))?)
        }
    }
}

#[utoipa::path(get, path = "/api/export/sessions", responses((status = 200)), security(("bearer" = [])))]
pub async fn export_sessions(State(engine): State<AppState>, claims: Claims, Query(q): Query<ExportQuery>) -> Result<axum::response::Response, ApiError> {
    let from = q.from.as_deref().unwrap_or("2000-01-01");
    let to = q.to.as_deref().unwrap_or("2099-12-31");
    let user_filter = if claims.role == "root" { None } else { Some(claims.user_id) };
    let sessions = db::get_history(&engine.pool, from, to, user_filter).await.map_err(internal)?;
    let fmt = q.format.as_deref().unwrap_or("csv");
    match fmt {
        "json" => {
            let body = serde_json::to_vec(&sessions).map_err(internal)?;
            Ok(axum::response::Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .header("content-disposition", "attachment; filename=\"sessions.json\"")
                .body(axum::body::Body::from(body)).map_err(|e| internal(e.to_string()))?)
        }
        _ => {
            let mut csv = String::from("id,task_id,user,session_type,status,started_at,ended_at,duration_s,task_path\n");
            for s in &sessions {
                csv.push_str(&format!("{},{},{},{},{},{},{},{},{}\n",
                    s.session.id, s.session.task_id.map(|t| t.to_string()).unwrap_or_default(),
                    escape_csv(&s.session.user), s.session.session_type, s.session.status, s.session.started_at,
                    s.session.ended_at.as_deref().unwrap_or(""), s.session.duration_s.unwrap_or(0),
                    escape_csv(&s.task_path.join(" > ")),
                ));
            }
            Ok(axum::response::Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/csv")
                .header("content-disposition", "attachment; filename=\"sessions.csv\"")
                .body(axum::body::Body::from(csv)).map_err(|e| internal(e.to_string()))?)
        }
    }
}

fn escape_csv(s: &str) -> String {
    // Prefix formula-triggering characters to prevent CSV injection in spreadsheet apps
    let needs_prefix = s.starts_with('=') || s.starts_with('+') || s.starts_with('-') || s.starts_with('@');
    let s = if needs_prefix { format!("'{}", s) } else { s.to_string() };
    // B6: Always quote prefixed fields + fields with special chars
    if needs_prefix || s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

#[utoipa::path(get, path = "/api/export/burns/{sprint_id}", responses((status = 200)), security(("bearer" = [])))]
pub async fn export_burns(State(engine): State<AppState>, _claims: Claims, Path(sprint_id): Path<i64>) -> Result<axum::response::Response, ApiError> {
    let burns = db::list_burns(&engine.pool, sprint_id).await.map_err(internal)?;
    let mut csv = String::from("created_at,task_id,points,hours,username,source,note\n");
    for b in &burns {
        csv.push_str(&format!("{},{},{},{},{},{},{}\n",
            b.created_at, b.task_id, b.points, b.hours,
            escape_csv(&b.username), escape_csv(&b.source),
            escape_csv(b.note.as_deref().unwrap_or(""))));
    }
    Ok(axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/csv")
        .header("content-disposition", &format!("attachment; filename=\"burns_sprint_{}.csv\"", sprint_id))
        .body(axum::body::Body::from(csv)).map_err(|e| internal(e.to_string()))?)
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ImportCsvRequest { pub csv: String }

#[utoipa::path(post, path = "/api/import/tasks", request_body = ImportCsvRequest, responses((status = 200)), security(("bearer" = [])))]
pub async fn import_tasks_csv(State(engine): State<AppState>, claims: Claims, Json(req): Json<ImportCsvRequest>) -> ApiResult<serde_json::Value> {
    if req.csv.len() > 1_048_576 { return Err(err(StatusCode::BAD_REQUEST, "CSV too large (max 1MB)")); }
    let mut created = 0i64;
    let mut errors = Vec::new();
    for (i, line) in req.csv.lines().enumerate() {
        if i == 0 { continue; }
        let cols = parse_csv_line(line);
        if cols.is_empty() || cols[0].trim().is_empty() { continue; }
        let title = cols[0].trim().to_string();
        let priority = cols.get(1).and_then(|s| s.trim().parse::<i64>().ok()).unwrap_or(3).clamp(1, 5);
        let estimated = cols.get(2).and_then(|s| s.trim().parse::<i64>().ok()).unwrap_or(0);
        let project = cols.get(3).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        match db::create_task(&engine.pool, claims.user_id, None, &title, None, project.as_deref(), None, priority, estimated, 0.0, 0.0, None).await {
            Ok(_) => created += 1,
            Err(e) => errors.push(format!("Line {}: {}", i + 1, e)),
        }
    }
    engine.notify(ChangeEvent::Tasks);
    Ok(Json(serde_json::json!({ "created": created, "errors": errors })))
}

// F4: JSON task import
#[derive(Deserialize, utoipa::ToSchema)]
pub struct ImportJsonRequest { pub tasks: Vec<ImportJsonTask> }
#[derive(Deserialize, utoipa::ToSchema)]
pub struct ImportJsonTask {
    pub title: String, pub description: Option<String>, pub project: Option<String>,
    pub priority: Option<i64>, pub estimated: Option<i64>,
    #[schema(no_recursion)]
    pub children: Option<Vec<ImportJsonTask>>,
}

#[utoipa::path(post, path = "/api/import/tasks/json", responses((status = 200)), security(("bearer" = [])))]
pub async fn import_tasks_json(State(engine): State<AppState>, claims: Claims, Json(req): Json<ImportJsonRequest>) -> ApiResult<serde_json::Value> {
    if req.tasks.len() > 500 { return Err(err(StatusCode::BAD_REQUEST, "Too many tasks (max 500)")); }
    let mut created = 0i64;
    async fn import_tree(pool: &db::Pool, user_id: i64, tasks: &[ImportJsonTask], parent_id: Option<i64>, created: &mut i64, depth: u32) -> Result<(), String> {
        if depth > 20 { return Err("Max nesting depth (20) exceeded".to_string()); }
        for t in tasks {
            if t.title.trim().is_empty() { return Err("Title cannot be empty".to_string()); }
            if t.title.len() > 500 { return Err(format!("Title too long: {}", &t.title[..50])); }
            let task = db::create_task(pool, user_id, parent_id, &t.title, t.description.as_deref(), t.project.as_deref(), None, t.priority.unwrap_or(3).clamp(1, 5), t.estimated.unwrap_or(0), 0.0, 0.0, None)
                .await.map_err(|e| e.to_string())?;
            *created += 1;
            if let Some(children) = &t.children {
                Box::pin(import_tree(pool, user_id, children, Some(task.id), created, depth + 1)).await?;
            }
        }
        Ok(())
    }
    let mut errors = Vec::new();
    if let Err(e) = import_tree(&engine.pool, claims.user_id, &req.tasks, None, &mut created, 0).await {
        errors.push(e);
    }
    engine.notify(ChangeEvent::Tasks);
    Ok(Json(serde_json::json!({ "created": created, "errors": errors })))
}

/// Parse a CSV line respecting quoted fields (handles commas and escaped quotes inside quotes)
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') { chars.next(); current.push('"'); }
                else { in_quotes = false; }
            } else { current.push(c); }
        } else if c == '"' { in_quotes = true; }
        else if c == ',' { fields.push(std::mem::take(&mut current)); }
        else { current.push(c); }
    }
    fields.push(current);
    fields
}
