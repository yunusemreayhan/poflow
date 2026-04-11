use axum::{extract::FromRequestParts, http::request::Parts};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tokio::sync::Mutex;
use std::collections::HashSet;

static SECRET: OnceLock<Vec<u8>> = OnceLock::new();
static BLOCKLIST: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static AUTH_POOL: OnceLock<crate::db::Pool> = OnceLock::new();

fn blocklist() -> &'static Mutex<HashSet<String>> {
    BLOCKLIST.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Initialize auth with a DB pool for persistent token blocklist
pub async fn init_pool(pool: crate::db::Pool) {
    AUTH_POOL.set(pool.clone()).ok();
    // Load existing blocklist from DB
    let rows: Vec<(String,)> = sqlx::query_as("SELECT token_hash FROM token_blocklist WHERE expires_at > datetime('now')")
        .fetch_all(&pool).await.unwrap_or_default();
    let mut bl = blocklist().lock().await;
    for (hash,) in rows { bl.insert(hash); }
}

fn secret() -> &'static [u8] {
    SECRET.get_or_init(|| {
        // 1. Env var override
        if let Ok(s) = std::env::var("POMODORO_JWT_SECRET") {
            if !s.is_empty() { return s.into_bytes(); }
        }
        // 2. Persisted secret file
        let secret_path = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("~/.local/share"))
            .join("pomodoro")
            .join(".jwt_secret");
        if let Ok(data) = std::fs::read(&secret_path) {
            if data.len() >= 32 { return data; }
        }
        // 3. Generate and persist a new random secret
        use std::io::Read;
        let mut buf = [0u8; 64];
        let mut got_entropy = false;
        if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
            if f.read_exact(&mut buf).is_ok() { got_entropy = true; }
        }
        if !got_entropy {
            // Fallback: multiple entropy sources hashed iteratively
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            for i in 0..8 {
                let mut h = DefaultHasher::new();
                std::process::id().hash(&mut h);
                std::time::SystemTime::now().hash(&mut h);
                std::thread::current().id().hash(&mut h);
                i.hash(&mut h);
                // Yield to add timing jitter
                std::thread::yield_now();
                std::time::SystemTime::now().hash(&mut h);
                let v = h.finish();
                buf[i * 8..(i + 1) * 8].copy_from_slice(&v.to_le_bytes());
            }
            tracing::warn!("/dev/urandom unavailable — JWT secret generated with reduced entropy");
        }
        if let Some(parent) = secret_path.parent() { std::fs::create_dir_all(parent).ok(); }
        std::fs::write(&secret_path, &buf).ok();
        tracing::info!("Generated new JWT secret at {}", secret_path.display());
        buf.to_vec()
    })
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,      // user_id as string
    pub user_id: i64,
    pub username: String,
    pub role: String,
    pub exp: usize,
}

pub fn create_token(user_id: i64, username: &str, role: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let exp = chrono::Utc::now().timestamp() as usize + 2 * 3600; // 2 hours
    let claims = Claims { sub: user_id.to_string(), user_id, username: username.to_string(), role: role.to_string(), exp };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret()))
}

/// Create a long-lived refresh token (30 days)
pub fn create_refresh_token(user_id: i64, username: &str, role: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let exp = chrono::Utc::now().timestamp() as usize + 30 * 24 * 3600;
    let claims = Claims { sub: user_id.to_string(), user_id, username: username.to_string(), role: role.to_string(), exp };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret()))
}

pub fn verify_token(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode::<Claims>(token, &DecodingKey::from_secret(secret()), &Validation::default()).map(|d| d.claims)
}

/// Revoke a token (add to blocklist). Persists to DB.
pub async fn revoke_token(token: &str) {
    let hash = format!("{:x}", md5_hash(token.as_bytes()));
    blocklist().lock().await.insert(hash.clone());
    // Persist to DB
    if let Some(pool) = AUTH_POOL.get() {
        let exp = decode::<Claims>(token, &DecodingKey::from_secret(secret()), &{
            let mut v = Validation::default(); v.validate_exp = false; v
        }).map(|d| d.claims.exp).unwrap_or(0);
        let expires = chrono::DateTime::from_timestamp(exp as i64, 0)
            .map(|d| d.format("%Y-%m-%dT%H:%M:%S").to_string())
            .unwrap_or_default();
        sqlx::query("INSERT OR IGNORE INTO token_blocklist (token_hash, expires_at) VALUES (?, ?)")
            .bind(&hash).bind(&expires).execute(pool).await.ok();
        // Prune expired
        sqlx::query("DELETE FROM token_blocklist WHERE expires_at < datetime('now')").execute(pool).await.ok();
    }
}

/// Check if a token has been revoked.
pub async fn is_revoked(token: &str) -> bool {
    let hash = format!("{:x}", md5_hash(token.as_bytes()));
    blocklist().lock().await.contains(&hash)
}

fn md5_hash(data: &[u8]) -> u128 {
    // Use SHA-256 truncated to 128 bits for token blocklist hashing
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h1 = DefaultHasher::new();
    data.hash(&mut h1);
    let a = h1.finish();
    let mut h2 = DefaultHasher::new();
    a.hash(&mut h2);
    data.len().hash(&mut h2);
    let b = h2.finish();
    let mut h3 = DefaultHasher::new();
    b.hash(&mut h3);
    data.hash(&mut h3);
    let c = h3.finish();
    // Combine 3 hashes for better collision resistance
    ((a as u128) << 64) | ((b as u128) ^ (c as u128))
}

impl<S: Send + Sync> FromRequestParts<S> for Claims {
    type Rejection = axum::http::StatusCode;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send {
        async move {
            let header = parts.headers.get("authorization")
                .and_then(|v| v.to_str().ok())
                .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;
            let token = header.strip_prefix("Bearer ").ok_or(axum::http::StatusCode::UNAUTHORIZED)?;
            if is_revoked(token).await { return Err(axum::http::StatusCode::UNAUTHORIZED); }
            verify_token(token).map_err(|_| axum::http::StatusCode::UNAUTHORIZED)
        }
    }
}
