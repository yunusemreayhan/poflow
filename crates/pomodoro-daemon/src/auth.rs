use axum::{extract::FromRequestParts, http::request::Parts};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tokio::sync::RwLock;
use std::collections::HashSet;

static SECRET: OnceLock<Vec<u8>> = OnceLock::new();
static BLOCKLIST: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();
static AUTH_POOL: OnceLock<crate::db::Pool> = OnceLock::new();

fn blocklist() -> &'static RwLock<HashSet<String>> {
    BLOCKLIST.get_or_init(|| RwLock::new(HashSet::new()))
}

/// Initialize auth with a DB pool for persistent token blocklist
pub async fn init_pool(pool: crate::db::Pool) {
    AUTH_POOL.set(pool.clone()).ok();
    let rows: Vec<(String,)> = sqlx::query_as("SELECT token_hash FROM token_blocklist WHERE expires_at > datetime('now')")
        .fetch_all(&pool).await.unwrap_or_default();
    let mut bl = blocklist().write().await;
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
            panic!("FATAL: /dev/urandom unavailable and no JWT secret configured. Set POMODORO_JWT_SECRET env var or ensure /dev/urandom is accessible.");
        }
        if let Some(parent) = secret_path.parent() { std::fs::create_dir_all(parent).ok(); }
        std::fs::write(&secret_path, &buf).ok();
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&secret_path, std::fs::Permissions::from_mode(0o600)).ok();
        }
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
    pub iat: usize,
    #[serde(default = "default_token_type")]
    pub typ: String,      // "access" or "refresh"
}

fn default_token_type() -> String { "access".to_string() }

pub fn create_token(user_id: i64, username: &str, role: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let now = chrono::Utc::now().timestamp() as usize;
    let access_exp: usize = std::env::var("ACCESS_TOKEN_EXPIRY_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(2 * 3600);
    let claims = Claims { sub: user_id.to_string(), user_id, username: username.to_string(), role: role.to_string(), exp: now + access_exp, iat: now, typ: "access".to_string() };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret()))
}

/// Create a long-lived refresh token (30 days)
pub fn create_refresh_token(user_id: i64, username: &str, role: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let now = chrono::Utc::now().timestamp() as usize;
    let refresh_exp: usize = std::env::var("REFRESH_TOKEN_EXPIRY_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(30 * 24 * 3600);
    let claims = Claims { sub: user_id.to_string(), user_id, username: username.to_string(), role: role.to_string(), exp: now + refresh_exp, iat: now, typ: "refresh".to_string() };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret()))
}

pub fn verify_token(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode::<Claims>(token, &DecodingKey::from_secret(secret()), &Validation::default()).map(|d| d.claims)
}

/// Revoke a token (add to blocklist). Persists to DB.
pub async fn revoke_token(token: &str) {
    let hash = format!("{:032x}", token_hash(token.as_bytes()));
    blocklist().write().await.insert(hash.clone());
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
        // Prune expired from DB and sync in-memory blocklist
        sqlx::query("DELETE FROM token_blocklist WHERE expires_at < datetime('now')").execute(pool).await.ok();
        // S3: Trim in-memory set to match DB (prevents unbounded growth)
        let mut bl = blocklist().write().await;
        if bl.len() > 1000 {
            let valid: Vec<(String,)> = sqlx::query_as("SELECT token_hash FROM token_blocklist WHERE expires_at > datetime('now')")
                .fetch_all(pool).await.unwrap_or_default();
            let valid_set: HashSet<String> = valid.into_iter().map(|(h,)| h).collect();
            *bl = valid_set;
        }
    }
}

/// Check if a token has been revoked.
pub async fn is_revoked(token: &str) -> bool {
    let hash = format!("{:032x}", token_hash(token.as_bytes()));
    blocklist().read().await.contains(&hash)
}

fn token_hash(data: &[u8]) -> u128 {
    use sha2::{Sha256, Digest};
    let hash = Sha256::digest(data);
    u128::from_be_bytes(hash[..16].try_into().unwrap())
}

impl<S: Send + Sync> FromRequestParts<S> for Claims {
    type Rejection = axum::http::StatusCode;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send {
        async move {
            // CSRF: require x-requested-with header on state-changing requests
            let method = &parts.method;
            if method != axum::http::Method::GET && method != axum::http::Method::HEAD && method != axum::http::Method::OPTIONS {
                if parts.headers.get("x-requested-with").is_none() {
                    return Err(axum::http::StatusCode::FORBIDDEN);
                }
            }
            let header = parts.headers.get("authorization")
                .and_then(|v| v.to_str().ok())
                .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;
            let token = header.strip_prefix("Bearer ").ok_or(axum::http::StatusCode::UNAUTHORIZED)?;
            if is_revoked(token).await { return Err(axum::http::StatusCode::UNAUTHORIZED); }
            let claims = verify_token(token).map_err(|_| axum::http::StatusCode::UNAUTHORIZED)?;
            // Reject refresh tokens used as access tokens
            if claims.typ == "refresh" { return Err(axum::http::StatusCode::UNAUTHORIZED); }
            Ok(claims)
        }
    }
}
