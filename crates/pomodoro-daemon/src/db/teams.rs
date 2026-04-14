use super::*;


pub async fn create_team(pool: &Pool, name: &str) -> Result<Team> {
    let now = now_str();
    let r = sqlx::query("INSERT INTO teams (name, created_at) VALUES (?,?)").bind(name).bind(&now).execute(pool).await?;
    Ok(sqlx::query_as::<_, Team>("SELECT * FROM teams WHERE id = ?").bind(r.last_insert_rowid()).fetch_one(pool).await?)
}

pub async fn list_teams(pool: &Pool) -> Result<Vec<Team>> {
    Ok(sqlx::query_as::<_, Team>("SELECT * FROM teams ORDER BY name").fetch_all(pool).await?)
}

pub async fn get_team_detail(pool: &Pool, id: i64) -> Result<TeamDetail> {
    let team = sqlx::query_as::<_, Team>("SELECT * FROM teams WHERE id = ?").bind(id).fetch_one(pool).await?;
    let members = sqlx::query_as::<_, TeamMember>("SELECT tm.team_id, tm.user_id, u.username, tm.role FROM team_members tm JOIN users u ON u.id = tm.user_id WHERE tm.team_id = ?").bind(id).fetch_all(pool).await?;
    let root_ids: Vec<(i64,)> = sqlx::query_as("SELECT task_id FROM team_root_tasks WHERE team_id = ?").bind(id).fetch_all(pool).await?;
    Ok(TeamDetail { team, members, root_task_ids: root_ids.into_iter().map(|r| r.0).collect() })
}

pub async fn delete_team(pool: &Pool, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM teams WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

pub async fn add_team_member(pool: &Pool, team_id: i64, user_id: i64, role: &str) -> Result<()> {
    sqlx::query("INSERT OR REPLACE INTO team_members (team_id, user_id, role) VALUES (?,?,?)").bind(team_id).bind(user_id).bind(role).execute(pool).await?;
    Ok(())
}

pub async fn remove_team_member(pool: &Pool, team_id: i64, user_id: i64) -> Result<()> {
    let r = sqlx::query("DELETE FROM team_members WHERE team_id = ? AND user_id = ?").bind(team_id).bind(user_id).execute(pool).await?;
    if r.rows_affected() == 0 { return Err(anyhow::anyhow!("Not a member")); }
    Ok(())
}

pub async fn get_user_teams(pool: &Pool, user_id: i64) -> Result<Vec<Team>> {
    Ok(sqlx::query_as::<_, Team>("SELECT t.* FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? ORDER BY t.name").bind(user_id).fetch_all(pool).await?)
}

pub async fn add_team_root_task(pool: &Pool, team_id: i64, task_id: i64) -> Result<()> {
    sqlx::query("INSERT OR IGNORE INTO team_root_tasks (team_id, task_id) VALUES (?,?)").bind(team_id).bind(task_id).execute(pool).await?;
    Ok(())
}

pub async fn remove_team_root_task(pool: &Pool, team_id: i64, task_id: i64) -> Result<()> {
    let r = sqlx::query("DELETE FROM team_root_tasks WHERE team_id = ? AND task_id = ?").bind(team_id).bind(task_id).execute(pool).await?;
    if r.rows_affected() == 0 { return Err(anyhow::anyhow!("Task not a root task")); }
    Ok(())
}

pub async fn is_team_admin(pool: &Pool, team_id: i64, user_id: i64) -> Result<bool> {
    let row: Option<(String,)> = sqlx::query_as("SELECT role FROM team_members WHERE team_id = ? AND user_id = ?")
        .bind(team_id).bind(user_id).fetch_optional(pool).await?;
    Ok(row.is_some_and(|r| r.0 == "admin"))
}
