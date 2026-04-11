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
    let p = std::path::Path::new(&path);
    // Only allow writing to user's download/document/desktop directories (not all of data dir)
    let allowed = dirs::download_dir()
        .into_iter()
        .chain(dirs::document_dir())
        .chain(dirs::desktop_dir())
        .any(|dir| p.starts_with(&dir));
    if !allowed {
        return Err("Write denied: path must be in Downloads, Documents, or Desktop directory".to_string());
    }
    // Prevent path traversal
    if path.contains("..") {
        return Err("Write denied: path traversal not allowed".to_string());
    }
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

fn auth_key() -> Vec<u8> {
    // Machine-specific key derived from hostname + username
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    whoami::hostname().hash(&mut h);
    whoami::username().hash(&mut h);
    "pomodoro-gui-auth".hash(&mut h);
    h.finish().to_le_bytes().to_vec()
}

fn xor_bytes(data: &[u8], key: &[u8]) -> Vec<u8> {
    data.iter().enumerate().map(|(i, b)| b ^ key[i % key.len()]).collect()
}

fn auth_dir() -> std::path::PathBuf {
    dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join("pomodoro-gui")
}

#[tauri::command]
async fn save_auth(_state: tauri::State<'_, Arc<AppState>>, data: String) -> Result<(), String> {
    let dir = auth_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let encrypted = xor_bytes(data.as_bytes(), &auth_key());
    std::fs::write(dir.join(".auth"), encrypted).map_err(|e| e.to_string())
}

#[tauri::command]
async fn load_auth(_state: tauri::State<'_, Arc<AppState>>) -> Result<String, String> {
    let raw = std::fs::read(auth_dir().join(".auth")).map_err(|e| e.to_string())?;
    let decrypted = xor_bytes(&raw, &auth_key());
    String::from_utf8(decrypted).map_err(|_| "Failed to decrypt auth data".to_string())
}

#[tauri::command]
async fn clear_auth(_state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    let _ = std::fs::remove_file(auth_dir().join(".auth"));
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
