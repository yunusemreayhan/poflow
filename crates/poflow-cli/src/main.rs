use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::{json, Value};

#[derive(Parser)]
#[command(name = "pomo", about = "Poflow timer CLI")]
struct Cli {
    /// Server URL
    #[arg(long, default_value = "http://127.0.0.1:9090", env = "POFLOW_URL")]
    url: String,
    /// Auth token
    #[arg(long, env = "POFLOW_TOKEN")]
    token: Option<String>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Login and print token
    Login { username: String, password: String },
    /// Show current timer state
    Status,
    /// Start a work session
    Start { #[arg(short, long)] task: Option<i64> },
    /// Pause the timer
    Pause,
    /// Resume the timer
    Resume,
    /// Stop the timer
    Stop,
    /// Skip current phase
    Skip,
    /// List tasks
    Tasks { #[arg(short, long)] status: Option<String> },
    /// Add a task
    Add {
        title: String,
        #[arg(short, long, default_value = "3")]
        priority: i64,
        #[arg(short, long, default_value = "1")]
        estimated: i64,
        #[arg(long)]
        project: Option<String>,
    },
    /// Show today's stats
    Stats,
    /// List sprints
    Sprints { #[arg(short, long)] status: Option<String> },
    /// List labels
    Labels,
    /// Add a label to a task
    Label { task_id: i64, label_id: i64 },
    /// Show task dependencies
    Deps { task_id: i64 },
    /// Export tasks as CSV
    Export,
    /// Log time on a task
    Log { task_id: i64, hours: f64, #[arg(short, long)] note: Option<String> },
    /// Show focus score
    Score,
    /// List estimation rooms
    Rooms,
    /// Join a room
    JoinRoom { room_id: i64 },
    /// Vote in a room
    Vote { room_id: i64, value: f64 },
    /// Mark a task as completed
    Done { task_id: i64 },
    /// Update task status
    SetStatus { task_id: i64, status: String },
    /// Delete a task
    Delete { task_id: i64 },
    /// Search tasks
    Search { query: String },
    /// Show daily standup report
    Standup,
    /// Assign a user to a task
    Assign { task_id: i64, username: String },
    /// Add a comment to a task
    Comment { task_id: i64, text: String },
    /// Show task detail
    Show { task_id: i64 },
    /// List checklist items for a task
    Checklist { task_id: i64 },
    /// Add checklist item
    Check { task_id: i64, title: String },
}

async fn api(client: &reqwest::Client, base: &str, token: Option<&str>, method: &str, path: &str, body: Option<Value>) -> Result<Value> {
    let url = format!("{}{}", base, path);
    let mut req = match method {
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        _ => client.get(&url),
    };
    if let Some(t) = token { req = req.header("Authorization", format!("Bearer {}", t)); }
    if method != "GET" { req = req.header("x-requested-with", "pomo-cli"); }
    if let Some(b) = body { req = req.json(&b); }
    let resp = req.send().await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() { anyhow::bail!("{}: {}", status, text); }
    if text.is_empty() { return Ok(Value::Null); }
    Ok(serde_json::from_str(&text)?)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = reqwest::Client::new();
    let base = &cli.url;
    let token = cli.token.as_deref();

    match cli.cmd {
        Cmd::Login { username, password } => {
            let resp = api(&client, base, None, "POST", "/api/auth/login", Some(json!({"username": username, "password": password}))).await?;
            println!("{}", resp["token"].as_str().unwrap_or(""));
        }
        Cmd::Status => {
            let state = api(&client, base, token, "GET", "/api/timer", None).await?;
            println!("{}", serde_json::to_string_pretty(&state)?);
        }
        Cmd::Start { task } => {
            let body = json!({ "task_id": task });
            let state = api(&client, base, token, "POST", "/api/timer/start", Some(body)).await?;
            println!("Started: {}", state["phase"]);
        }
        Cmd::Pause => { api(&client, base, token, "POST", "/api/timer/pause", None).await?; println!("Paused"); }
        Cmd::Resume => { api(&client, base, token, "POST", "/api/timer/resume", None).await?; println!("Resumed"); }
        Cmd::Stop => { api(&client, base, token, "POST", "/api/timer/stop", None).await?; println!("Stopped"); }
        Cmd::Skip => { api(&client, base, token, "POST", "/api/timer/skip", None).await?; println!("Skipped"); }
        Cmd::Tasks { status } => {
            let path = match status { Some(ref s) => format!("/api/tasks?status={}", s), None => "/api/tasks".to_string() };
            let tasks = api(&client, base, token, "GET", &path, None).await?;
            if let Some(arr) = tasks.as_array() {
                for t in arr {
                    println!("#{} [{}] {} (P{}) - {}", t["id"], t["status"].as_str().unwrap_or("?"), t["title"].as_str().unwrap_or("?"), t["priority"], t["user"].as_str().unwrap_or("?"));
                }
            }
        }
        Cmd::Add { title, priority, estimated, project } => {
            let task = api(&client, base, token, "POST", "/api/tasks", Some(json!({
                "title": title, "priority": priority, "estimated": estimated, "project": project
            }))).await?;
            println!("Created task #{}: {}", task["id"], task["title"]);
        }
        Cmd::Stats => {
            let stats = api(&client, base, token, "GET", "/api/stats?days=7", None).await?;
            if let Some(arr) = stats.as_array() {
                for s in arr {
                    println!("{}: {} completed, {} interrupted, {}m focus", s["date"].as_str().unwrap_or("?"), s["completed"], s["interrupted"], s["total_focus_s"].as_i64().unwrap_or(0) / 60);
                }
            }
        }
        Cmd::Sprints { status } => {
            let path = match status { Some(ref s) => format!("/api/sprints?status={}", s), None => "/api/sprints".to_string() };
            let sprints = api(&client, base, token, "GET", &path, None).await?;
            if let Some(arr) = sprints.as_array() {
                for s in arr { println!("#{} [{}] {} ({})", s["id"], s["status"].as_str().unwrap_or("?"), s["name"].as_str().unwrap_or("?"), s["project"].as_str().unwrap_or("-")); }
            }
        }
        Cmd::Labels => {
            let labels = api(&client, base, token, "GET", "/api/labels", None).await?;
            if let Some(arr) = labels.as_array() {
                for l in arr { println!("#{} {} ({})", l["id"], l["name"].as_str().unwrap_or("?"), l["color"].as_str().unwrap_or("#000")); }
            }
        }
        Cmd::Label { task_id, label_id } => {
            api(&client, base, token, "PUT", &format!("/api/tasks/{}/labels/{}", task_id, label_id), None).await?;
            println!("Label {} added to task {}", label_id, task_id);
        }
        Cmd::Deps { task_id } => {
            let deps = api(&client, base, token, "GET", &format!("/api/tasks/{}/dependencies", task_id), None).await?;
            if let Some(arr) = deps.as_array() {
                if arr.is_empty() { println!("No dependencies"); }
                else { for d in arr { println!("Depends on #{}", d); } }
            }
        }
        Cmd::Export => {
            let csv = api(&client, base, token, "GET", "/api/export/tasks?format=csv", None).await?;
            print!("{}", csv.as_str().unwrap_or(&csv.to_string()));
        }
        Cmd::Log { task_id, hours, note } => {
            api(&client, base, token, "POST", &format!("/api/tasks/{}/time", task_id), Some(json!({"hours": hours, "description": note}))).await?;
            println!("Logged {}h on task #{}", hours, task_id);
        }
        Cmd::Score => {
            let score = api(&client, base, token, "GET", "/api/analytics/focus-score", None).await?;
            println!("Focus Score: {}/100 (streak: {}d)", score["score"], score["streak_days"]);
            if let Some(c) = score["components"].as_object() {
                for (k, v) in c { println!("  {}: {}", k, v); }
            }
        }
        Cmd::Rooms => {
            let rooms = api(&client, base, token, "GET", "/api/rooms", None).await?;
            if let Some(arr) = rooms.as_array() {
                for r in arr { println!("#{} [{}] {} ({})", r["id"], r["status"].as_str().unwrap_or("?"), r["name"].as_str().unwrap_or("?"), r["estimation_unit"].as_str().unwrap_or("?")); }
            }
        }
        Cmd::JoinRoom { room_id } => {
            api(&client, base, token, "POST", &format!("/api/rooms/{}/join", room_id), None).await?;
            println!("Joined room {}", room_id);
        }
        Cmd::Vote { room_id, value } => {
            api(&client, base, token, "POST", &format!("/api/rooms/{}/vote", room_id), Some(json!({"value": value}))).await?;
            println!("Voted {} in room {}", value, room_id);
        }
        Cmd::Done { task_id } => {
            api(&client, base, token, "PUT", &format!("/api/tasks/{}", task_id), Some(json!({"status":"completed"}))).await?;
            println!("Task #{} marked as completed", task_id);
        }
        Cmd::SetStatus { task_id, status } => {
            api(&client, base, token, "PUT", &format!("/api/tasks/{}", task_id), Some(json!({"status": status}))).await?;
            println!("Task #{} → {}", task_id, status);
        }
        Cmd::Delete { task_id } => {
            api(&client, base, token, "DELETE", &format!("/api/tasks/{}", task_id), None).await?;
            println!("Task #{} deleted", task_id);
        }
        Cmd::Search { query } => {
            let results = api(&client, base, token, "GET", &format!("/api/search?q={}", query.replace(' ', "%20").replace('#', "%23").replace('&', "%26")), None).await?;
            if let Some(tasks) = results["tasks"].as_array() {
                for t in tasks { println!("[task] #{} {}", t["id"], t["title"].as_str().unwrap_or("?")); }
            }
            if let Some(comments) = results["comments"].as_array() {
                for c in comments { println!("[comment] task#{} {}", c["task_id"], c["snippet"].as_str().unwrap_or("?")); }
            }
            if let Some(sprints) = results["sprints"].as_array() {
                for s in sprints { println!("[sprint] #{} {}", s["id"], s["name"].as_str().unwrap_or("?")); }
            }
        }
        Cmd::Standup => {
            let report = api(&client, base, token, "GET", "/api/reports/standup", None).await?;
            print!("{}", report["markdown"].as_str().unwrap_or("No data"));
        }
        Cmd::Assign { task_id, username } => {
            api(&client, base, token, "POST", &format!("/api/tasks/{}/assignees", task_id), Some(json!({"username": username}))).await?;
            println!("Assigned {} to task #{}", username, task_id);
        }
        Cmd::Comment { task_id, text } => {
            api(&client, base, token, "POST", &format!("/api/tasks/{}/comments", task_id), Some(json!({"content": text}))).await?;
            println!("Comment added to task #{}", task_id);
        }
        Cmd::Show { task_id } => {
            let detail = api(&client, base, token, "GET", &format!("/api/tasks/{}", task_id), None).await?;
            let t = &detail["task"];
            println!("#{} [{}] {} (P{})", t["id"], t["status"].as_str().unwrap_or("?"), t["title"].as_str().unwrap_or("?"), t["priority"]);
            if let Some(d) = t["description"].as_str() { if !d.is_empty() { println!("  {}", d); } }
            if let Some(p) = t["project"].as_str() { println!("  Project: {}", p); }
            if let Some(d) = t["due_date"].as_str() { println!("  Due: {}", d); }
            if let Some(comments) = detail["comments"].as_array() {
                for c in comments { println!("  💬 {} ({}): {}", c["user"].as_str().unwrap_or("?"), &c["created_at"].as_str().unwrap_or("?")[..10], c["content"].as_str().unwrap_or("")); }
            }
        }
        Cmd::Checklist { task_id } => {
            let items = api(&client, base, token, "GET", &format!("/api/tasks/{}/checklist", task_id), None).await?;
            if let Some(arr) = items.as_array() {
                if arr.is_empty() { println!("No checklist items"); }
                for item in arr { println!("[{}] {}", if item["checked"].as_bool().unwrap_or(false) { "x" } else { " " }, item["title"].as_str().unwrap_or("?")); }
            }
        }
        Cmd::Check { task_id, title } => {
            api(&client, base, token, "POST", &format!("/api/tasks/{}/checklist", task_id), Some(json!({"title": title}))).await?;
            println!("Checklist item added to task #{}", task_id);
        }
    }
    Ok(())
}
