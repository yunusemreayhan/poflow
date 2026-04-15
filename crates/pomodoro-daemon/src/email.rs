//! Email notifications via SMTP.
//!
//! Configure via environment variables:
//! - POMODORO_SMTP_HOST (e.g., smtp.gmail.com)
//! - POMODORO_SMTP_PORT (default: 587)
//! - POMODORO_SMTP_USER
//! - POMODORO_SMTP_PASS
//! - POMODORO_SMTP_FROM (e.g., pomodoro@example.com)
//!
//! If POMODORO_SMTP_HOST is not set, email is disabled (no-op).

use std::sync::OnceLock;

struct SmtpConfig {
    host: String,
    port: u16,
    user: String,
    pass: String,
    from: String,
}

static CONFIG: OnceLock<Option<SmtpConfig>> = OnceLock::new();

fn config() -> &'static Option<SmtpConfig> {
    CONFIG.get_or_init(|| {
        let host = std::env::var("POMODORO_SMTP_HOST").ok()?;
        Some(SmtpConfig {
            host,
            port: std::env::var("POMODORO_SMTP_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(587),
            user: std::env::var("POMODORO_SMTP_USER").unwrap_or_default(),
            pass: std::env::var("POMODORO_SMTP_PASS").unwrap_or_default(),
            from: std::env::var("POMODORO_SMTP_FROM").unwrap_or_else(|_| "pomodoro@localhost".to_string()),
        })
    })
}

/// Send an email notification. No-op if SMTP is not configured.
pub fn send(to_email: &str, subject: &str, body: &str) {
    let cfg = match config() {
        Some(c) => c,
        None => return,
    };
    let to = to_email.to_string();
    let subj = subject.to_string();
    let text = body.to_string();
    let from = cfg.from.clone();
    let host = cfg.host.clone();
    let port = cfg.port;
    let user = cfg.user.clone();
    let pass = cfg.pass.clone();

    tokio::spawn(async move {
        use lettre::{Message, SmtpTransport, Transport};
        use lettre::transport::smtp::authentication::Credentials;

        let email = match Message::builder()
            .from(from.parse().unwrap_or_else(|_| "pomodoro@localhost".parse().unwrap()))
            .to(match to.parse() { Ok(a) => a, Err(_) => return })
            .subject(subj)
            .body(text) {
            Ok(e) => e,
            Err(e) => { tracing::warn!("Email build error: {}", e); return; }
        };

        let transport = if user.is_empty() {
            SmtpTransport::builder_dangerous(&host).port(port).build()
        } else {
            match SmtpTransport::relay(&host) {
                Ok(b) => b.port(port).credentials(Credentials::new(user, pass)).build(),
                Err(e) => { tracing::warn!("SMTP relay error: {}", e); return; }
            }
        };

        match transport.send(&email) {
            Ok(_) => tracing::debug!("Email sent to {}", to),
            Err(e) => tracing::warn!("Email send error: {}", e),
        }
    });
}

/// Send task assignment notification
pub fn notify_assigned(to_email: &str, task_title: &str, assigned_by: &str) {
    send(to_email,
        &format!("Task assigned: {}", task_title),
        &format!("You were assigned to \"{}\" by {}.\n\nLog in to view the task.", task_title, assigned_by));
}

/// Send due date reminder
pub fn notify_due_soon(to_email: &str, task_title: &str, due_date: &str) {
    send(to_email,
        &format!("Due soon: {}", task_title),
        &format!("Task \"{}\" is due on {}.\n\nLog in to update your progress.", task_title, due_date));
}
