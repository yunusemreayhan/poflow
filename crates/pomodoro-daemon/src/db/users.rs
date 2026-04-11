use super::*;


pub(crate) async fn seed_root_user(pool: &Pool) -> Result<()> {
    let count = user_count(pool).await?;
    if count == 0 {
        let hash = bcrypt::hash("root", 12).map_err(|e| anyhow::anyhow!(e))?;
        create_user(pool, "root", &hash, "root").await?;
        tracing::info!("Seeded default root user (root/root)");
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
    sqlx::query("DELETE FROM burn_log WHERE user_id = ?").bind(id).execute(pool).await?;
    sqlx::query("DELETE FROM comments WHERE user_id = ?").bind(id).execute(pool).await?;
    sqlx::query("DELETE FROM task_assignees WHERE user_id = ?").bind(id).execute(pool).await?;
    sqlx::query("DELETE FROM room_members WHERE user_id = ?").bind(id).execute(pool).await?;
    sqlx::query("DELETE FROM room_votes WHERE user_id = ?").bind(id).execute(pool).await?;
    sqlx::query("DELETE FROM audit_log WHERE user_id = ?").bind(id).execute(pool).await?;
    sqlx::query("DELETE FROM webhooks WHERE user_id = ?").bind(id).execute(pool).await?;
    sqlx::query("UPDATE sessions SET user_id = (SELECT id FROM users WHERE role = 'root' LIMIT 1) WHERE user_id = ?").bind(id).execute(pool).await?;
    sqlx::query("UPDATE tasks SET user_id = (SELECT id FROM users WHERE role = 'root' LIMIT 1) WHERE user_id = ?").bind(id).execute(pool).await?;
    sqlx::query("UPDATE sprint_tasks SET added_by_id = (SELECT id FROM users WHERE role = 'root' LIMIT 1) WHERE added_by_id = ?").bind(id).execute(pool).await?;
    sqlx::query("UPDATE rooms SET creator_id = (SELECT id FROM users WHERE role = 'root' LIMIT 1) WHERE creator_id = ?").bind(id).execute(pool).await?;
    sqlx::query("UPDATE sprints SET created_by_id = (SELECT id FROM users WHERE role = 'root' LIMIT 1) WHERE created_by_id = ?").bind(id).execute(pool).await?;
    sqlx::query("DELETE FROM users WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

pub async fn update_user_role(pool: &Pool, id: i64, role: &str) -> Result<User> {
    sqlx::query("UPDATE users SET role = ? WHERE id = ?").bind(role).bind(id).execute(pool).await?;
    get_user(pool, id).await
}

pub async fn update_user_password(pool: &Pool, id: i64, password_hash: &str) -> Result<()> {
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?").bind(password_hash).bind(id).execute(pool).await?;
    Ok(())
}

pub async fn update_username(pool: &Pool, id: i64, username: &str) -> Result<()> {
    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM users WHERE username = ? AND id != ?")
        .bind(username).bind(id).fetch_optional(pool).await?;
    if existing.is_some() { return Err(anyhow::anyhow!("Username already taken")); }
    sqlx::query("UPDATE users SET username = ? WHERE id = ?").bind(username).bind(id).execute(pool).await?;
    Ok(())
}

// --- Task CRUD ---
