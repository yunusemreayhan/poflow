use super::*;


pub(crate) async fn seed_root_user(pool: &Pool) -> Result<()> {
    let (root_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE role = 'root'")
        .fetch_one(pool).await?;
    if root_count == 0 {
        let password = std::env::var("POMODORO_ROOT_PASSWORD").unwrap_or_else(|_| {
            let mut buf = [0u8; 16];
            getrandom::fill(&mut buf).expect("getrandom failed for root password");
            let pw: String = buf.iter().map(|b| format!("{:02x}", b)).collect();
            // V30-8: Write generated password to a restricted file instead of logging
            let pw_path = super::data_dir().join(".root_password");
            if let Err(e) = std::fs::write(&pw_path, &pw) {
                tracing::error!("Failed to write root password file: {} — set POMODORO_ROOT_PASSWORD env var instead", e);
            } else {
                #[cfg(unix)] {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&pw_path, std::fs::Permissions::from_mode(0o600)).ok();
                }
                tracing::warn!("Root password written to {} — set POMODORO_ROOT_PASSWORD to override", pw_path.display());
            }
            pw
        });
        let hash = bcrypt::hash(&password, 12).map_err(|e| anyhow::anyhow!(e))?;
        create_user(pool, "root", &hash, "root").await?;
        tracing::info!("Seeded default root user");
    }
    Ok(())
}

pub async fn create_user(pool: &Pool, username: &str, password_hash: &str, role: &str) -> Result<User> {
    let now = now_str();
    let id = sqlx::query("INSERT INTO users (username, password_hash, role, created_at) VALUES (?, ?, ?, ?)")
        .bind(username).bind(password_hash).bind(role).bind(&now)
        .execute(pool).await?.last_insert_rowid();
    Ok(sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?").bind(id).fetch_one(pool).await?)
}

pub async fn get_user_by_username(pool: &Pool, username: &str) -> Result<User> {
    Ok(sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?").bind(username).fetch_one(pool).await?)
}

pub async fn get_user(pool: &Pool, id: i64) -> Result<User> {
    Ok(sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?").bind(id).fetch_one(pool).await?)
}

pub async fn user_count(pool: &Pool) -> Result<i64> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users").fetch_one(pool).await?;
    Ok(count)
}

pub async fn list_users(pool: &Pool) -> Result<Vec<User>> {
    Ok(sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY created_at ASC").fetch_all(pool).await?)
}

pub async fn delete_user(pool: &Pool, id: i64) -> Result<()> {
    let user = get_user(pool, id).await?;
    if user.role == "root" {
        let (root_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE role = 'root'").fetch_one(pool).await?;
        if root_count <= 1 { return Err(anyhow::anyhow!("Cannot delete the last root user")); }
    }
    // B10: Wrap in transaction for atomicity
    let mut tx = pool.begin().await?;
    // B1-v24: Verify reassignment target exists before proceeding
    let target: Option<(i64,)> = sqlx::query_as("SELECT id FROM users WHERE role = 'root' AND id != ? LIMIT 1")
        .bind(id).fetch_optional(&mut *tx).await?;
    let target_id = target.map(|(id,)| id).ok_or_else(|| anyhow::anyhow!("No other root user to reassign resources to"))?;
    sqlx::query("UPDATE burn_log SET user_id = ? WHERE user_id = ?").bind(target_id).bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE comments SET user_id = ? WHERE user_id = ?").bind(target_id).bind(id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM task_assignees WHERE user_id = ?").bind(id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM room_members WHERE user_id = ?").bind(id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM room_votes WHERE user_id = ?").bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE audit_log SET user_id = ? WHERE user_id = ?").bind(target_id).bind(id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM webhooks WHERE user_id = ?").bind(id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM notifications WHERE user_id = ?").bind(id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM notification_prefs WHERE user_id = ?").bind(id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM task_watchers WHERE user_id = ?").bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE task_attachments SET user_id = ? WHERE user_id = ?").bind(target_id).bind(id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM user_configs WHERE user_id = ?").bind(id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM team_members WHERE user_id = ?").bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE sessions SET user_id = ? WHERE user_id = ?").bind(target_id).bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE tasks SET user_id = ? WHERE user_id = ?").bind(target_id).bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE sprint_tasks SET added_by_id = ? WHERE added_by_id = ?").bind(target_id).bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE rooms SET creator_id = ? WHERE creator_id = ?").bind(target_id).bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE sprints SET created_by_id = ? WHERE created_by_id = ?").bind(target_id).bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE epic_groups SET created_by = ? WHERE created_by = ?").bind(target_id).bind(id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM users WHERE id = ?").bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn update_user_role(pool: &Pool, id: i64, role: &str) -> Result<User> {
    sqlx::query("UPDATE users SET role = ? WHERE id = ?").bind(role).bind(id).execute(pool).await?;
    get_user(pool, id).await
}

pub async fn update_user_password(pool: &Pool, id: i64, password_hash: &str) -> Result<()> {
    sqlx::query("UPDATE users SET password_hash = ?, password_changed_at = ? WHERE id = ?")
        .bind(password_hash).bind(now_str()).bind(id).execute(pool).await?;
    Ok(())
}

/// S2: Rehash password without updating password_changed_at (for bcrypt cost upgrades on login)
pub async fn rehash_user_password(pool: &Pool, id: i64, password_hash: &str) -> Result<()> {
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(password_hash).bind(id).execute(pool).await?;
    Ok(())
}

pub async fn update_username(pool: &Pool, id: i64, username: &str) -> Result<()> {
    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM users WHERE username = ? AND id != ?")
        .bind(username).bind(id).fetch_optional(pool).await?;
    if existing.is_some() { return Err(anyhow::anyhow!("Username already taken")); }
    sqlx::query("UPDATE users SET username = ? WHERE id = ?").bind(username).bind(id).execute(pool).await?;
    Ok(())
}

pub async fn list_usernames(pool: &Pool) -> Result<Vec<(i64, String)>> {
    let rows: Vec<(i64, String)> = sqlx::query_as("SELECT id, username FROM users ORDER BY username").fetch_all(pool).await?;
    Ok(rows)
}

// --- Task CRUD ---
