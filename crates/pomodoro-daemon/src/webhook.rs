use crate::db::{self, Pool};
use std::net::IpAddr;

static WEBHOOK_CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
fn webhook_client() -> &'static reqwest::Client {
    WEBHOOK_CLIENT.get_or_init(|| reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build().unwrap_or_default())
}

fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_broadcast() || v4.is_unspecified(),
        IpAddr::V6(v6) => {
            if v6.is_loopback() || v6.is_unspecified() { return true; }
            let segs = v6.segments();
            // Link-local fe80::/10
            if segs[0] & 0xffc0 == 0xfe80 { return true; }
            // Unique local fc00::/7
            if segs[0] & 0xfe00 == 0xfc00 { return true; }
            // IPv4-mapped ::ffff:x.x.x.x
            if let Some(v4) = v6.to_ipv4_mapped() {
                return v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_broadcast() || v4.is_unspecified();
            }
            false
        },
    }
}

async fn is_safe_url(url: &str) -> Option<String> {
    let Ok(parsed) = url::Url::parse(url) else { return None };
    if !matches!(parsed.scheme(), "http" | "https") { return None; }
    let Some(host) = parsed.host_str() else { return None };
    // Direct IP check
    if let Ok(ip) = host.parse::<IpAddr>() {
        return if is_private_ip(&ip) { None } else { Some(url.to_string()) };
    }
    // DNS resolution check — resolve once and rewrite URL to use resolved IP (prevents DNS rebinding)
    let port = parsed.port().unwrap_or(if parsed.scheme() == "https" { 443 } else { 80 });
    match tokio::net::lookup_host(format!("{}:{}", host, port)).await {
        Ok(mut addrs) => {
            let safe_addr = addrs.find(|a| !is_private_ip(&a.ip()));
            safe_addr.map(|a| {
                let mut pinned = parsed.clone();
                pinned.set_host(Some(&a.ip().to_string())).ok();
                pinned.to_string()
            })
        },
        Err(_) => None,
    }
}

/// Fire webhooks for an event in the background. Non-blocking, errors are logged.
pub fn dispatch(pool: Pool, event: &str, payload: serde_json::Value) {
    let event = event.to_string();
    tokio::spawn(async move {
        let hooks = match db::get_active_webhooks(&pool, &event).await {
            Ok(h) => h,
            Err(e) => { tracing::warn!("Failed to load webhooks: {}", e.to_string().chars().take(200).collect::<String>()); return; }
        };
        let client = webhook_client();
        for hook in hooks {
            let Some(safe_url) = is_safe_url(&hook.url).await else {
                tracing::warn!("Webhook {} blocked: resolves to private/loopback IP", hook.url);
                continue;
            };
            let body = serde_json::json!({ "event": &event, "data": &payload });
            let body_str = serde_json::to_string(&body).unwrap_or_default();
            // B10: Compute signature once, reuse on retries
            let signature = if let Some(ref encrypted_secret) = hook.secret {
                let secret = db::webhooks::decrypt_secret(encrypted_secret).unwrap_or_default();
                if !secret.is_empty() {
                    use hmac::{Hmac, Mac, KeyInit};
                    use sha2::Sha256;
                    let mut mac = <Hmac<Sha256>>::new_from_slice(secret.as_bytes()).unwrap();
                    mac.update(body_str.as_bytes());
                    Some(format!("sha256={}", mac.finalize().into_bytes().iter().map(|b| format!("{:02x}", b)).collect::<String>()))
                } else { None }
            } else { None };
            let mut attempts = 0;
            loop {
                attempts += 1;
                // B10: Rebuild full request each attempt to avoid lost headers on try_clone failure
                let mut retry_req = client.post(&safe_url)
                    .header("content-type", "application/json")
                    .header("x-pomodoro-event", &event)
                    .header("host", url::Url::parse(&hook.url).and_then(|u| Ok(u.host_str().unwrap_or("").to_string())).unwrap_or_default())
                    .body(body_str.clone());
                if let Some(ref sig) = signature { retry_req = retry_req.header("x-pomodoro-signature", sig.as_str()); }
                match retry_req.send().await {
                    Ok(resp) if resp.status().is_success() => break,
                    Ok(resp) => {
                        tracing::warn!("Webhook {} returned {}", hook.url, resp.status());
                        if attempts >= 3 { break; }
                    }
                    Err(e) => {
                        tracing::warn!("Webhook {} attempt {}/3 failed: {}", hook.url, attempts, e);
                        if attempts >= 3 { break; }
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(1 << attempts)).await;
            }
        }
    });
}
