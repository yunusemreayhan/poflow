use super::*;


#[utoipa::path(get, path = "/api/config", responses((status = 200, body = crate::config::Config)), security(("bearer" = [])))]
pub async fn get_config(State(engine): State<AppState>, claims: Claims) -> ApiResult<crate::config::Config> {
    Ok(Json(engine.get_user_config(claims.user_id).await))
}

#[utoipa::path(put, path = "/api/config", request_body = crate::config::Config, responses((status = 200, body = crate::config::Config)), security(("bearer" = [])))]
pub async fn update_config(State(engine): State<AppState>, claims: Claims, Json(cfg): Json<crate::config::Config>) -> ApiResult<crate::config::Config> {
    // V1: Validate config bounds
    if cfg.work_duration_min == 0 || cfg.work_duration_min > 240 { return Err(err(StatusCode::BAD_REQUEST, "work_duration_min must be 1-240")); }
    if cfg.short_break_min == 0 || cfg.short_break_min > 60 { return Err(err(StatusCode::BAD_REQUEST, "short_break_min must be 1-60")); }
    if cfg.long_break_min == 0 || cfg.long_break_min > 120 { return Err(err(StatusCode::BAD_REQUEST, "long_break_min must be 1-120")); }
    if cfg.long_break_interval == 0 || cfg.long_break_interval > 20 { return Err(err(StatusCode::BAD_REQUEST, "long_break_interval must be 1-20")); }
    if cfg.daily_goal > 50 { return Err(err(StatusCode::BAD_REQUEST, "daily_goal must be 0-50")); }
    if !["hours", "points"].contains(&cfg.estimation_mode.as_str()) { return Err(err(StatusCode::BAD_REQUEST, "estimation_mode must be 'hours' or 'points'")); }
    if !["dark", "light"].contains(&cfg.theme.as_str()) { return Err(err(StatusCode::BAD_REQUEST, "theme must be 'dark' or 'light'")); }
    // BL2/V1: Validate auto_archive_days (0 = disabled, 1-3650 = active)
    if cfg.auto_archive_days > 3650 { return Err(err(StatusCode::BAD_REQUEST, "auto_archive_days must be 0-3650")); }
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
        theme: Some(cfg.theme.clone()),
        notify_desktop: Some(if cfg.notification_enabled { 1 } else { 0 }),
        notify_sound: Some(if cfg.sound_enabled { 1 } else { 0 }),
    };
    db::set_user_config(&engine.pool, claims.user_id, &uc).await.map_err(internal)?;
    engine.invalidate_user_config_cache(claims.user_id).await;
    // Root also updates global config (preserve network settings from current config)
    if claims.role == "root" {
        let mut save_cfg = cfg.clone();
        let mut current = engine.config.lock().await;
        save_cfg.bind_address = current.bind_address.clone();
        save_cfg.bind_port = current.bind_port;
        save_cfg.cors_origins = current.cors_origins.clone();
        save_cfg.save().map_err(internal)?;
        *current = save_cfg;
    }
    Ok(Json(cfg))
}

// --- Profile ---
