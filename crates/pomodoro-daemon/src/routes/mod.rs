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
// Sliding window counter rate limiter with non-blocking mutex
#[doc(hidden)] // Public for integration tests only
pub struct RateLimiter {
    pub(crate) buckets: parking_lot::Mutex<std::collections::HashMap<String, (u32, u32, u64)>>, // (prev_count, curr_count, curr_window_start)
    pub(crate) max_requests: u32,
    pub(crate) window_secs: u64,
}
impl RateLimiter {
    fn new(max_requests: u32, window_secs: u64) -> Self {
        Self { buckets: parking_lot::Mutex::new(std::collections::HashMap::new()), max_requests, window_secs }
    }
    pub(crate) fn check_and_record(&self, key: &str) -> bool {
        let now = std::time::SystemTime::now().duration_since(std::time::SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs();
        let window = self.window_secs;
        let curr_window = now / window;
        let mut map = self.buckets.lock();
        // Periodic cleanup: if map is large, evict stale entries
        if map.len() > 500 {
            map.retain(|_, (_, _, ws)| curr_window.saturating_sub(*ws) <= 1);
        }
        let entry = map.entry(key.to_string()).or_insert((0, 0, curr_window));
        // Rotate windows
        if entry.2 < curr_window {
            if curr_window - entry.2 == 1 {
                entry.0 = entry.1; // prev = old current
            } else {
                entry.0 = 0; // too old, reset
            }
            entry.1 = 0;
            entry.2 = curr_window;
        }
        // Sliding window estimate: prev * (1 - elapsed_fraction) + curr
        let elapsed_frac = (now % window) as f64 / window as f64;
        let estimate = (entry.0 as f64 * (1.0 - elapsed_frac)) + entry.1 as f64;
        if estimate >= self.max_requests as f64 {
            return false;
        }
        entry.1 += 1;
        true
    }
}

impl RateLimiter {
    /// Clear all rate limit state (for testing)
    pub fn reset(&self) {
        self.buckets.lock().clear();
    }
}

static AUTH_LIMITER: std::sync::OnceLock<RateLimiter> = std::sync::OnceLock::new();
pub fn auth_limiter() -> &'static RateLimiter {
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
    if std::env::var("POMODORO_NO_RATE_LIMIT").is_ok() {
        static WARN: std::sync::Once = std::sync::Once::new();
        WARN.call_once(|| tracing::warn!("POMODORO_NO_RATE_LIMIT is set — ALL rate limiting is DISABLED. Do not use in production."));
        return Ok(());
    }
    let ip = extract_ip(headers);
    let limiter = auth_limiter();
    check_rate_limit(limiter, &ip)
}

fn check_rate_limit(limiter: &RateLimiter, ip: &str) -> Result<(), ApiError> {
    if limiter.check_and_record(ip) {
        Ok(())
    } else {
        Err(err(StatusCode::TOO_MANY_REQUESTS, "Too many attempts. Try again later."))
    }
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
pub mod watchers;
pub use watchers::{watch_task, unwatch_task, get_task_watchers, get_watched_tasks,
    __path_watch_task, __path_unwatch_task, __path_get_task_watchers, __path_get_watched_tasks};
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
mod custom_statuses;
pub use custom_statuses::*;
mod custom_fields;
pub use custom_fields::*;
mod checklists;
pub use checklists::*;
mod saved_views;
pub use saved_views::*;