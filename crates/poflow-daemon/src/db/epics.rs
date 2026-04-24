use super::*;

pub async fn get_sprint_root_tasks(pool: &Pool, sprint_id: i64) -> Result<Vec<i64>> {
    let rows: Vec<(i64,)> =
        sqlx::query_as("SELECT task_id FROM sprint_root_tasks WHERE sprint_id = ?")
            .bind(sprint_id)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

pub async fn add_sprint_root_task(pool: &Pool, sprint_id: i64, task_id: i64) -> Result<()> {
    sqlx::query("INSERT OR IGNORE INTO sprint_root_tasks (sprint_id, task_id) VALUES (?,?)")
        .bind(sprint_id)
        .bind(task_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn remove_sprint_root_task(pool: &Pool, sprint_id: i64, task_id: i64) -> Result<()> {
    let r = sqlx::query("DELETE FROM sprint_root_tasks WHERE sprint_id = ? AND task_id = ?")
        .bind(sprint_id)
        .bind(task_id)
        .execute(pool)
        .await?;
    if r.rows_affected() == 0 {
        return Err(anyhow::anyhow!("Task not a root task"));
    }
    Ok(())
}

pub async fn get_descendant_ids(pool: &Pool, root_ids: &[i64]) -> Result<Vec<i64>> {
    if root_ids.is_empty() {
        return Ok(vec![]);
    }
    let placeholders: String = root_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "WITH RECURSIVE descendants AS (\
            SELECT id, 0 as depth FROM tasks WHERE id IN ({ph}) \
            UNION ALL \
            SELECT t.id, d.depth + 1 FROM tasks t JOIN descendants d ON t.parent_id = d.id WHERE d.depth < 50\
        ) SELECT id FROM descendants",
        ph = placeholders
    );
    let mut q = sqlx::query_as::<_, (i64,)>(&sql);
    for id in root_ids {
        q = q.bind(id);
    }
    let rows: Vec<(i64,)> = q.fetch_all(pool).await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

// --- Epic Groups ---

pub async fn create_epic_group(pool: &Pool, name: &str, user_id: i64) -> Result<EpicGroup> {
    let now = now_str();
    let result = sqlx::query(
        "INSERT INTO epic_groups (name, created_by, created_at, updated_at) VALUES (?,?,?,?)",
    )
    .bind(name)
    .bind(user_id)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    let id = result.last_insert_rowid();
    Ok(
        sqlx::query_as::<_, EpicGroup>("SELECT * FROM epic_groups WHERE id = ?")
            .bind(id)
            .fetch_one(pool)
            .await?,
    )
}

pub async fn list_epic_groups(pool: &Pool) -> Result<Vec<EpicGroup>> {
    Ok(
        sqlx::query_as::<_, EpicGroup>("SELECT * FROM epic_groups ORDER BY id")
            .fetch_all(pool)
            .await?,
    )
}

pub async fn get_epic_group_detail(pool: &Pool, id: i64) -> Result<EpicGroupDetail> {
    let group = sqlx::query_as::<_, EpicGroup>("SELECT * FROM epic_groups WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    let task_ids: Vec<(i64,)> =
        sqlx::query_as("SELECT task_id FROM epic_group_tasks WHERE group_id = ?")
            .bind(id)
            .fetch_all(pool)
            .await?;
    let snapshots = sqlx::query_as::<_, EpicSnapshot>(
        "SELECT * FROM epic_snapshots WHERE group_id = ? ORDER BY date",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    Ok(EpicGroupDetail {
        group,
        task_ids: task_ids.into_iter().map(|r| r.0).collect(),
        snapshots,
    })
}

pub async fn delete_epic_group(pool: &Pool, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM epic_groups WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn add_epic_group_task(pool: &Pool, group_id: i64, task_id: i64) -> Result<()> {
    sqlx::query("INSERT OR IGNORE INTO epic_group_tasks (group_id, task_id) VALUES (?,?)")
        .bind(group_id)
        .bind(task_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn remove_epic_group_task(pool: &Pool, group_id: i64, task_id: i64) -> Result<()> {
    let r = sqlx::query("DELETE FROM epic_group_tasks WHERE group_id = ? AND task_id = ?")
        .bind(group_id)
        .bind(task_id)
        .execute(pool)
        .await?;
    if r.rows_affected() == 0 {
        return Err(anyhow::anyhow!("Task not in epic group"));
    }
    Ok(())
}

pub async fn snapshot_epic_group(pool: &Pool, group_id: i64) -> Result<()> {
    let today = Utc::now().naive_utc().format("%Y-%m-%d").to_string();
    // Get all descendant tasks of the root tasks in this group
    let root_ids: Vec<(i64,)> =
        sqlx::query_as("SELECT task_id FROM epic_group_tasks WHERE group_id = ?")
            .bind(group_id)
            .fetch_all(pool)
            .await?;
    if root_ids.is_empty() {
        return Ok(());
    }
    let rids: Vec<i64> = root_ids.into_iter().map(|r| r.0).collect();
    let all_ids = get_descendant_ids(pool, &rids).await?;

    // Aggregate stats
    let placeholders: String = all_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let q = format!("SELECT COUNT(*), COALESCE(SUM(CASE WHEN status IN ('completed','done') THEN 1 ELSE 0 END),0), \
        COALESCE(SUM(remaining_points),0.0), COALESCE(SUM(CASE WHEN status IN ('completed','done') THEN remaining_points ELSE 0.0 END),0.0), \
        COALESCE(SUM(estimated_hours),0.0), COALESCE(SUM(CASE WHEN status IN ('completed','done') THEN estimated_hours ELSE 0.0 END),0.0) \
        FROM tasks WHERE id IN ({}) AND deleted_at IS NULL", placeholders);
    let mut qb = sqlx::query_as::<_, (i64, i64, f64, f64, f64, f64)>(&q);
    for id in &all_ids {
        qb = qb.bind(id);
    }
    let (total_tasks, done_tasks, total_points, done_points, total_hours, done_hours) =
        qb.fetch_one(pool).await?;

    sqlx::query("INSERT INTO epic_snapshots (group_id, date, total_tasks, done_tasks, total_points, done_points, total_hours, done_hours) \
        VALUES (?,?,?,?,?,?,?,?) ON CONFLICT(group_id, date) DO UPDATE SET total_tasks=excluded.total_tasks, done_tasks=excluded.done_tasks, \
        total_points=excluded.total_points, done_points=excluded.done_points, total_hours=excluded.total_hours, done_hours=excluded.done_hours")
        .bind(group_id).bind(&today).bind(total_tasks).bind(done_tasks).bind(total_points).bind(done_points).bind(total_hours).bind(done_hours)
        .execute(pool).await?;
    Ok(())
}

pub async fn snapshot_all_epic_groups(pool: &Pool) -> Result<()> {
    let groups: Vec<(i64,)> = sqlx::query_as("SELECT id FROM epic_groups")
        .fetch_all(pool)
        .await?;
    for (gid,) in groups {
        snapshot_epic_group(pool, gid).await?;
    }
    Ok(())
}

// --- Teams ---
