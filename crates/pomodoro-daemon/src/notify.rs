use crate::engine::TimerPhase;
use anyhow::Result;

pub fn send_notification(title: &str, body: &str, phase: TimerPhase) -> Result<()> {
    let icon = match phase {
        TimerPhase::Work => "dialog-warning",
        TimerPhase::ShortBreak => "dialog-information",
        TimerPhase::LongBreak => "dialog-information",
        TimerPhase::Idle => "dialog-information",
    };

    notify_rust::Notification::new()
        .summary(title)
        .body(body)
        .icon(icon)
        .appname("Pomodoro")
        .urgency(notify_rust::Urgency::Normal)
        .timeout(notify_rust::Timeout::Milliseconds(8000))
        .show()?;

    Ok(())
}

pub fn notify_session_complete(phase: TimerPhase, session_count: u32) {
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
    send_notification(&title, &body, phase).ok();
}
