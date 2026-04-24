use super::*;

#[derive(Debug, Clone, FromRow, serde::Serialize)]
pub struct TaskDependency {
    pub task_id: i64,
    pub depends_on: i64,
}

pub async fn add_dependency(pool: &Pool, task_id: i64, depends_on: i64) -> Result<()> {
    if task_id == depends_on {
        return Err(anyhow::anyhow!("Task cannot depend on itself"));
    }
    // BL1: Check for circular dependencies — walk chain from depends_on
    let rows: Vec<(i64,)> = sqlx::query_as(
        "WITH RECURSIVE chain(id) AS (
            SELECT depends_on FROM task_dependencies WHERE task_id = ?
            UNION
            SELECT td.depends_on FROM task_dependencies td JOIN chain c ON td.task_id = c.id
        ) SELECT id FROM chain",
    )
    .bind(depends_on)
    .fetch_all(pool)
    .await?;
    if rows.iter().any(|(id,)| *id == task_id) {
        return Err(anyhow::anyhow!("Circular dependency detected"));
    }
    sqlx::query("INSERT OR IGNORE INTO task_dependencies (task_id, depends_on) VALUES (?, ?)")
        .bind(task_id)
        .bind(depends_on)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn remove_dependency(pool: &Pool, task_id: i64, depends_on: i64) -> Result<()> {
    let r = sqlx::query("DELETE FROM task_dependencies WHERE task_id = ? AND depends_on = ?")
        .bind(task_id)
        .bind(depends_on)
        .execute(pool)
        .await?;
    if r.rows_affected() == 0 {
        return Err(anyhow::anyhow!("Dependency not found"));
    }
    Ok(())
}

pub async fn get_dependencies(pool: &Pool, task_id: i64) -> Result<Vec<i64>> {
    let rows: Vec<(i64,)> =
        sqlx::query_as("SELECT depends_on FROM task_dependencies WHERE task_id = ?")
            .bind(task_id)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

pub async fn get_dependents(pool: &Pool, task_id: i64) -> Result<Vec<i64>> {
    let rows: Vec<(i64,)> =
        sqlx::query_as("SELECT task_id FROM task_dependencies WHERE depends_on = ?")
            .bind(task_id)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

pub async fn get_all_dependencies(pool: &Pool) -> Result<Vec<TaskDependency>> {
    Ok(
        sqlx::query_as::<_, TaskDependency>("SELECT * FROM task_dependencies")
            .fetch_all(pool)
            .await?,
    )
}
