use super::*;

const ROOM_SELECT: &str = "SELECT r.id, r.name, r.room_type, r.estimation_unit, r.project, r.project_id, p.name as project_name, r.creator_id, u.username as creator, r.status, r.current_task_id, r.created_at FROM rooms r JOIN users u ON r.creator_id = u.id LEFT JOIN projects p ON r.project_id = p.id";

pub async fn create_room(
    pool: &Pool,
    name: &str,
    room_type: &str,
    estimation_unit: &str,
    project: Option<&str>,
    project_id: Option<i64>,
    creator_id: i64,
) -> Result<Room> {
    let now = now_str();
    let id = sqlx::query("INSERT INTO rooms (name, room_type, estimation_unit, project, project_id, creator_id, status, created_at) VALUES (?, ?, ?, ?, ?, ?, 'lobby', ?)")
        .bind(name).bind(room_type).bind(estimation_unit).bind(project).bind(project_id).bind(creator_id).bind(&now)
        .execute(pool).await?.last_insert_rowid();
    sqlx::query(
        "INSERT INTO room_members (room_id, user_id, role, joined_at) VALUES (?, ?, 'admin', ?)",
    )
    .bind(id)
    .bind(creator_id)
    .bind(&now)
    .execute(pool)
    .await?;
    get_room(pool, id).await
}

pub async fn list_rooms(pool: &Pool) -> Result<Vec<Room>> {
    Ok(sqlx::query_as::<_, Room>(&format!(
        "{} ORDER BY r.created_at DESC LIMIT 200",
        ROOM_SELECT
    ))
    .fetch_all(pool)
    .await?)
}

