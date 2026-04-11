use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
struct ConnectionConfig {
    base_url: String,
    token: Option<String>,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:9090".to_string(),
            token: None,
        }
    }
}

struct AppState {
    config: Mutex<ConnectionConfig>,
    client: reqwest::Client,
}

#[tauri::command]
async fn api_call(state: tauri::State<'_, Arc<AppState>>, method: String, path: String, body: Option<Value>) -> Result<Value, String> {
    let config = state.config.lock().await.clone();
    let url = format!("{}{}", config.base_url, path);
    let client = &state.client;

    let mut req = match method.as_str() {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        _ => return Err(format!("Unknown method: {}", method)),
    };

    if let Some(token) = &config.token {
        req = req.header("Authorization", format!("Bearer {}", token));
    }
    req = req.header("X-Requested-With", "PomodoroGUI");
    if let Some(b) = body {
        req = req.json(&b);
    }

    let resp = req.send().await.map_err(|e| format!("Request failed: {}", e))?;
    let status = resp.status().as_u16();
    let text = resp.text().await.map_err(|e| e.to_string())?;

    if status >= 400 {
        return Err(text);
    }
    if text.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_token(state: tauri::State<'_, Arc<AppState>>, token: String) -> Result<(), String> {
    state.config.lock().await.token = if token.is_empty() { None } else { Some(token) };
    Ok(())
}

#[tauri::command]
async fn get_connection(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, String> {
    let c = state.config.lock().await;
    Ok(serde_json::json!({
        "base_url": c.base_url,
        "has_token": c.token.is_some(),
    }))
}

#[tauri::command]
async fn set_connection(state: tauri::State<'_, Arc<AppState>>, base_url: String) -> Result<(), String> {
    state.config.lock().await.base_url = base_url;
    Ok(())
}

#[tauri::command]
async fn write_file(path: String, content: String) -> Result<(), String> {
    // Only allow writing to user's data/download directories
    let p = std::path::Path::new(&path);
    let allowed = dirs::download_dir()
        .into_iter()
        .chain(dirs::document_dir())
        .chain(dirs::data_dir())
        .chain(dirs::desktop_dir())
        .any(|dir| p.starts_with(&dir));
    if !allowed {
        return Err("Write denied: path must be in Downloads, Documents, Desktop, or data directory".to_string());
    }
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_auth(state: tauri::State<'_, Arc<AppState>>, data: String) -> Result<(), String> {
    let dir = dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join("pomodoro-gui");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(".auth"), data).map_err(|e| e.to_string())
}

#[tauri::command]
async fn load_auth(state: tauri::State<'_, Arc<AppState>>) -> Result<String, String> {
    let path = dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join("pomodoro-gui").join(".auth");
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn clear_auth(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    let path = dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join("pomodoro-gui").join(".auth");
    let _ = std::fs::remove_file(&path);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = Arc::new(AppState {
        config: Mutex::new(ConnectionConfig::default()),
        client: reqwest::Client::new(),
    });

    tauri::Builder::default()
        .manage(app_state)
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![api_call, set_token, get_connection, set_connection, write_file, save_auth, load_auth, clear_auth])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
