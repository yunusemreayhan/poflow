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
        // Try to extract a clean error message, don't leak raw server internals
        let msg = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str().map(String::from)))
            .unwrap_or_else(|| format!("Request failed ({})", status));
        return Err(msg);
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
    // Warn if not HTTPS and not localhost
    if !base_url.starts_with("https://") && !base_url.contains("127.0.0.1") && !base_url.contains("localhost") {
        eprintln!("WARNING: Connection to {} is not using HTTPS — credentials may be transmitted in plaintext", base_url);
    }
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
    // Prevent path traversal — canonicalize to resolve symlinks
    let canonical = p.parent()
        .and_then(|parent| std::fs::canonicalize(parent).ok())
        .map(|parent| parent.join(p.file_name().unwrap_or_default()));
    let check_path = canonical.as_deref().unwrap_or(p);
    let allowed = dirs::download_dir()
        .into_iter()
        .chain(dirs::document_dir())
        .chain(dirs::desktop_dir())
        .any(|dir| check_path.starts_with(&dir));
    if !allowed {
        return Err("Write denied: path must be in Downloads, Documents, or Desktop directory".to_string());
    }
    if path.contains("..") {
        return Err("Write denied: path traversal not allowed".to_string());
    }
    // Block executable file extensions
    let blocked_ext = [".desktop", ".sh", ".bash", ".bat", ".cmd", ".exe", ".ps1", ".app", ".run",
        ".py", ".pl", ".rb", ".jar", ".deb", ".rpm", ".appimage", ".msi", ".com", ".csh", ".ksh", ".zsh"];
    if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
        let dot_ext = format!(".{}", ext.to_lowercase());
        if blocked_ext.contains(&dot_ext.as_str()) {
            return Err(format!("Write denied: .{} files not allowed", ext));
        }
    }
    tokio::fs::write(&path, content).await.map_err(|e| e.to_string())
}

fn auth_key() -> Vec<u8> {
    use sha2::{Sha256, Digest};
    // Load or create a random salt (generated once per installation)
    let dir = auth_dir();
    let salt_path = dir.join(".auth_salt");
    let salt = if let Ok(s) = std::fs::read(&salt_path) {
        if s.len() == 32 { s } else {
            let s = generate_salt();
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(&salt_path, &s);
            s
        }
    } else {
        let s = generate_salt();
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(&salt_path, &s);
        s
    };
    let mut h = Sha256::new();
    h.update(&salt);
    h.update(b":");
    h.update(whoami::hostname().as_bytes());
    h.update(b":");
    h.update(whoami::username().as_bytes());
    h.update(b":pomodoro-gui-auth-v2");
    h.finalize().to_vec()
}

fn generate_salt() -> Vec<u8> {
    use std::io::Read;
    let mut buf = vec![0u8; 32];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut buf);
    }
    buf
}

fn encrypt_auth(data: &[u8], key: &[u8]) -> Result<Vec<u8>, String> {
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    // Generate random 12-byte nonce
    let mut nonce_bytes = [0u8; 12];
    std::io::Read::read_exact(&mut std::fs::File::open("/dev/urandom").map_err(|e| e.to_string())?, &mut nonce_bytes).map_err(|e| e.to_string())?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, data).map_err(|e| e.to_string())?;
    // Prepend nonce to ciphertext
    let mut out = nonce_bytes.to_vec();
    out.extend(ciphertext);
    Ok(out)
}

fn decrypt_auth(data: &[u8], key: &[u8]) -> Result<Vec<u8>, String> {
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
    if data.len() < 12 { return Err("Data too short".into()); }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let nonce = Nonce::from_slice(&data[..12]);
    cipher.decrypt(nonce, &data[12..]).map_err(|e| e.to_string())
}

fn auth_dir() -> std::path::PathBuf {
    dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join("pomodoro-gui")
}

#[tauri::command]
async fn save_auth(_state: tauri::State<'_, Arc<AppState>>, data: String) -> Result<(), String> {
    let dir = auth_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let encrypted = encrypt_auth(data.as_bytes(), &auth_key())?;
    std::fs::write(dir.join(".auth"), encrypted).map_err(|e| e.to_string())
}

#[tauri::command]
async fn load_auth(_state: tauri::State<'_, Arc<AppState>>) -> Result<String, String> {
    let raw = std::fs::read(auth_dir().join(".auth")).map_err(|e| e.to_string())?;
    let decrypted = decrypt_auth(&raw, &auth_key())?;
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
