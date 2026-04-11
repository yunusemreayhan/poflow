use crate::auth::{self, Claims};
use crate::db;
use crate::engine::{Engine, TimerPhase, ChangeEvent};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// Inline rate limiter (no external module dependency)
struct RateLimiter {
    attempts: std::sync::Mutex<std::collections::HashMap<String, Vec<std::time::Instant>>>,
    max_requests: usize,
    window_secs: u64,
}
impl RateLimiter {
    fn new(max_requests: usize, window_secs: u64) -> Self {
        Self { attempts: std::sync::Mutex::new(std::collections::HashMap::new()), max_requests, window_secs }
    }
}

static AUTH_LIMITER: std::sync::OnceLock<RateLimiter> = std::sync::OnceLock::new();
fn auth_limiter() -> &'static RateLimiter {
    AUTH_LIMITER.get_or_init(|| RateLimiter::new(10, 60))
}

pub(crate) fn check_auth_rate_limit(headers: &axum::http::HeaderMap) -> Result<(), ApiError> {
    let ip = headers.get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        // Normalize IPv4-mapped IPv6 (::ffff:127.0.0.1 → 127.0.0.1)
        .map(|s| s.strip_prefix("::ffff:").unwrap_or(&s).to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let limiter = auth_limiter();
    let now = std::time::Instant::now();
    let mut map = limiter.attempts.lock().unwrap();
    let entries = map.entry(ip).or_default();
    entries.retain(|t| now.duration_since(*t).as_secs() < limiter.window_secs);
    if entries.len() >= limiter.max_requests {
        return Err(err(StatusCode::TOO_MANY_REQUESTS, "Too many attempts. Try again later."));
    }
    entries.push(now);
    Ok(())
}

pub type AppState = Arc<Engine>;

// Types extracted to types.rs
mod types;
pub use types::*;

// --- Route modules ---

mod auth_routes;
pub use auth_routes::*;
mod timer;
pub use timer::*;
mod tasks;
pub use tasks::*;
mod comments;
pub use comments::*;
mod burns_task;
pub use burns_task::*;
mod assignees;
pub use assignees::*;
mod history;
pub use history::*;
mod config;
pub use config::*;
mod profile;
pub use profile::*;
mod admin;
pub use admin::*;
mod misc;
pub use misc::*;
mod rooms;
pub use rooms::*;
mod sprints;
pub use sprints::*;
mod epics;
pub use epics::*;
mod teams;
pub use teams::*;
mod burns;
pub use burns::*;
mod export;
pub use export::*;
mod audit;
pub use audit::*;
mod labels;
pub use labels::*;
mod recurrence;
pub use recurrence::*;
mod dependencies;
pub use dependencies::*;
mod webhooks;
pub use webhooks::*;
mod templates;
pub use templates::*;
mod attachments;
pub use attachments::*;
