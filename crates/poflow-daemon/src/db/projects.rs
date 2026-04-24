use super::*;

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub key: String,
    pub lead_user_id: Option<i64>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn list_projects(pool: &Pool) -> Result<Vec<Project>> {
    Ok(
        sqlx::query_as::<_, Project>("SELECT * FROM projects ORDER BY name")
            .fetch_all(pool)
            .await?,
    )
}

pub async fn get_project(pool: &Pool, id: i64) -> Result<Project> {
    Ok(
        sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = ?")
            .bind(id)
            .fetch_one(pool)
            .await?,
    )
}

pub async fn get_project_by_key(pool: &Pool, key: &str) -> Result<Project> {
    Ok(
        sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE key = ?")
            .bind(key)
            .fetch_one(pool)
            .await?,
    )
}

pub async fn create_project(
    pool: &Pool,
    name: &str,
    description: Option<&str>,
    key: &str,
    lead_user_id: Option<i64>,
) -> Result<Project> {
    let now = now_str();
    let id = sqlx::query("INSERT INTO projects (name, description, key, lead_user_id, status, created_at, updated_at) VALUES (?, ?, ?, ?, 'active', ?, ?)")
        .bind(name).bind(description).bind(key).bind(lead_user_id).bind(&now).bind(&now)
        .execute(pool).await?.last_insert_rowid();
    get_project(pool, id).await
}

pub async fn update_project(
    pool: &Pool,
    id: i64,
    name: Option<&str>,
    description: Option<Option<&str>>,
    key: Option<&str>,
    lead_user_id: Option<Option<i64>>,
    status: Option<&str>,
) -> Result<Project> {
    let current = get_project(pool, id).await?;
    let new_name = name.unwrap_or(&current.name);
    let new_desc = match description {
        Some(v) => v.map(|s| s.to_string()),
        None => current.description,
    };
    let new_key = key.unwrap_or(&current.key);
    let new_lead = match lead_user_id {
        Some(v) => v,
        None => current.lead_user_id,
    };
    let new_status = status.unwrap_or(&current.status);
    let now = now_str();
    sqlx::query("UPDATE projects SET name = ?, description = ?, key = ?, lead_user_id = ?, status = ?, updated_at = ? WHERE id = ?")
        .bind(new_name).bind(&new_desc).bind(new_key).bind(new_lead).bind(new_status).bind(&now).bind(id)
        .execute(pool).await?;
    get_project(pool, id).await
}

pub async fn delete_project(pool: &Pool, id: i64) -> Result<()> {
    // Unlink tasks/sprints/rooms before deleting
    sqlx::query("UPDATE tasks SET project_id = NULL WHERE project_id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    sqlx::query("UPDATE sprints SET project_id = NULL WHERE project_id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    sqlx::query("UPDATE rooms SET project_id = NULL WHERE project_id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    let r = sqlx::query("DELETE FROM projects WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    if r.rows_affected() == 0 {
        return Err(anyhow::anyhow!("Project not found"));
    }
    Ok(())
}
