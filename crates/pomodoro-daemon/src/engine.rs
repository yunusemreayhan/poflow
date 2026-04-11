use crate::config::Config;
use crate::db::{self, Pool};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, watch};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub enum TimerPhase {
    Idle,
    Work,
    ShortBreak,
    LongBreak,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub enum TimerStatus {
    Idle,
    Running,
    Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct EngineState {
    pub phase: TimerPhase,
    pub status: TimerStatus,
    pub elapsed_s: u32,
    pub duration_s: u32,
    pub session_count: u32,
    pub current_task_id: Option<i64>,
    pub current_session_id: Option<i64>,
    pub current_user_id: i64,
    pub daily_completed: i64,
    pub daily_goal: u32,
}

impl Default for EngineState {
    fn default() -> Self {
        Self {
            phase: TimerPhase::Idle,
            status: TimerStatus::Idle,
            elapsed_s: 0,
            duration_s: 25 * 60,
            session_count: 0,
            current_task_id: None,
            current_session_id: None,
            current_user_id: 0,
            daily_completed: 0,
            daily_goal: 8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeEvent {
    Tasks,
    Sprints,
    Rooms,
    Config,
}

pub struct Engine {
    /// Per-user timer states
    ///
    /// LOCK ORDERING: Always acquire `config` before `states` to prevent deadlocks.
    /// - tick(): config → states
    /// - start(): config → states
    /// - get_state(): states → config (safe: both are short-lived, no nested await)
    pub states: Arc<Mutex<HashMap<i64, EngineState>>>,
    pub config: Arc<Mutex<Config>>,
    pub pool: Pool,
    /// Broadcasts the state of whichever user just changed
    pub tx: watch::Sender<EngineState>,
    pub changes: broadcast::Sender<ChangeEvent>,
    /// Cached per-user configs (user_id → (config, fetched_at))
    user_config_cache: Arc<Mutex<HashMap<i64, (Config, std::time::Instant)>>>,
}

impl Engine {
    pub async fn new(pool: Pool, config: Config) -> Self {
        // Recover any sessions left running from a previous crash
        db::recover_interrupted_sessions(&pool).await.ok();
        let state = EngineState {
            daily_goal: config.daily_goal,
            duration_s: config.work_duration_min * 60,
            ..Default::default()
        };
        let (tx, _) = watch::channel(state);
        let (changes, _) = broadcast::channel(64);
        Self {
            states: Arc::new(Mutex::new(HashMap::new())),
            config: Arc::new(Mutex::new(config)),
            pool,
            tx,
            changes,
            user_config_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn idle_state(user_id: i64, config: &Config) -> EngineState {
        EngineState {
            current_user_id: user_id,
            daily_goal: config.daily_goal,
            duration_s: config.work_duration_min * 60,
            ..Default::default()
        }
    }

    pub async fn get_user_config(&self, user_id: i64) -> Config {
        // Check cache first (60s TTL)
        {
            let cache = self.user_config_cache.lock().await;
            if let Some((cfg, fetched)) = cache.get(&user_id) {
                if fetched.elapsed().as_secs() < 60 {
                    return cfg.clone();
                }
            }
        }
        let mut config = self.config.lock().await.clone();
        if let Ok(Some(uc)) = db::get_user_config(&self.pool, user_id).await {
            if let Some(v) = uc.work_duration_min { config.work_duration_min = v as u32; }
            if let Some(v) = uc.short_break_min { config.short_break_min = v as u32; }
            if let Some(v) = uc.long_break_min { config.long_break_min = v as u32; }
            if let Some(v) = uc.long_break_interval { config.long_break_interval = v as u32; }
            if let Some(v) = uc.auto_start_breaks { config.auto_start_breaks = v != 0; }
            if let Some(v) = uc.auto_start_work { config.auto_start_work = v != 0; }
            if let Some(v) = uc.daily_goal { config.daily_goal = v as u32; }
            if let Some(v) = uc.theme { config.theme = v; }
        }
        self.user_config_cache.lock().await.insert(user_id, (config.clone(), std::time::Instant::now()));
        config
    }

    async fn stop_session(pool: &Pool, state: &mut EngineState, reason: &str) {
        if let Some(sid) = state.current_session_id.take() {
            if let Err(e) = db::end_session(pool, sid, reason).await {
                tracing::warn!("Failed to end session {}: {}", sid, e);
            }
        }
    }

    pub async fn start(&self, user_id: i64, task_id: Option<i64>, phase: Option<TimerPhase>) -> anyhow::Result<EngineState> {
        let config = self.get_user_config(user_id).await;
        let mut states = self.states.lock().await;
        let state = states.entry(user_id).or_insert_with(|| Self::idle_state(user_id, &config));

        // Stop any existing session
        if state.status != TimerStatus::Idle {
            Self::stop_session(&self.pool, state, "interrupted").await;
        }

        let phase = phase.unwrap_or(TimerPhase::Work);
        let duration_s = match phase {
            TimerPhase::Work => config.work_duration_min * 60,
            TimerPhase::ShortBreak => config.short_break_min * 60,
            TimerPhase::LongBreak => config.long_break_min * 60,
            TimerPhase::Idle => return Ok(state.clone()),
        };

        let session_type = match phase {
            TimerPhase::Work => "work",
            TimerPhase::ShortBreak => "short_break",
            TimerPhase::LongBreak => "long_break",
            TimerPhase::Idle => "work",
        };

        let session = db::create_session(&self.pool, user_id, task_id, session_type).await?;
        let daily = db::get_today_completed_for_user(&self.pool, Some(user_id)).await.unwrap_or(0);

        state.phase = phase;
        state.status = TimerStatus::Running;
        state.elapsed_s = 0;
        state.duration_s = duration_s;
        state.current_task_id = task_id;
        state.current_session_id = Some(session.id);
        state.current_user_id = user_id;
        state.daily_completed = daily;
        state.daily_goal = config.daily_goal;

        let s = state.clone();
        self.tx.send(s.clone()).ok();
        Ok(s)
    }

    pub async fn pause(&self, user_id: i64) -> anyhow::Result<EngineState> {
        let mut states = self.states.lock().await;
        let state = match states.get_mut(&user_id) {
            Some(s) => s,
            None => return Ok(Self::idle_state(user_id, &*self.config.lock().await)),
        };
        if state.status == TimerStatus::Running {
            state.status = TimerStatus::Paused;
        }
        let s = state.clone();
        self.tx.send(s.clone()).ok();
        Ok(s)
    }

    pub async fn resume(&self, user_id: i64) -> anyhow::Result<EngineState> {
        let mut states = self.states.lock().await;
        let state = match states.get_mut(&user_id) {
            Some(s) => s,
            None => return Ok(Self::idle_state(user_id, &*self.config.lock().await)),
        };
        if state.status == TimerStatus::Paused {
            state.status = TimerStatus::Running;
        }
        let s = state.clone();
        self.tx.send(s.clone()).ok();
        Ok(s)
    }

    pub async fn stop(&self, user_id: i64) -> anyhow::Result<EngineState> {
        let mut states = self.states.lock().await;
        let state = match states.get_mut(&user_id) {
            Some(s) => s,
            None => return Ok(Self::idle_state(user_id, &*self.config.lock().await)),
        };
        Self::stop_session(&self.pool, state, "interrupted").await;
        let preserved = (state.session_count, state.daily_completed, state.daily_goal, state.duration_s);
        *state = EngineState {
            current_user_id: user_id,
            session_count: preserved.0,
            daily_completed: preserved.1,
            daily_goal: preserved.2,
            duration_s: preserved.3,
            ..Default::default()
        };
        let s = state.clone();
        self.tx.send(s.clone()).ok();
        Ok(s)
    }

    pub async fn skip(&self, user_id: i64) -> anyhow::Result<EngineState> {
        let config = self.get_user_config(user_id).await;
        let mut states = self.states.lock().await;
        let state = states.entry(user_id).or_insert_with(|| Self::idle_state(user_id, &config));
        // End current session
        if let Some(sid) = state.current_session_id.take() {
            if let Err(e) = db::end_session(&self.pool, sid, "skipped").await {
                tracing::warn!("Failed to end session {}: {}", sid, e);
            }
        }
        // Advance to next phase
        let was_work = state.phase == TimerPhase::Work;
        if was_work {
            state.session_count += 1;
        }
        let next_phase = match state.phase {
            TimerPhase::Work => if state.session_count % config.long_break_interval == 0 { TimerPhase::LongBreak } else { TimerPhase::ShortBreak },
            _ => TimerPhase::Work,
        };
        state.phase = next_phase;
        state.elapsed_s = 0;
        state.duration_s = match next_phase {
            TimerPhase::Work => config.work_duration_min * 60,
            TimerPhase::ShortBreak => config.short_break_min * 60,
            TimerPhase::LongBreak => config.long_break_min * 60,
            TimerPhase::Idle => 0,
        };
        state.status = TimerStatus::Idle;
        let s = state.clone();
        self.tx.send(s.clone()).ok();
        Ok(s)
    }

    /// Tick all active user timers. Returns completed states for notification.
    pub async fn tick(&self) -> anyhow::Result<Vec<EngineState>> {
        // Two-phase tick: lock states briefly to advance timers, then release lock for DB I/O.
        // Lock duration is O(active_running_users) which is acceptable for typical deployments.
        struct Completion {
            session_id: Option<i64>,
            was_work: bool,
            task_id: Option<i64>,
            user_id: i64,
            duration_s: u32,
            auto_start: bool,
            next_session_type: &'static str,
        }

        let completions: Vec<Completion>;
        let completed_states: Vec<EngineState>;
        {
            let global_config = self.config.lock().await.clone();
            let mut states = self.states.lock().await;
            let mut comps = Vec::new();

            // Pre-load per-user configs for active users (uses cache)
            let user_ids: Vec<i64> = states.iter()
                .filter(|(_, s)| s.status == TimerStatus::Running)
                .map(|(uid, _)| *uid)
                .collect();
            // Release states lock briefly to fetch configs, then re-acquire
            drop(states);
            let mut user_configs = std::collections::HashMap::new();
            for uid in &user_ids {
                user_configs.insert(*uid, self.get_user_config(*uid).await);
            }
            states = self.states.lock().await;

            for uid in &user_ids {
                let config = user_configs.get(uid).unwrap_or(&global_config);
                let state = match states.get_mut(uid) {
                    Some(s) if s.status == TimerStatus::Running => s,
                    _ => continue,
                };

                state.elapsed_s += 1;
                if state.elapsed_s < state.duration_s {
                    continue;
                }

                // Session completed — update in-memory state
                let was_work = state.phase == TimerPhase::Work;
                let completed_duration_s = state.duration_s; // capture before overwrite
                let old_session_id = state.current_session_id.take();
                if was_work {
                    state.session_count += 1;
                    state.daily_completed += 1;
                }

                let next_phase = if was_work {
                    if state.session_count % config.long_break_interval == 0 { TimerPhase::LongBreak } else { TimerPhase::ShortBreak }
                } else {
                    TimerPhase::Work
                };
                let auto_start = if was_work { config.auto_start_breaks } else { config.auto_start_work };

                state.phase = next_phase;
                state.elapsed_s = 0;
                state.duration_s = match next_phase {
                    TimerPhase::Work => config.work_duration_min * 60,
                    TimerPhase::ShortBreak => config.short_break_min * 60,
                    TimerPhase::LongBreak => config.long_break_min * 60,
                    TimerPhase::Idle => 0,
                };
                state.status = if auto_start { TimerStatus::Running } else { TimerStatus::Idle };

                let next_session_type = match next_phase {
                    TimerPhase::Work => "work", TimerPhase::ShortBreak => "short_break",
                    TimerPhase::LongBreak => "long_break", TimerPhase::Idle => "work",
                };

                comps.push(Completion {
                    session_id: old_session_id,
                    was_work,
                    task_id: state.current_task_id,
                    user_id: state.current_user_id,
                    duration_s: completed_duration_s,
                    auto_start,
                    next_session_type,
                });
            }

            // Collect completed states while still holding lock
            completed_states = comps.iter().filter_map(|c| states.get(&c.user_id).cloned()).collect();
            completions = comps;

            // Broadcast running states
            if completed_states.is_empty() {
                if let Some(s) = states.values().find(|s| s.status == TimerStatus::Running) {
                    self.tx.send(s.clone()).ok();
                }
            }
        } // Lock released here

        // Phase 2: DB work without holding the lock
        for (i, c) in completions.iter().enumerate() {
            if let Some(sid) = c.session_id {
                if let Err(e) = db::end_session(&self.pool, sid, "completed").await { tracing::warn!("Failed to end session {}: {}", sid, e); };
            }
            if c.was_work {
                if let Some(tid) = c.task_id {
                    db::increment_task_actual(&self.pool, tid).await
                        .map_err(|e| tracing::warn!("Failed to increment actual: {}", e)).ok();
                    let hours = c.duration_s as f64 / 3600.0;
                    let sprint_id = db::find_task_active_sprint(&self.pool, tid).await.unwrap_or(None);
                    db::log_burn(&self.pool, sprint_id, tid, c.session_id, c.user_id, 0.0, hours, "timer", None).await
                        .map_err(|e| tracing::warn!("Failed to log burn: {}", e)).ok();
                }
            }
            if c.auto_start {
                if let Ok(session) = db::create_session(&self.pool, c.user_id, c.task_id, c.next_session_type).await {
                    // Re-acquire lock briefly to set session ID
                    let mut states = self.states.lock().await;
                    if let Some(state) = states.get_mut(&c.user_id) {
                        state.current_session_id = Some(session.id);
                    }
                }
            }
            self.tx.send(completed_states[i].clone()).ok();
        }

        Ok(completed_states)
    }

    pub async fn get_state(&self, user_id: i64) -> EngineState {
        let config = self.config.lock().await.clone();
        let states = self.states.lock().await;
        let mut state = states.get(&user_id).cloned().unwrap_or_else(|| Self::idle_state(user_id, &config));
        drop(states);
        // Always refresh daily_completed from DB for accuracy after restart
        state.daily_completed = db::get_today_completed_for_user(&self.pool, Some(user_id)).await.unwrap_or(state.daily_completed);
        state
    }

    pub async fn is_task_active(&self, task_id: i64) -> bool {
        let states = self.states.lock().await;
        states.values().any(|s| s.current_task_id == Some(task_id) && s.status != TimerStatus::Idle)
    }

    pub async fn update_config(&self, config: Config) -> anyhow::Result<()> {
        config.save()?;
        *self.config.lock().await = config;
        self.user_config_cache.lock().await.clear();
        Ok(())
    }

    pub async fn invalidate_user_config_cache(&self, user_id: i64) {
        self.user_config_cache.lock().await.remove(&user_id);
    }

    pub async fn get_config(&self) -> Config {
        self.config.lock().await.clone()
    }

    pub fn notify(&self, event: ChangeEvent) {
        let _ = self.changes.send(event);
    }
}
