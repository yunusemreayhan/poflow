use crate::engine::TimerPhase;
use anyhow::Result;

pub fn send_notification(title: &str, body: &str, phase: TimerPhase, play_sound: bool) -> Result<()> {
    let icon = match phase {
        TimerPhase::Work => "dialog-warning",
        _ => "dialog-information",
    };

    let mut n = notify_rust::Notification::new();
    n.summary(title).body(body).icon(icon).appname("Pomodoro")
        .urgency(notify_rust::Urgency::Normal)
        .timeout(notify_rust::Timeout::Milliseconds(8000));
    if play_sound { n.sound_name("complete"); }
    n.show()?;
    Ok(())
}

pub fn notify_session_complete(phase: TimerPhase, session_count: u32, play_sound: bool) {
    let (title, body) = match phase {
        TimerPhase::ShortBreak => (
            "🍅 Work session complete!".to_string(),
            format!("Great focus! Take a short break. Sessions today: {}", session_count),
        ),
        TimerPhase::LongBreak => (
            "🍅 Work session complete!".to_string(),
            format!("Excellent! You've earned a long break. Sessions: {}", session_count),
        ),
        TimerPhase::Work => (
            "☕ Break is over!".to_string(),
            "Time to get back to work!".to_string(),
        ),
        TimerPhase::Idle => return,
    };
    tokio::task::spawn_blocking(move || {
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| send_notification(&title, &body, phase, play_sound).ok())).is_err() {
            tracing::debug!("Desktop notification unavailable (no D-Bus?)");
        }
    });
}

pub fn notify_due_task(title: &str, urgency: &str) {
    let summary = format!("📅 Task {}", urgency);
    let body = title.to_string();
    tokio::task::spawn_blocking(move || {
        let _ = notify_rust::Notification::new()
            .summary(&summary)
            .body(&body)
            .icon("appointment-soon")
            .appname("Pomodoro")
            .urgency(notify_rust::Urgency::Normal)
            .timeout(notify_rust::Timeout::Milliseconds(10000))
            .show();
    });
}
