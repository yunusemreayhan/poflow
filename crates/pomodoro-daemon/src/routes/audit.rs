use super::*;

#[derive(Deserialize)]
pub struct AuditQuery {
    pub entity_type: Option<String>,
    pub entity_id: Option<i64>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[utoipa::path(get, path = "/api/audit", responses((status = 200)), security(("bearer" = [])))]
pub async fn list_audit(State(engine): State<AppState>, _claims: Claims, Query(q): Query<AuditQuery>) -> ApiResult<Vec<db::AuditEntry>> {
    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(100).min(500);
    let offset = (page - 1) * per_page;
    db::list_audit(&engine.pool, q.entity_type.as_deref(), q.entity_id, per_page, offset).await.map(Json).map_err(internal)
}