// B4: Use ROOM_SELECT constant for user-filtered room list
pub async fn list_user_rooms(pool: &Pool, user_id: i64) -> Result<Vec<Room>> {
    Ok(sqlx::query_as::<_, Room>(&format!(
        "{} JOIN room_members rm ON rm.room_id = r.id WHERE rm.user_id = ? ORDER BY r.id DESC",
        ROOM_SELECT
    ))
    .bind(user_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_room(pool: &Pool, id: i64) -> Result<Room> {
    Ok(
        sqlx::query_as::<_, Room>(&format!("{} WHERE r.id = ?", ROOM_SELECT))
            .bind(id)
            .fetch_one(pool)
            .await?,
    )
}

pub async fn delete_room(pool: &Pool, id: i64) -> Result<()> {
    sqlx::query("DELETE FROM rooms WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

const MEMBER_SELECT: &str = "SELECT rm.room_id, rm.user_id, u.username, rm.role, rm.joined_at FROM room_members rm JOIN users u ON rm.user_id = u.id";

pub async fn join_room(pool: &Pool, room_id: i64, user_id: i64) -> Result<()> {
    let now = now_str();
    // BL6: Room creator re-joins as admin
    let (creator_id,): (i64,) = sqlx::query_as("SELECT creator_id FROM rooms WHERE id = ?")
        .bind(room_id)
        .fetch_one(pool)
        .await?;
    let role = if creator_id == user_id {
        "admin"
    } else {
        "voter"
    };
    sqlx::query("INSERT OR IGNORE INTO room_members (room_id, user_id, role, joined_at) VALUES (?, ?, ?, ?)")
        .bind(room_id).bind(user_id).bind(role).bind(&now).execute(pool).await?;
    Ok(())
}

pub async fn leave_room(pool: &Pool, room_id: i64, user_id: i64) -> Result<()> {
    let r = sqlx::query("DELETE FROM room_members WHERE room_id = ? AND user_id = ?")
        .bind(room_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    if r.rows_affected() == 0 {
        return Err(anyhow::anyhow!("Not a member"));
    }
    Ok(())
}

pub async fn set_room_member_role(
    pool: &Pool,
    room_id: i64,
    user_id: i64,
    role: &str,
) -> Result<()> {
    let r = sqlx::query("UPDATE room_members SET role = ? WHERE room_id = ? AND user_id = ?")
        .bind(role)
        .bind(room_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    if r.rows_affected() == 0 {
        return Err(anyhow::anyhow!("User is not a member"));
    }
    Ok(())
}

pub async fn get_room_members(pool: &Pool, room_id: i64) -> Result<Vec<RoomMember>> {
    Ok(sqlx::query_as::<_, RoomMember>(&format!(
        "{} WHERE rm.room_id = ? ORDER BY rm.joined_at",
        MEMBER_SELECT
    ))
    .bind(room_id)
    .fetch_all(pool)
    .await?)
}

pub async fn is_room_admin(pool: &Pool, room_id: i64, user_id: i64) -> Result<bool> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT role FROM room_members WHERE room_id = ? AND user_id = ?")
            .bind(room_id)
            .bind(user_id)
            .fetch_all(pool)
            .await?;
    Ok(rows.first().map(|(r,)| r == "admin").unwrap_or(false))
}

pub async fn start_voting(pool: &Pool, room_id: i64, task_id: i64) -> Result<Room> {
    sqlx::query("UPDATE rooms SET status = 'voting', current_task_id = ? WHERE id = ?")
        .bind(task_id)
        .bind(room_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM room_votes WHERE room_id = ? AND task_id = ?")
        .bind(room_id)
        .bind(task_id)
        .execute(pool)
        .await?;
    get_room(pool, room_id).await
}

pub const VOTE_SELECT: &str = "SELECT rv.id, rv.room_id, rv.task_id, rv.user_id, u.username, rv.value, rv.created_at FROM room_votes rv JOIN users u ON rv.user_id = u.id";

pub async fn cast_vote(
    pool: &Pool,
    room_id: i64,
    task_id: i64,
    user_id: i64,
    value: f64,
) -> Result<()> {
    let now = now_str();
    sqlx::query("INSERT INTO room_votes (room_id, task_id, user_id, value, created_at) VALUES (?, ?, ?, ?, ?) ON CONFLICT(room_id, task_id, user_id) DO UPDATE SET value = ?, created_at = ?")
        .bind(room_id).bind(task_id).bind(user_id).bind(value).bind(&now).bind(value).bind(&now)
        .execute(pool).await?;
    Ok(())
}

pub async fn reveal_votes(pool: &Pool, room_id: i64) -> Result<Room> {
    sqlx::query("UPDATE rooms SET status = 'revealed' WHERE id = ?")
        .bind(room_id)
        .execute(pool)
        .await?;
    get_room(pool, room_id).await
}

pub async fn get_room_votes(pool: &Pool, room_id: i64, task_id: i64) -> Result<Vec<RoomVote>> {
    Ok(sqlx::query_as::<_, RoomVote>(&format!(
        "{} WHERE rv.room_id = ? AND rv.task_id = ?",
        VOTE_SELECT
    ))
    .bind(room_id)
    .bind(task_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_room_state(pool: &Pool, room_id: i64) -> Result<RoomState> {
    let room = get_room(pool, room_id).await?;

    // Fetch members first (needed for task scoping when no project)
    let members = get_room_members(pool, room_id).await?;

    let tasks_fut = async {
        match &room.project {
            Some(p) if !p.is_empty() => {
                sqlx::query_as(&format!(
                "{} WHERE t.project = ? AND t.deleted_at IS NULL ORDER BY t.sort_order LIMIT 1000",
                TASK_SELECT
            ))
                .bind(p)
                .fetch_all(pool)
                .await
            }
            _ => {
                // B3: Scope to tasks owned by room members (not all global leaf tasks)
                let member_ids: Vec<i64> = members.iter().map(|m| m.user_id).collect();
                if member_ids.is_empty() {
                    Ok(vec![])
                } else {
                    let ph = member_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                    let sql = format!("{} WHERE t.user_id IN ({}) AND t.deleted_at IS NULL ORDER BY t.sort_order LIMIT 500", TASK_SELECT, ph);
                    let mut q = sqlx::query_as::<_, Task>(&sql);
                    for uid in &member_ids {
                        q = q.bind(uid);
                    }
                    q.fetch_all(pool).await
                }
            }
        }
    };
    let current_task_fut = async {
        match room.current_task_id {
            Some(tid) => get_task(pool, tid).await.ok(),
            None => None,
        }
    };
    let all_room_votes_fut = async {
        // P1: Limit to last 500 votes (covers current + recent history)
        sqlx::query_as::<_, RoomVote>(&format!(
            "{} WHERE rv.room_id = ? ORDER BY rv.created_at DESC LIMIT 500",
            VOTE_SELECT
        ))
        .bind(room_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    };

    let (tasks_result, current_task, all_room_votes) =
        tokio::join!(tasks_fut, current_task_fut, all_room_votes_fut);
    let tasks = tasks_result?;

    let votes = if let Some(tid) = room.current_task_id {
        let revealed = room.status == "revealed";
        members
            .iter()
            .map(|m| {
                let v = all_room_votes
                    .iter()
                    .find(|v| v.task_id == tid && v.user_id == m.user_id);
                RoomVoteView {
                    username: m.username.clone(),
                    voted: v.is_some(),
                    value: if revealed {
                        v.and_then(|v| v.value)
                    } else {
                        None
                    },
                }
            })
            .collect()
    } else {
        vec![]
    };

    // Build vote history from pre-fetched votes
    let current_tid = room.current_task_id.unwrap_or(-1);
    let mut seen = std::collections::HashSet::new();
    let voted_tids: Vec<i64> = all_room_votes
        .iter()
        .filter(|v| v.task_id != current_tid)
        .filter_map(|v| {
            if seen.insert(v.task_id) {
                Some(v.task_id)
            } else {
                None
            }
        })
        .collect();

    let mut vote_history = Vec::new();
    if !voted_tids.is_empty() {
        let ph = voted_tids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("SELECT id, title FROM tasks WHERE id IN ({})", ph);
        let mut q = sqlx::query_as::<_, (i64, String)>(&sql);
        for tid in &voted_tids {
            q = q.bind(tid);
        }
        let title_map: std::collections::HashMap<i64, String> =
            q.fetch_all(pool).await?.into_iter().collect();

        for tid in voted_tids {
            let task_votes: Vec<RoomVote> = all_room_votes
                .iter()
                .filter(|v| v.task_id == tid)
                .cloned()
                .collect();
            if task_votes.is_empty() {
                continue;
            }
            let task_title = title_map.get(&tid).cloned().unwrap_or_default();
            let values: Vec<f64> = task_votes.iter().filter_map(|v| v.value).collect();
            let avg = if values.is_empty() {
                0.0
            } else {
                values.iter().sum::<f64>() / values.len() as f64
            };
            let consensus =
                !values.is_empty() && values.iter().all(|v| (*v - values[0]).abs() < 0.01);
            vote_history.push(VoteResult {
                task_id: tid,
                task_title,
                votes: task_votes,
                average: avg,
                consensus,
            });
        }
    }

    Ok(RoomState {
        room,
        members,
        current_task,
        votes,
        tasks,
        vote_history,
    })
}

pub async fn set_room_status(pool: &Pool, room_id: i64, status: &str) -> Result<()> {
    sqlx::query("UPDATE rooms SET status = ?, current_task_id = CASE WHEN ? = 'lobby' THEN NULL ELSE current_task_id END WHERE id = ?")
        .bind(status).bind(status).bind(room_id).execute(pool).await?;
    Ok(())
}

pub async fn accept_estimate(
    pool: &Pool,
    _room_id: i64,
    task_id: i64,
    value: f64,
    unit: &str,
) -> Result<Task> {
    match unit {
        "hours" | "mandays" => {
            let hours = if unit == "mandays" {
                value * 8.0
            } else {
                value
            };
            update_task(
                pool,
                task_id,
                UpdateTaskOpts {
                    estimated_hours: Some(hours),
                    status: Some("estimated"),
                    ..Default::default()
                },
            )
            .await
        }
        _ => {
            update_task(
                pool,
                task_id,
                UpdateTaskOpts {
                    estimated: Some(value as i64),
                    remaining_points: Some(value),
                    status: Some("estimated"),
                    ..Default::default()
                },
            )
            .await
        }
    }
}

pub async fn get_task_votes(pool: &Pool, task_id: i64) -> Result<Vec<RoomVote>> {
    Ok(sqlx::query_as::<_, RoomVote>(&format!(
        "{} WHERE rv.task_id = ? ORDER BY rv.created_at DESC",
        VOTE_SELECT
    ))
    .bind(task_id)
    .fetch_all(pool)
    .await?)
}

// --- Sprint CRUD ---
