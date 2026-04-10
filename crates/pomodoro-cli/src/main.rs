use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

#[derive(Parser)]
#[command(name = "pomo", about = "Pomodoro timer CLI")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Show current timer state
    Status,
    /// Start a work session
    Start {
        #[arg(short, long)]
        task: Option<i64>,
    },
    /// Pause the timer
    Pause,
    /// Resume the timer
    Resume,
    /// Stop the timer
    Stop,
    /// Skip current phase
    Skip,
    /// List tasks
    Tasks {
        #[arg(short, long)]
        status: Option<String>,
    },
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
}

fn socket_path() -> std::path::PathBuf {
    let uid = unsafe { libc::getuid() };
    let run_dir = format!("/run/user/{}", uid);
    if std::path::Path::new(&run_dir).exists() {
        std::path::PathBuf::from(run_dir).join("pomodoro.sock")
    } else {
        dirs::cache_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp")).join("pomodoro.sock")
    }
}

async fn call(method: &str, params: Value) -> Result<Value> {
    let stream = UnixStream::connect(socket_path()).await?;
    let (reader, mut writer) = stream.into_split();
    let req = json!({ "method": method, "params": params });
    let mut line = serde_json::to_string(&req)?;
    line.push('\n');
    writer.write_all(line.as_bytes()).await?;
    writer.shutdown().await?;
    let mut lines = BufReader::new(reader).lines();
    if let Some(resp_line) = lines.next_line().await? {
        let resp: Value = serde_json::from_str(&resp_line)?;
        if resp["success"].as_bool().unwrap_or(false) {
            Ok(resp["data"].clone())
        } else {
            Err(anyhow::anyhow!("{}", resp["error"].as_str().unwrap_or("unknown error")))
        }
    } else {
        Err(anyhow::anyhow!("No response from daemon"))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Status => {
            let state = call("get_state", json!({})).await?;
            println!("{}", serde_json::to_string_pretty(&state)?);
        }
        Cmd::Start { task } => {
            let state = call("start", json!({ "task_id": task })).await?;
            println!("Started: {:?}", state["phase"]);
        }
        Cmd::Pause => { call("pause", json!({})).await?; println!("Paused"); }
        Cmd::Resume => { call("resume", json!({})).await?; println!("Resumed"); }
        Cmd::Stop => { call("stop", json!({})).await?; println!("Stopped"); }
        Cmd::Skip => { call("skip", json!({})).await?; println!("Skipped"); }
        Cmd::Tasks { status } => {
            let tasks = call("list_tasks", json!({ "status": status })).await?;
            println!("{}", serde_json::to_string_pretty(&tasks)?);
        }
        Cmd::Add { title, priority, estimated, project } => {
            let task = call("create_task", json!({
                "title": title, "priority": priority,
                "estimated": estimated, "project": project
            })).await?;
            println!("Created task #{}: {}", task["id"], task["title"]);
        }
        Cmd::Stats => {
            let stats = call("get_stats", json!({ "days": 7 })).await?;
            println!("{}", serde_json::to_string_pretty(&stats)?);
        }
    }
    Ok(())
}
