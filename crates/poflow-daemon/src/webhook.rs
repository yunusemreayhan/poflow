use crate::db::{self, Pool};
use std::net::IpAddr;

static WEBHOOK_CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
fn webhook_client() -> &'static reqwest::Client {
    WEBHOOK_CLIENT.get_or_init(|| reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build().unwrap_or_default())
}

/// Public wrapper for route-level validation
pub fn is_private_ip_pub(ip: &IpAddr) -> bool { is_private_ip(ip) }

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

async fn is_safe_url(url: &str) -> Option<(String, std::net::SocketAddr)> {
    let Ok(parsed) = url::Url::parse(url) else { return None };
    if !matches!(parsed.scheme(), "http" | "https") { return None; }
    let host = parsed.host_str()?;
    let port = parsed.port().unwrap_or(if parsed.scheme() == "https" { 443 } else { 80 });
    // Direct IP check
    if let Ok(ip) = host.parse::<IpAddr>() {
        return if is_private_ip(&ip) { None } else { Some((url.to_string(), std::net::SocketAddr::new(ip, port))) };
    }
    // DNS resolution check — resolve once and pin IP
    match tokio::net::lookup_host(format!("{}:{}", host, port)).await {
        Ok(mut addrs) => {
            addrs.find(|a| !is_private_ip(&a.ip()))
                .map(|a| (url.to_string(), a))
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
            let Some((safe_url, pinned_addr)) = is_safe_url(&hook.url).await else {
                tracing::warn!("Webhook {} blocked: resolves to private/loopback IP", hook.url);
                continue;
            };
            // Build a per-hook client that pins DNS to the resolved IP (preserves TLS SNI)
            let pinned_client = {
                let host = url::Url::parse(&hook.url).ok().and_then(|u| u.host_str().map(|s| s.to_string())).unwrap_or_default();
                reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(10))
                    .resolve(&host, pinned_addr)
                    .build().unwrap_or_else(|_| client.clone())
            };
            let body = serde_json::json!({ "event": &event, "data": &payload });
            let body_str = serde_json::to_string(&body).unwrap_or_default();
            // B10: Compute signature once, reuse on retries
            let signature = if let Some(ref encrypted_secret) = hook.secret {
                let secret = db::webhooks::decrypt_secret(encrypted_secret).unwrap_or_default();
                if !secret.is_empty() {
                    use hmac::{Hmac, Mac, KeyInit};
                    use sha2::Sha256;
                    if let Ok(mut mac) = <Hmac<Sha256>>::new_from_slice(secret.as_bytes()) {
                        mac.update(body_str.as_bytes());
                        Some(format!("sha256={}", mac.finalize().into_bytes().iter().map(|b| format!("{:02x}", b)).collect::<String>()))
                    } else { None }
                } else { None }
            } else { None };
            let mut attempts = 0u32;
            let mut last_status: Option<u16> = None;
            #[allow(unused_assignments)]
            let mut last_error: Option<String> = None;
            loop {
                attempts += 1;
                // B10: Rebuild full request each attempt to avoid lost headers on try_clone failure
                let mut retry_req = pinned_client.post(&safe_url)
                    .header("content-type", "application/json")
                    .header("x-poflow-event", &event)
                    .body(body_str.clone());
                if let Some(ref sig) = signature { retry_req = retry_req.header("x-poflow-signature", sig.as_str()); }
                match retry_req.send().await {
                    Ok(resp) if resp.status().is_success() => {
                        last_status = Some(resp.status().as_u16());
                        last_error = None;
                        break;
                    }
                    Ok(resp) => {
                        let code = resp.status().as_u16();
                        tracing::warn!("Webhook {} returned {}", hook.url, code);
                        last_status = Some(code);
                        last_error = Some(format!("HTTP {}", code));
                        if attempts >= 3 { break; }
                    }
                    Err(e) => {
                        tracing::warn!("Webhook {} attempt {}/3 failed: {}", hook.url, attempts, e);
                        last_error = Some(e.to_string().chars().take(500).collect());
                        if attempts >= 3 { break; }
                    }
                }
                let base = 1u64 << attempts;
                let mut jitter_buf = [0u8; 1];
                let jitter_ms = if getrandom::fill(&mut jitter_buf).is_ok() { jitter_buf[0] as u64 * 4 } else { 0 };
                tokio::time::sleep(std::time::Duration::from_millis(base * 1000 + jitter_ms)).await;
            }
            // Log delivery result
            let success = last_error.is_none();
            if let Err(e) = db::log_delivery(&pool, hook.id, &event, last_status, success, attempts, last_error.as_deref()).await {
                tracing::warn!("Failed to log webhook delivery: {}", e);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    // --- is_private_ip IPv4 ---

    #[test]
    fn private_ip_loopback_v4() {
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2))));
    }

    #[test]
    fn private_ip_rfc1918_10() {
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(10, 255, 255, 255))));
    }

    #[test]
    fn private_ip_rfc1918_172() {
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(172, 16, 0, 0))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(172, 31, 255, 255))));
    }

    #[test]
    fn private_ip_rfc1918_192() {
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1))));
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(192, 168, 255, 255))));
    }

    #[test]
    fn private_ip_link_local_v4() {
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1))));
    }

    #[test]
    fn private_ip_broadcast_v4() {
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::BROADCAST)));
    }

    #[test]
    fn private_ip_unspecified_v4() {
        assert!(is_private_ip(&IpAddr::V4(Ipv4Addr::UNSPECIFIED)));
    }

    #[test]
    fn public_ip_v4() {
        assert!(!is_private_ip(&IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_private_ip(&IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
        assert!(!is_private_ip(&IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))));
    }

    // --- is_private_ip IPv6 ---

    #[test]
    fn private_ip_loopback_v6() {
        assert!(is_private_ip(&IpAddr::V6(Ipv6Addr::LOCALHOST)));
    }

    #[test]
    fn private_ip_unspecified_v6() {
        assert!(is_private_ip(&IpAddr::V6(Ipv6Addr::UNSPECIFIED)));
    }

    #[test]
    fn private_ip_link_local_v6() {
        // fe80::1
        assert!(is_private_ip(&IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1))));
    }

    #[test]
    fn private_ip_unique_local_v6() {
        // fc00::1
        assert!(is_private_ip(&IpAddr::V6(Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 1))));
        // fd00::1
        assert!(is_private_ip(&IpAddr::V6(Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1))));
    }

    #[test]
    fn private_ip_v4_mapped_loopback() {
        // ::ffff:127.0.0.1
        assert!(is_private_ip(&IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0x7f00, 0x0001))));
    }

    #[test]
    fn private_ip_v4_mapped_private() {
        // ::ffff:10.0.0.1
        assert!(is_private_ip(&IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0x0a00, 0x0001))));
    }

    #[test]
    fn public_ip_v4_mapped() {
        // ::ffff:8.8.8.8
        assert!(!is_private_ip(&IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0x0808, 0x0808))));
    }

    #[test]
    fn public_ip_v6() {
        // 2001:db8::1 (documentation range, but not private per our function)
        assert!(!is_private_ip(&IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1))));
        // 2606:4700::1 (Cloudflare)
        assert!(!is_private_ip(&IpAddr::V6(Ipv6Addr::new(0x2606, 0x4700, 0, 0, 0, 0, 0, 1))));
    }

    // --- is_private_ip_pub wrapper ---

    #[test]
    fn public_wrapper_matches() {
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        assert_eq!(is_private_ip_pub(&ip), is_private_ip(&ip));
        let ip: IpAddr = "8.8.8.8".parse().unwrap();
        assert_eq!(is_private_ip_pub(&ip), is_private_ip(&ip));
    }

    // --- is_safe_url ---

    #[tokio::test]
    async fn safe_url_rejects_private_ip() {
        assert!(is_safe_url("http://127.0.0.1/hook").await.is_none());
        assert!(is_safe_url("http://10.0.0.1/hook").await.is_none());
        assert!(is_safe_url("http://192.168.1.1/hook").await.is_none());
    }

    #[tokio::test]
    async fn safe_url_rejects_bad_scheme() {
        assert!(is_safe_url("ftp://example.com/hook").await.is_none());
        assert!(is_safe_url("file:///etc/passwd").await.is_none());
    }

    #[tokio::test]
    async fn safe_url_rejects_invalid() {
        assert!(is_safe_url("not a url").await.is_none());
        assert!(is_safe_url("").await.is_none());
    }

    #[tokio::test]
    async fn safe_url_accepts_public_ip() {
        // Direct public IP should be accepted
        let result = is_safe_url("http://8.8.8.8/hook").await;
        assert!(result.is_some());
        let (url, addr) = result.unwrap();
        assert_eq!(url, "http://8.8.8.8/hook");
        assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)));
    }

    // --- HMAC computation (extracted logic test) ---

    #[test]
    fn hmac_signature_format() {
        use hmac::{Hmac, Mac, KeyInit};
        use sha2::Sha256;
        let secret = "test-secret";
        let body = r#"{"event":"test","data":{}}"#;
        let mut mac = <Hmac<Sha256>>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body.as_bytes());
        let sig = format!("sha256={}", mac.finalize().into_bytes().iter().map(|b| format!("{:02x}", b)).collect::<String>());
        assert!(sig.starts_with("sha256="));
        assert_eq!(sig.len(), 7 + 64); // "sha256=" + 64 hex chars
    }

    #[test]
    fn hmac_deterministic() {
        use hmac::{Hmac, Mac, KeyInit};
        use sha2::Sha256;
        let compute = |s: &str, b: &str| -> String {
            let mut mac = <Hmac<Sha256>>::new_from_slice(s.as_bytes()).unwrap();
            mac.update(b.as_bytes());
            format!("sha256={}", mac.finalize().into_bytes().iter().map(|b| format!("{:02x}", b)).collect::<String>())
        };
        assert_eq!(compute("key", "body"), compute("key", "body"));
        assert_ne!(compute("key1", "body"), compute("key2", "body"));
    }
}
