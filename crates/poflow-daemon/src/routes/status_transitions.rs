use super::{err, internal, ApiResult, AppState};
use crate::auth::{self, Claims};
use crate::db;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct TransitionQuery {
    pub project_id: Option<i64>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateTransitionRequest {
    pub from_status: String,
    pub to_status: String,
    pub project_id: Option<i64>,
}

#[utoipa::path(get, path = "/api/workflows/transitions", params(("project_id" = Option<i64>, Query, description = "Filter by project")), responses((status = 200, body = Vec<db::StatusTransition>)), security(("bearer" = [])))]
pub async fn list_transitions(State(engine): State<AppState>, _claims: Claims, Query(q): Query<TransitionQuery>) -> ApiResult<Vec<db::StatusTransition>> {
    db::list_status_transitions(&engine.pool, q.project_id).await.map(Json).map_err(internal)
}

#[utoipa::path(post, path = "/api/workflows/transitions", request_body = CreateTransitionRequest, responses((status = 201, body = db::StatusTransition)), security(("bearer" = [])))]
pub async fn create_transition(State(engine): State<AppState>, claims: Claims, Json(req): Json<CreateTransitionRequest>) -> Result<(StatusCode, Json<db::StatusTransition>), super::ApiError> {
    if !auth::is_admin_or_root(&claims) { return Err(err(StatusCode::FORBIDDEN, "Admin or root required")); }
    if req.from_status.trim().is_empty() || req.to_status.trim().is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "from_status and to_status are required"));
    }
    if req.from_status == req.to_status {
        return Err(err(StatusCode::BAD_REQUEST, "from_status and to_status must differ"));
    }
    let t = db::create_status_transition(&engine.pool, req.from_status.trim(), req.to_status.trim(), req.project_id)
        .await.map_err(|e| {
            if e.to_string().contains("UNIQUE") { err(StatusCode::CONFLICT, "Transition already exists") }
            else { internal(e) }
        })?;
    Ok((StatusCode::CREATED, Json(t)))
}

#[utoipa::path(delete, path = "/api/workflows/transitions/{id}", responses((status = 204)), security(("bearer" = [])))]
pub async fn delete_transition(State(engine): State<AppState>, claims: Claims, Path(id): Path<i64>) -> Result<StatusCode, super::ApiError> {
    if !auth::is_admin_or_root(&claims) { return Err(err(StatusCode::FORBIDDEN, "Admin or root required")); }
    db::delete_status_transition(&engine.pool, id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}
