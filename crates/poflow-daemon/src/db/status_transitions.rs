use super::{now_str, Pool};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, utoipa::ToSchema)]
pub struct StatusTransition {
    pub id: i64,
    pub from_status: String,
    pub to_status: String,
    pub project_id: Option<i64>,
    pub created_at: String,
}

pub async fn list_status_transitions(
    pool: &Pool,
    project_id: Option<i64>,
) -> Result<Vec<StatusTransition>> {
    let rows = if let Some(pid) = project_id {
        sqlx::query_as::<_, StatusTransition>(
            "SELECT * FROM status_transitions WHERE project_id = ? ORDER BY from_status, to_status",
        )
        .bind(pid)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, StatusTransition>(
            "SELECT * FROM status_transitions WHERE project_id IS NULL ORDER BY from_status, to_status",
        )
        .fetch_all(pool)
        .await?
    };
    Ok(rows)
}

pub async fn create_status_transition(
    pool: &Pool,
    from_status: &str,
    to_status: &str,
    project_id: Option<i64>,
) -> Result<StatusTransition> {
    // Check for duplicates (SQLite UNIQUE treats NULLs as distinct)
    let exists = if let Some(pid) = project_id {
        let (c,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM status_transitions WHERE from_status = ? AND to_status = ? AND project_id = ?",
        ).bind(from_status).bind(to_status).bind(pid).fetch_one(pool).await?;
        c > 0
    } else {
        let (c,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM status_transitions WHERE from_status = ? AND to_status = ? AND project_id IS NULL",
        ).bind(from_status).bind(to_status).fetch_one(pool).await?;
        c > 0
    };
    if exists {
        anyhow::bail!("UNIQUE constraint failed: transition already exists");
    }
    let now = now_str();
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO status_transitions (from_status, to_status, project_id, created_at) VALUES (?, ?, ?, ?) RETURNING id",
    )
    .bind(from_status)
    .bind(to_status)
    .bind(project_id)
    .bind(&now)
    .fetch_one(pool)
    .await?;
    Ok(StatusTransition {
        id,
        from_status: from_status.to_string(),
        to_status: to_status.to_string(),
        project_id,
        created_at: now,
    })
}

pub async fn delete_status_transition(pool: &Pool, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM status_transitions WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Check if a status transition is allowed.
/// Returns Ok(()) if allowed, Err with message if not.
/// Rules: if no transitions are defined (for the project or globally), everything is allowed.
/// If transitions exist, only listed from→to pairs are permitted.
/// Project-specific rules take precedence over global rules.
pub async fn validate_status_transition(
    pool: &Pool,
    from_status: &str,
    to_status: &str,
    project_id: Option<i64>,
) -> Result<()> {
    // Check project-specific rules first
    if let Some(pid) = project_id {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM status_transitions WHERE project_id = ?")
                .bind(pid)
                .fetch_one(pool)
                .await?;
        if count > 0 {
            let (allowed,): (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM status_transitions WHERE from_status = ? AND to_status = ? AND project_id = ?",
            )
            .bind(from_status)
            .bind(to_status)
            .bind(pid)
            .fetch_one(pool)
            .await?;
            if allowed == 0 {
                anyhow::bail!(
                    "Transition from '{}' to '{}' is not allowed",
                    from_status,
                    to_status
                );
            }
            return Ok(());
        }
    }
    // Fall back to global rules
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM status_transitions WHERE project_id IS NULL")
            .fetch_one(pool)
            .await?;
    if count == 0 {
        return Ok(()); // No rules defined = everything allowed
    }
    let (allowed,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM status_transitions WHERE from_status = ? AND to_status = ? AND project_id IS NULL",
    )
    .bind(from_status)
    .bind(to_status)
    .fetch_one(pool)
    .await?;
    if allowed == 0 {
        anyhow::bail!(
            "Transition from '{}' to '{}' is not allowed",
            from_status,
            to_status
        );
    }
    Ok(())
}
