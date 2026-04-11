use super::*;

#[derive(Deserialize)]
pub struct ExportQuery { pub format: Option<String>, pub from: Option<String>, pub to: Option<String> }

#[utoipa::path(get, path = "/api/export/tasks", responses((status = 200)), security(("bearer" = [])))]
pub async fn export_tasks(State(engine): State<AppState>, claims: Claims, Query(q): Query<ExportQuery>) -> Result<axum::response::Response, ApiError> {
    // Non-root users only export their own tasks
    let tasks = if claims.role == "root" {
        db::list_tasks(&engine.pool, None, None).await.map_err(internal)?
    } else {
        db::list_tasks(&engine.pool, None, None).await.map_err(internal)?
            .into_iter().filter(|t| t.user_id == claims.user_id).collect()
    };
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
    let sessions = db::get_history(&engine.pool, from, to, Some(claims.user_id)).await.map_err(internal)?;
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
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
