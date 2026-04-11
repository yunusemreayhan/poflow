use crate::db::{self, Pool};

/// Fire webhooks for an event in the background. Non-blocking, errors are logged.
pub fn dispatch(pool: Pool, event: &str, payload: serde_json::Value) {
    let event = event.to_string();
    tokio::spawn(async move {
        let hooks = match db::get_active_webhooks(&pool, &event).await {
            Ok(h) => h,
            Err(e) => { tracing::warn!("Failed to load webhooks: {}", e); return; }
        };
        let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build().unwrap_or_default();
        for hook in hooks {
            let mut req = client.post(&hook.url)
                .header("content-type", "application/json")
                .header("x-pomodoro-event", &event)
                .json(&serde_json::json!({ "event": &event, "data": &payload }));
            if let Some(ref secret) = hook.secret {
                req = req.header("x-pomodoro-secret", secret);
            }
            if let Err(e) = req.send().await {
                tracing::warn!("Webhook {} failed: {}", hook.url, e);
            }
        }
    });
}
