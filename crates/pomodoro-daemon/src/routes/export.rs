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
            let mut csv = String::from("id,parent_id,title,description,project,tags,priority,estimated,actual,status,due_date,created_at,work_duration_minutes\n");
            for t in &tasks {
                csv.push_str(&format!("{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                    t.id,
                    t.parent_id.map(|p| p.to_string()).unwrap_or_default(),
                    escape_csv(&t.title),
                    escape_csv(t.description.as_deref().unwrap_or("")),
                    escape_csv(t.project.as_deref().unwrap_or("")),
                    escape_csv(t.tags.as_deref().unwrap_or("")),
                    t.priority, t.estimated, t.actual, t.status,
                    t.due_date.as_deref().unwrap_or(""),
                    t.created_at,
                    t.work_duration_minutes.map(|m| m.to_string()).unwrap_or_default(),
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
    // V3: Validate date format
    if chrono::NaiveDate::parse_from_str(from, "%Y-%m-%d").is_err() { return Err(err(StatusCode::BAD_REQUEST, "Invalid 'from' date format (expected YYYY-MM-DD)")); }
    if chrono::NaiveDate::parse_from_str(to, "%Y-%m-%d").is_err() { return Err(err(StatusCode::BAD_REQUEST, "Invalid 'to' date format (expected YYYY-MM-DD)")); }
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
pub async fn export_burns(State(engine): State<AppState>, claims: Claims, Path(sprint_id): Path<i64>) -> Result<axum::response::Response, ApiError> {
    // B14: Verify sprint ownership
    let sprint = db::get_sprint(&engine.pool, sprint_id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Sprint not found"))?;
    if !is_owner_or_root(sprint.created_by_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not sprint owner")); }
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
    let mut tx = engine.pool.begin().await.map_err(internal)?;
    // Detect header to determine column mapping
    let mut lines = req.csv.lines();
    let header = match lines.next() {
        Some(h) => h,
        None => return Ok(Json(serde_json::json!({ "created": 0, "errors": [] }))),
    };
    let hcols = parse_csv_line(header);
    let hcols: Vec<&str> = hcols.iter().map(|s| s.trim()).collect();
    // Build column index map from header
    let col_idx = |name: &str| hcols.iter().position(|h| h.eq_ignore_ascii_case(name));
    let idx_title = col_idx("title").unwrap_or(0);
    let idx_priority = col_idx("priority");
    let idx_estimated = col_idx("estimated");
    let idx_project = col_idx("project");
    let idx_description = col_idx("description");
    let idx_tags = col_idx("tags");
    let idx_due_date = col_idx("due_date");
    for (i, line) in lines.enumerate() {
        let cols = parse_csv_line(line);
        if cols.is_empty() || cols.get(idx_title).map(|s| s.trim().is_empty()).unwrap_or(true) { continue; }
        let title = cols[idx_title].trim().to_string();
        if title.len() > 500 { errors.push(format!("Line {}: title too long", i + 2)); continue; }
        let priority = idx_priority.and_then(|i| cols.get(i)).and_then(|s| s.trim().parse::<i64>().ok()).unwrap_or(3).clamp(1, 5);
        let estimated = idx_estimated.and_then(|i| cols.get(i)).and_then(|s| s.trim().parse::<i64>().ok()).unwrap_or(0);
        let project = idx_project.and_then(|i| cols.get(i)).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let description = idx_description.and_then(|i| cols.get(i)).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let tags = idx_tags.and_then(|i| cols.get(i)).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let due_date = idx_due_date.and_then(|i| cols.get(i)).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        // B3/V1: Validate due_date format
        if let Some(ref d) = due_date { if !valid_date(d) { errors.push(format!("Line {}: invalid due_date '{}'", i + 2, d)); continue; } }
        let now = db::now_str();
        if let Err(e) = sqlx::query("INSERT INTO tasks (user_id, title, description, project, tags, priority, estimated, actual, estimated_hours, remaining_points, due_date, status, sort_order, created_at, updated_at) VALUES (?,?,?,?,?,?,?,0,0.0,0.0,?,'backlog',0,?,?)")
            .bind(claims.user_id).bind(&title).bind(description.as_deref()).bind(project.as_deref()).bind(tags.as_deref()).bind(priority).bind(estimated).bind(due_date.as_deref()).bind(&now).bind(&now)
            .execute(&mut *tx).await {
            tx.rollback().await.ok();
            // B3: Reset created count since rollback undid all inserts
            return Err(internal(format!("Line {}: {} ({} rows rolled back)", i + 2, e, created)));
        }
        created += 1;
    }
    tx.commit().await.map_err(internal)?;
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
    // V2: Count total tasks including children
    fn count_tasks(tasks: &[ImportJsonTask]) -> usize {
        tasks.iter().map(|t| 1 + t.children.as_ref().map(|c| count_tasks(c)).unwrap_or(0)).sum()
    }
    let total = count_tasks(&req.tasks);
    if total > 2000 { return Err(err(StatusCode::BAD_REQUEST, format!("Too many total tasks including children ({}, max 2000)", total))); }
    let mut created = 0i64;
    let mut tx = engine.pool.begin().await.map_err(internal)?;
    async fn import_tree(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>, user_id: i64, tasks: &[ImportJsonTask], parent_id: Option<i64>, created: &mut i64, depth: u32) -> Result<(), String> {
        if depth > 20 { return Err("Max nesting depth (20) exceeded".to_string()); }
        for t in tasks {
            if t.title.trim().is_empty() { return Err("Title cannot be empty".to_string()); }
            if t.title.len() > 500 { return Err(format!("Title too long: {}", t.title.chars().take(50).collect::<String>())); }
            let now = db::now_str();
            let priority = t.priority.unwrap_or(3).clamp(1, 5);
            let estimated = t.estimated.unwrap_or(0);
            let id = sqlx::query("INSERT INTO tasks (parent_id, user_id, title, description, project, priority, estimated, actual, estimated_hours, remaining_points, status, sort_order, created_at, updated_at) VALUES (?,?,?,?,?,?,?,0,0.0,0.0,'backlog',0,?,?)")
                .bind(parent_id).bind(user_id).bind(t.title.trim()).bind(t.description.as_deref()).bind(t.project.as_deref()).bind(priority).bind(estimated).bind(&now).bind(&now)
                .execute(&mut **tx).await.map_err(|e| e.to_string())?.last_insert_rowid();
            *created += 1;
            if let Some(children) = &t.children {
                Box::pin(import_tree(tx, user_id, children, Some(id), created, depth + 1)).await?;
            }
        }
        Ok(())
    }
    let mut errors = Vec::new();
    match import_tree(&mut tx, claims.user_id, &req.tasks, None, &mut created, 0).await {
        Ok(()) => { tx.commit().await.map_err(internal)?; }
        Err(e) => { tx.rollback().await.ok(); created = 0; errors.push(e); }
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

// F4: iCal feed — tasks with due dates + sprint date ranges
#[utoipa::path(get, path = "/api/export/ical", responses((status = 200)), security(("bearer" = [])))]
pub async fn export_ical(State(engine): State<AppState>, claims: Claims) -> Result<axum::response::Response, ApiError> {
    let user_filter = if claims.role == "root" { None } else { Some(claims.user_id) };
    let filter = db::TaskFilter { status: None, project: None, search: None, assignee: None, due_before: None, due_after: None, priority: None, team_id: None, user_id: user_filter };
    let tasks = db::list_tasks_paged(&engine.pool, filter, 50000, 0).await.map_err(internal)?;
    let sprints = db::list_sprints(&engine.pool, None, None).await.map_err(internal)?;

    let mut ical = String::from("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//PomodoroLinux//EN\r\nCALSCALE:GREGORIAN\r\nMETHOD:PUBLISH\r\n");
    for t in &tasks {
        if let Some(ref due) = t.due_date {
            let uid = format!("task-{}@pomodoro", t.id);
            let summary = ical_escape(&t.title);
            let date = due.replace('-', "");
            ical.push_str(&format!("BEGIN:VEVENT\r\nUID:{}\r\nDTSTART;VALUE=DATE:{}\r\nSUMMARY:{}\r\n", uid, date, summary));
            if let Some(ref desc) = t.description { ical.push_str(&format!("DESCRIPTION:{}\r\n", ical_escape(desc))); }
            if t.priority >= 4 { ical.push_str("PRIORITY:1\r\n"); }
            ical.push_str("END:VEVENT\r\n");
        }
    }
    for s in &sprints {
        if let (Some(ref start), Some(ref end)) = (&s.start_date, &s.end_date) {
            let uid = format!("sprint-{}@pomodoro", s.id);
            ical.push_str(&format!("BEGIN:VEVENT\r\nUID:{}\r\nDTSTART;VALUE=DATE:{}\r\nDTEND;VALUE=DATE:{}\r\nSUMMARY:Sprint: {}\r\n",
                uid, start.replace('-', ""), end.replace('-', ""), ical_escape(&s.name)));
            if let Some(ref goal) = s.goal { ical.push_str(&format!("DESCRIPTION:{}\r\n", ical_escape(goal))); }
            ical.push_str("END:VEVENT\r\n");
        }
    }
    ical.push_str("END:VCALENDAR\r\n");

    Ok(axum::response::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/calendar; charset=utf-8")
        .header("content-disposition", "attachment; filename=\"pomodoro.ics\"")
        .body(axum::body::Body::from(ical)).map_err(|e| internal(e.to_string()))?)
}

fn ical_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace(';', "\\;").replace(',', "\\,").replace(':', "\\:").replace('\n', "\\n").replace('\r', "")
}
