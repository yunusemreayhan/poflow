use super::*;

#[derive(Debug, Clone, FromRow, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct TaskTemplate {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub data: String, // JSON: {title, description, project, tags, priority, estimated, children: [...]}
    pub created_at: String,
}

pub async fn list_templates(pool: &Pool, user_id: i64) -> Result<Vec<TaskTemplate>> {
    Ok(
        sqlx::query_as("SELECT * FROM task_templates WHERE user_id = ? ORDER BY name")
            .bind(user_id)
            .fetch_all(pool)
            .await?,
    )
}

pub async fn create_template(
    pool: &Pool,
    user_id: i64,
    name: &str,
    data: &str,
) -> Result<TaskTemplate> {
    let now = now_str();
    let id = sqlx::query(
        "INSERT INTO task_templates (user_id, name, data, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(name)
    .bind(data)
    .bind(&now)
    .execute(pool)
    .await?
    .last_insert_rowid();
    Ok(sqlx::query_as("SELECT * FROM task_templates WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?)
}

pub async fn delete_template(pool: &Pool, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM task_templates WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_template(pool: &Pool, id: i64) -> Result<TaskTemplate> {
    Ok(sqlx::query_as("SELECT * FROM task_templates WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?)
}
