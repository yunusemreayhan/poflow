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

// S5: Encrypt/decrypt webhook secrets at rest using JWT secret as key
fn derive_key() -> Vec<u8> {
    use hmac::{Hmac, Mac, KeyInit};
    use sha2::Sha256;
    let jwt_secret = std::env::var("POMODORO_JWT_SECRET").unwrap_or_else(|_| "default-key".to_string());
    let mut mac = Hmac::<Sha256>::new_from_slice(jwt_secret.as_bytes()).unwrap();
    mac.update(b"webhook-secret-encryption");
    mac.finalize().into_bytes().to_vec()
}

fn encrypt_secret(plaintext: &str) -> String {
    let key = derive_key();
    let encrypted: Vec<u8> = plaintext.as_bytes().iter().enumerate().map(|(i, b)| b ^ key[i % key.len()]).collect();
    encrypted.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn decrypt_secret(ciphertext: &str) -> Option<String> {
    let encrypted: Vec<u8> = (0..ciphertext.len()).step_by(2)
        .map(|i| u8::from_str_radix(&ciphertext[i..i+2], 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    let key = derive_key();
    let decrypted: Vec<u8> = encrypted.iter().enumerate().map(|(i, b)| b ^ key[i % key.len()]).collect();
    String::from_utf8(decrypted).ok()
}

pub async fn list_webhooks(pool: &Pool, user_id: i64) -> Result<Vec<Webhook>> {
    Ok(sqlx::query_as::<_, Webhook>("SELECT * FROM webhooks WHERE user_id = ? ORDER BY id").bind(user_id).fetch_all(pool).await?)
}

pub async fn create_webhook(pool: &Pool, user_id: i64, url: &str, events: &str, secret: Option<&str>) -> Result<Webhook> {
    let now = now_str();
    let encrypted = secret.map(encrypt_secret);
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
    // B11: Escape LIKE wildcards in event name
    let escaped = event.replace('%', "\\%").replace('_', "\\_");
    Ok(sqlx::query_as::<_, Webhook>("SELECT * FROM webhooks WHERE active = 1 AND (events = '*' OR events LIKE ? ESCAPE '\\')")
        .bind(format!("%{}%", escaped)).fetch_all(pool).await?)
}
