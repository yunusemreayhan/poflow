use super::*;


#[utoipa::path(get, path = "/api/config", responses((status = 200, body = crate::config::Config)), security(("bearer" = [])))]
pub async fn get_config(State(engine): State<AppState>, claims: Claims) -> ApiResult<crate::config::Config> {
    Ok(Json(engine.get_user_config(claims.user_id).await))
}

#[utoipa::path(put, path = "/api/config", request_body = crate::config::Config, responses((status = 200, body = crate::config::Config)), security(("bearer" = [])))]
pub async fn update_config(State(engine): State<AppState>, claims: Claims, Json(cfg): Json<crate::config::Config>) -> ApiResult<crate::config::Config> {
    // Save per-user overrides
    let uc = db::UserConfig {
        user_id: claims.user_id,
        work_duration_min: Some(cfg.work_duration_min as i64),
        short_break_min: Some(cfg.short_break_min as i64),
        long_break_min: Some(cfg.long_break_min as i64),
        long_break_interval: Some(cfg.long_break_interval as i64),
        auto_start_breaks: Some(if cfg.auto_start_breaks { 1 } else { 0 }),
        auto_start_work: Some(if cfg.auto_start_work { 1 } else { 0 }),
        daily_goal: Some(cfg.daily_goal as i64),
        theme: None,
        notify_desktop: None,
        notify_sound: None,
    };
    db::set_user_config(&engine.pool, claims.user_id, &uc).await.map_err(internal)?;
    // Root also updates global config
    if claims.role == "root" {
        cfg.save().map_err(internal)?;
        *engine.config.lock().await = cfg.clone();
    }
    Ok(Json(cfg))
}

// --- Profile ---
