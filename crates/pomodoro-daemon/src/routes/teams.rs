use super::*;

#[utoipa::path(get, path = "/api/teams", responses((status = 200, body = Vec<db::Team>)), security(("bearer" = [])))]
pub async fn list_teams(State(engine): State<AppState>, _claims: Claims) -> ApiResult<Vec<db::Team>> {
    db::list_teams(&engine.pool).await.map(Json).map_err(internal)
}



#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateTeamRequest { pub name: String }

#[utoipa::path(post, path = "/api/teams", responses((status = 201, body = db::Team)), security(("bearer" = [])))]
pub async fn create_team(State(engine): State<AppState>, claims: Claims, Json(req): Json<CreateTeamRequest>) -> Result<(StatusCode, Json<db::Team>), ApiError> {
    if req.name.trim().is_empty() { return Err(err(StatusCode::BAD_REQUEST, "Team name cannot be empty")); }
    if req.name.len() > 100 { return Err(err(StatusCode::BAD_REQUEST, "Team name too long (max 100 chars)")); }
    let team = db::create_team(&engine.pool, req.name.trim()).await.map_err(internal)?;
    db::add_team_member(&engine.pool, team.id, claims.user_id, "admin").await.map_err(internal)?;
    Ok((StatusCode::CREATED, Json(team)))
}

#[utoipa::path(get, path = "/api/teams/{id}", responses((status = 200, body = db::TeamDetail)), security(("bearer" = [])))]
pub async fn get_team(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<db::TeamDetail> {
    db::get_team_detail(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(delete, path = "/api/teams/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_team(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, ApiError> {
    if claims.role != "root" { return Err(err(StatusCode::FORBIDDEN, "Only root can delete teams")); }
    db::delete_team(&engine.pool, id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}



#[derive(Deserialize, utoipa::ToSchema)]
pub struct TeamMemberRequest { pub user_id: i64, #[serde(default = "default_member_role")] pub role: String }
fn default_member_role() -> String { "member".to_string() }

#[utoipa::path(post, path = "/api/teams/{id}/members", responses((status = 204)), security(("bearer" = [])))]
pub async fn add_team_member(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<TeamMemberRequest>) -> Result<StatusCode, ApiError> {
    if !db::is_team_admin(&engine.pool, id, claims.user_id).await.map_err(internal)? && claims.role != "root" {
        return Err(err(StatusCode::FORBIDDEN, "Team admin only"));
    }
    if !["admin", "member"].contains(&req.role.as_str()) {
        return Err(err(StatusCode::BAD_REQUEST, "Role must be 'admin' or 'member'"));
    }
    db::add_team_member(&engine.pool, id, req.user_id, &req.role).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/api/teams/{id}/members/{user_id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn remove_team_member(State(engine): State<AppState>, claims: Claims, Path((id, user_id)): Path<(i64, i64)>) -> Result<StatusCode, ApiError> {
    if !db::is_team_admin(&engine.pool, id, claims.user_id).await.map_err(internal)? && claims.role != "root" {
        return Err(err(StatusCode::FORBIDDEN, "Team admin only"));
    }
    db::remove_team_member(&engine.pool, id, user_id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/me/teams", responses((status = 200, body = Vec<db::Team>)), security(("bearer" = [])))]
pub async fn get_my_teams(State(engine): State<AppState>, claims: Claims) -> ApiResult<Vec<db::Team>> {
    db::get_user_teams(&engine.pool, claims.user_id).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/teams/{id}/roots", responses((status = 204)), security(("bearer" = [])))]
pub async fn add_team_root_tasks(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>, Json(req): Json<EpicGroupTasksRequest>) -> Result<StatusCode, ApiError> {
    if !db::is_team_admin(&engine.pool, id, claims.user_id).await.map_err(internal)? && claims.role != "root" {
        return Err(err(StatusCode::FORBIDDEN, "Team admin only"));
    }
    for tid in req.task_ids {
        db::get_task(&engine.pool, tid).await.map_err(|_| err(StatusCode::NOT_FOUND, &format!("Task {} not found", tid)))?;
        db::add_team_root_task(&engine.pool, id, tid).await.map_err(internal)?;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/api/teams/{id}/roots/{task_id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn remove_team_root_task(State(engine): State<AppState>, claims: Claims, Path((id, task_id)): Path<(i64, i64)>) -> Result<StatusCode, ApiError> {
    if !db::is_team_admin(&engine.pool, id, claims.user_id).await.map_err(internal)? && claims.role != "root" {
        return Err(err(StatusCode::FORBIDDEN, "Team admin only"));
    }
    db::remove_team_root_task(&engine.pool, id, task_id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/teams/{id}/scope", responses((status = 200, body = Vec<i64>)), security(("bearer" = [])))]
pub async fn get_team_scope(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<Vec<i64>> {
    let detail = db::get_team_detail(&engine.pool, id).await.map_err(internal)?;
    if detail.root_task_ids.is_empty() { return Ok(Json(vec![])); }
    db::get_descendant_ids(&engine.pool, &detail.root_task_ids).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/sprints/{id}/snapshot", responses((status = 200, body = db::SprintDailyStat)), security(("bearer" = [])))]
pub async fn snapshot_sprint(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> ApiResult<db::SprintDailyStat> {
    let sprint = db::get_sprint(&engine.pool, id).await.map_err(|_| err(StatusCode::NOT_FOUND, "Sprint not found"))?;
    if !is_owner_or_root(sprint.created_by_id, &claims) { return Err(err(StatusCode::FORBIDDEN, "Not owner")); }
    db::snapshot_sprint(&engine.pool, id).await.map(Json).map_err(internal)
}

#[utoipa::path(get, path = "/api/sprints/{id}/board", responses((status = 200, body = db::SprintBoard)), security(("bearer" = [])))]
pub async fn get_sprint_board(State(engine): State<AppState>, _claims: Claims, Path(id): Path<i64>) -> ApiResult<db::SprintBoard> {
    db::get_sprint_board(&engine.pool, id).await.map(Json).map_err(internal)
}

// --- Burn log ---
