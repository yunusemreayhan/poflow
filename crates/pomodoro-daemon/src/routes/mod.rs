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
pub(crate) struct RateLimiter {
    pub(crate) attempts: std::sync::Mutex<std::collections::HashMap<String, Vec<std::time::Instant>>>,
    pub(crate) max_requests: usize,
    pub(crate) window_secs: u64,
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

// General API rate limiter: 200 requests per 60 seconds per IP (for mutation endpoints)
static API_LIMITER: std::sync::OnceLock<RateLimiter> = std::sync::OnceLock::new();
pub(crate) fn api_limiter() -> &'static RateLimiter {
    API_LIMITER.get_or_init(|| RateLimiter::new(200, 60))
}

pub(crate) fn extract_ip(headers: &axum::http::HeaderMap) -> String {
    headers.get("x-real-ip")
        .or_else(|| headers.get("x-forwarded-for"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        .map(|s| s.strip_prefix("::ffff:").unwrap_or(&s).to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

pub(crate) fn check_auth_rate_limit(headers: &axum::http::HeaderMap) -> Result<(), ApiError> {
    let ip = extract_ip(headers);
    let limiter = auth_limiter();
    check_rate_limit(limiter, &ip)
}

fn check_rate_limit(limiter: &RateLimiter, ip: &str) -> Result<(), ApiError> {
    let now = std::time::Instant::now();
    let mut map = limiter.attempts.lock().unwrap();
    // Periodic full cleanup: prune stale IPs every 500 insertions
    if map.len() > 500 {
        map.retain(|_, entries| {
            entries.retain(|t| now.duration_since(*t).as_secs() < limiter.window_secs);
            !entries.is_empty()
        });
    }
    let entries = map.entry(ip.to_string()).or_default();
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
mod watchers;
pub use watchers::{watch_task, unwatch_task, get_task_watchers, get_watched_tasks};
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
mod notifications;
pub use notifications::*;
