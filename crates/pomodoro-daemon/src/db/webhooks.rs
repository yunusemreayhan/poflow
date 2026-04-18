use super::*;

#[derive(Debug, Clone, FromRow, serde::Serialize)]
pub struct Webhook {
    pub id: i64,
    pub user_id: i64,
    pub url: String,
    pub events: String,
    #[serde(skip_serializing)]
    pub secret: Option<String>,
    pub active: i64,
    pub created_at: String,
}

// S5: Encrypt/decrypt webhook secrets at rest using AES-256-GCM
fn derive_key() -> Result<[u8; 32]> {
    use hmac::{Hmac, Mac, KeyInit};
    use sha2::Sha256;
    let secret_bytes: Vec<u8> = std::env::var("POMODORO_JWT_SECRET")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|s| s.into_bytes())
        .unwrap_or_else(|| {
            let path = crate::db::data_dir().join(".jwt_secret");
            std::fs::read(&path).ok()
                .filter(|d| d.len() >= 32)
                .unwrap_or_else(|| {
                    // Generate a new random secret file if none exists
                    let mut secret = vec![0u8; 64];
                    getrandom::fill(&mut secret).expect("getrandom failed for JWT secret generation");
                    let _ = std::fs::write(&path, &secret);
                    tracing::warn!("Generated new .jwt_secret file for webhook key derivation");
                    secret
                })
        });
    let mut mac = Hmac::<Sha256>::new_from_slice(&secret_bytes)
        .map_err(|e| anyhow::anyhow!("HMAC init: {e}"))?;
    mac.update(b"webhook-secret-encryption");
    let result = mac.finalize().into_bytes();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    Ok(key)
}

fn encrypt_secret(plaintext: &str) -> Result<String> {
    use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
    use aes_gcm::Nonce;
    let key = derive_key()?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| anyhow::anyhow!("AES init: {e}"))?;
    let mut nonce_bytes = [0u8; 12];
    getrandom::fill(&mut nonce_bytes).map_err(|e| anyhow::anyhow!("getrandom: {e}"))?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes()).map_err(|e| anyhow::anyhow!("encrypt: {e}"))?;
    // Format: hex(nonce) + ":" + hex(ciphertext)
    let nonce_hex: String = nonce_bytes.iter().map(|b| format!("{:02x}", b)).collect();
    let ct_hex: String = ciphertext.iter().map(|b| format!("{:02x}", b)).collect();
    Ok(format!("{}:{}", nonce_hex, ct_hex))
}

pub fn decrypt_secret(stored: &str) -> Option<String> {
    // Try AES-GCM format first (nonce:ciphertext)
    if let Some((nonce_hex, ct_hex)) = stored.split_once(':') {
        if nonce_hex.len() == 24 {
            return decrypt_secret_aes(nonce_hex, ct_hex);
        }
    }
    // Fallback: legacy XOR format (no colon)
    decrypt_secret_xor(stored)
}

/// V38-4: Re-encrypt any legacy XOR webhook secrets to AES-GCM at startup
pub async fn migrate_legacy_secrets(pool: &Pool) {
    let rows: Vec<(i64, String)> = sqlx::query_as("SELECT id, secret FROM webhooks WHERE secret IS NOT NULL")
        .fetch_all(pool).await.unwrap_or_default();
    for (id, stored) in rows {
        // Skip if already AES-GCM format (contains colon with 24-char nonce hex)
        if stored.split_once(':').is_some_and(|(n, _)| n.len() == 24) { continue; }
        // Try XOR decrypt, then re-encrypt with AES-GCM
        if let Some(plaintext) = decrypt_secret_xor(&stored) {
            if let Ok(new_encrypted) = encrypt_secret(&plaintext) {
                sqlx::query("UPDATE webhooks SET secret = ? WHERE id = ?")
                    .bind(&new_encrypted).bind(id).execute(pool).await.ok();
                tracing::info!("Migrated webhook {} secret from XOR to AES-GCM", id);
            }
        }
    }
}

