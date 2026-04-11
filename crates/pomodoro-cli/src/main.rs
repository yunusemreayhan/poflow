use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::{json, Value};

#[derive(Parser)]
#[command(name = "pomo", about = "Pomodoro timer CLI")]
struct Cli {
    /// Server URL
    #[arg(long, default_value = "http://127.0.0.1:9090", env = "POMODORO_URL")]
    url: String,
    /// Auth token
    #[arg(long, env = "POMODORO_TOKEN")]
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
    }
    Ok(())
}
