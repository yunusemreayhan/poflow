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
    state.config.lock().await.token = Some(token);
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
    std::fs::write(&path, content).map_err(|e| e.to_string())
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
        .invoke_handler(tauri::generate_handler![api_call, set_token, get_connection, set_connection, write_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