fn decrypt_secret_aes(nonce_hex: &str, ct_hex: &str) -> Option<String> {
    use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
    use aes_gcm::Nonce;
    let nonce_bytes: Vec<u8> = (0..nonce_hex.len()).step_by(2)
        .map(|i| u8::from_str_radix(&nonce_hex[i..i+2], 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    let ciphertext: Vec<u8> = (0..ct_hex.len()).step_by(2)
        .map(|i| u8::from_str_radix(&ct_hex[i..i+2], 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    let key = derive_key().ok()?;
    let cipher = Aes256Gcm::new_from_slice(&key).ok()?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let plaintext = cipher.decrypt(nonce, ciphertext.as_ref()).ok()?;
    String::from_utf8(plaintext).ok()
}

fn decrypt_secret_xor(ciphertext_hex: &str) -> Option<String> {
    // Legacy XOR decryption for backwards compatibility
    let encrypted: Vec<u8> = (0..ciphertext_hex.len()).step_by(2)
        .map(|i| u8::from_str_radix(&ciphertext_hex[i..i+2], 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    use hmac::{Hmac, Mac, KeyInit};
    use sha2::Sha256;
    let secret_bytes: Vec<u8> = std::env::var("POMODORO_JWT_SECRET")
        .ok().filter(|s| !s.is_empty()).map(|s| s.into_bytes())
        .unwrap_or_else(|| {
            let path = crate::db::data_dir().join(".jwt_secret");
            std::fs::read(&path).ok().filter(|d| d.len() >= 32)
                .unwrap_or_else(|| { use sha2::{Sha256 as S, Digest}; S::digest(crate::db::data_dir().to_string_lossy().as_bytes()).to_vec() })
        });
    let mut mac = Hmac::<Sha256>::new_from_slice(&secret_bytes).ok()?;
    mac.update(b"webhook-secret-encryption");
    let key = mac.finalize().into_bytes().to_vec();
    let decrypted: Vec<u8> = encrypted.iter().enumerate().map(|(i, b)| b ^ key[i % key.len()]).collect();
    String::from_utf8(decrypted).ok()
}

pub async fn list_webhooks(pool: &Pool, user_id: i64) -> Result<Vec<Webhook>> {
    Ok(sqlx::query_as::<_, Webhook>("SELECT * FROM webhooks WHERE user_id = ? ORDER BY id").bind(user_id).fetch_all(pool).await?)
}

pub async fn create_webhook(pool: &Pool, user_id: i64, url: &str, events: &str, secret: Option<&str>) -> Result<Webhook> {
    let now = now_str();
    let encrypted = secret.map(encrypt_secret).transpose()?;
    let id = sqlx::query("INSERT INTO webhooks (user_id, url, events, secret, created_at) VALUES (?,?,?,?,?)")
        .bind(user_id).bind(url).bind(events).bind(&encrypted).bind(&now)
        .execute(pool).await?.last_insert_rowid();
    Ok(sqlx::query_as::<_, Webhook>("SELECT * FROM webhooks WHERE id = ?").bind(id).fetch_one(pool).await?)
}

pub async fn delete_webhook(pool: &Pool, id: i64, user_id: i64) -> Result<()> {
    let r = sqlx::query("DELETE FROM webhooks WHERE id = ? AND user_id = ?").bind(id).bind(user_id).execute(pool).await?;
    if r.rows_affected() == 0 { return Err(anyhow::anyhow!("Webhook not found")); }
    Ok(())
}

pub async fn get_active_webhooks(pool: &Pool, event: &str) -> Result<Vec<Webhook>> {
    // V31-5: Use exact comma-delimited matching instead of LIKE
    // V37-20: Also match slack:-prefixed events for Slack integrations
    let slack_event = format!("slack:{}", event);
    Ok(sqlx::query_as::<_, Webhook>(
        "SELECT * FROM webhooks WHERE active = 1 AND (events = '*' OR events = ? OR events LIKE ? OR events LIKE ? OR events LIKE ? OR events = ? OR events LIKE ? OR events LIKE ? OR events LIKE ?)"
    )
        .bind(event)
        .bind(format!("{},%", event))
        .bind(format!("%,{}", event))
        .bind(format!("%,{},%", event))
        .bind(&slack_event)
        .bind(format!("{},%", slack_event))
        .bind(format!("%,{}", slack_event))
        .bind(format!("%,{},%", slack_event))
        .fetch_all(pool).await?)
}
