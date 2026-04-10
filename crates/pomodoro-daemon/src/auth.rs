use axum::{extract::FromRequestParts, http::request::Parts};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

const DEFAULT_SECRET: &[u8] = b"pomodoro-jwt-secret-change-in-production";

fn secret() -> Vec<u8> {
    std::env::var("POMODORO_JWT_SECRET").map(|s| s.into_bytes()).unwrap_or_else(|_| DEFAULT_SECRET.to_vec())
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
    let exp = chrono::Utc::now().timestamp() as usize + 7 * 24 * 3600;
    let claims = Claims { sub: user_id.to_string(), user_id, username: username.to_string(), role: role.to_string(), exp };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(&secret()))
}

pub fn verify_token(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode::<Claims>(token, &DecodingKey::from_secret(&secret()), &Validation::default()).map(|d| d.claims)
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
            verify_token(token).map_err(|_| axum::http::StatusCode::UNAUTHORIZED)
        }
    }
}
