use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::Emitter;
use tauri::Manager;
use base64::Engine as _;

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
    // V35-4: Validate path to prevent SSRF — must start with /api
    if !path.starts_with("/api") {
        return Err("Invalid API path".to_string());
    }
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
    // V35-7: Limit content size to 50MB to prevent disk exhaustion
    if content.len() > 50 * 1024 * 1024 {
        return Err("Write denied: content exceeds 50MB limit".to_string());
    }
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

#[tauri::command]
async fn binary_download(state: tauri::State<'_, Arc<AppState>>, path: String) -> Result<String, String> {
    if !path.starts_with("/api") {
        return Err("Invalid API path".to_string());
    }
    let config = state.config.lock().await.clone();
    let url = format!("{}{}", config.base_url, path);
    let mut req = state.client.get(&url).header("X-Requested-With", "PomodoroGUI");
    if let Some(token) = &config.token {
        req = req.header("Authorization", format!("Bearer {}", token));
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    if resp.status().as_u16() >= 400 {
        return Err(format!("Download failed ({})", resp.status().as_u16()));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
}

#[tauri::command]
async fn binary_upload(state: tauri::State<'_, Arc<AppState>>, path: String, data: String, filename: String, mime: String) -> Result<Value, String> {
    if !path.starts_with("/api") {
        return Err("Invalid API path".to_string());
    }
    let config = state.config.lock().await.clone();
    let url = format!("{}{}", config.base_url, path);
    let bytes = base64::engine::general_purpose::STANDARD.decode(&data).map_err(|e| e.to_string())?;
    let mut req = state.client.post(&url)
        .header("X-Requested-With", "PomodoroGUI")
        .header("Content-Type", if mime.is_empty() { "application/octet-stream" } else { &mime })
        .header("X-Filename", &filename)
        .body(bytes);
    if let Some(token) = &config.token {
        req = req.header("Authorization", format!("Bearer {}", token));
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if status >= 400 {
        return Err(serde_json::from_str::<Value>(&text).ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str().map(String::from)))
            .unwrap_or_else(|| format!("Upload failed ({})", status)));
    }
    if text.is_empty() { return Ok(Value::Null); }
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

fn auth_key() -> Vec<u8> {
    use sha2::{Sha256, Digest};
    // Load or create a random salt (generated once per installation)
    let dir = auth_dir();
    let salt_path = dir.join(".auth_salt");
    let salt = if let Ok(s) = std::fs::read(&salt_path) {
        if s.len() == 32 { s } else {
            let s = generate_salt().expect("Failed to generate cryptographic salt");
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(&salt_path, &s);
            s
        }
    } else {
        let s = generate_salt().expect("Failed to generate cryptographic salt");
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(&salt_path, &s);
        s
    };
    let mut h = Sha256::new();
    h.update(&salt);
    h.update(b":");
    h.update(whoami::fallible::hostname().unwrap_or_else(|_| "unknown".to_string()).as_bytes());
    h.update(b":");
    h.update(whoami::username().as_bytes());
    h.update(b":pomodoro-gui-auth-v2");
    h.finalize().to_vec()
}

fn generate_salt() -> Result<Vec<u8>, String> {
    let mut buf = vec![0u8; 32];
    getrandom::fill(&mut buf).map_err(|e| format!("Failed to generate salt: {}", e))?;
    Ok(buf)
}

fn encrypt_auth(data: &[u8], key: &[u8]) -> Result<Vec<u8>, String> {
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let mut nonce_bytes = [0u8; 12];
    getrandom::fill(&mut nonce_bytes).map_err(|e| format!("Failed to generate nonce: {}", e))?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, data).map_err(|e| e.to_string())?;
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

#[tauri::command]
async fn indicator_status() -> Result<bool, String> {
    let output = std::process::Command::new("pgrep").args(["-f", "panel-indicator.py"])
        .output().map_err(|e| e.to_string())?;
    Ok(output.status.success())
}

#[tauri::command]
async fn indicator_toggle(enable: bool) -> Result<bool, String> {
    if enable {
        let home = dirs::home_dir().unwrap_or_default();
        let candidates = [
            std::path::PathBuf::from("/usr/share/pomodoro/panel-indicator.py"),
            home.join("repos/pomodoroLinux/tools/panel-indicator.py"),
            home.join(".local/share/pomodoro/panel-indicator.py"),
        ];
        let script = candidates.iter().find(|p| p.exists())
            .ok_or("panel-indicator.py not found")?;
        std::process::Command::new("python3").arg(script)
            .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
            .spawn().map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        std::process::Command::new("pkill").args(["-f", "panel-indicator.py"])
            .output().map_err(|e| e.to_string())?;
        Ok(false)
    }
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
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            // U6: Register global shortcut Ctrl+Shift+P to toggle timer
            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            let handle = app.handle().clone();
            app.global_shortcut().on_shortcut("CmdOrCtrl+Shift+P", move |_app, _shortcut, event| {
                if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                    let _ = handle.emit("global-timer-toggle", ());
                }
            })?;

            // U7: Native tray icon
            use tauri::tray::TrayIconBuilder;
            use tauri::menu::{MenuBuilder, MenuItemBuilder};
            let handle2 = app.handle().clone();
            let handle3 = app.handle().clone();
            let toggle_item = MenuItemBuilder::with_id("toggle", "Start/Pause").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&toggle_item, &quit_item]).build()?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().cloned().unwrap_or_else(|| tauri::image::Image::new(&[], 0, 0)))
                .tooltip("Pomodoro")
                .menu(&menu)
                .on_menu_event(move |_app, event: tauri::menu::MenuEvent| {
                    match event.id().as_ref() {
                        "toggle" => { let _ = handle2.emit("global-timer-toggle", ()); }
                        "quit" => { std::process::exit(0); }
                        _ => {}
                    }
                })
                .on_tray_icon_event(move |_tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { button: tauri::tray::MouseButton::Left, .. } = event {
                        // Show/focus the main window on left click
                        if let Some(w) = handle3.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![api_call, set_token, get_connection, set_connection, write_file, save_auth, load_auth, clear_auth, indicator_status, indicator_toggle, binary_download, binary_upload])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
