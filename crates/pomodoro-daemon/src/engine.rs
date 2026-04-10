use crate::config::Config;
use crate::db::{self, Pool};
use serde::{Deserialize, Serialize};
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
    pub state: Arc<Mutex<EngineState>>,
    pub config: Arc<Mutex<Config>>,
    pub pool: Pool,
    pub tx: watch::Sender<EngineState>,
    pub changes: broadcast::Sender<ChangeEvent>,
}

impl Engine {
    pub async fn new(pool: Pool, config: Config) -> Self {
        let daily = db::get_today_completed(&pool).await.unwrap_or(0);
        let state = EngineState {
            daily_completed: daily,
            daily_goal: config.daily_goal,
            duration_s: config.work_duration_min * 60,
            ..Default::default()
        };
        let (tx, _) = watch::channel(state.clone());
        let (changes, _) = broadcast::channel(64);
        Self {
            state: Arc::new(Mutex::new(state)),
            config: Arc::new(Mutex::new(config)),
            pool,
            tx,
            changes,
        }
    }

    /// Stop any currently running session before starting a new one
    async fn stop_current_session(&self, state: &mut EngineState, reason: &str) {
        if let Some(sid) = state.current_session_id.take() {
            db::end_session(&self.pool, sid, reason).await.ok();
        }
    }

    pub async fn start(&self, user_id: i64, task_id: Option<i64>, phase: Option<TimerPhase>) -> anyhow::Result<EngineState> {
        let mut state = self.state.lock().await;
        let mut config = self.config.lock().await.clone();

        // Overlay per-user config
        if let Ok(Some(uc)) = db::get_user_config(&self.pool, user_id).await {
            if let Some(v) = uc.work_duration_min { config.work_duration_min = v as u32; }
            if let Some(v) = uc.short_break_min { config.short_break_min = v as u32; }
            if let Some(v) = uc.long_break_min { config.long_break_min = v as u32; }
            if let Some(v) = uc.long_break_interval { config.long_break_interval = v as u32; }
        }

        // Stop any existing session first
        if state.status != TimerStatus::Idle {
            self.stop_current_session(&mut state, "interrupted").await;
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

        state.phase = phase;
        state.status = TimerStatus::Running;
        state.elapsed_s = 0;
        state.duration_s = duration_s;
        state.current_task_id = task_id;
        state.current_session_id = Some(session.id);
        state.current_user_id = user_id;

        let s = state.clone();
        self.tx.send(s.clone()).ok();
        Ok(s)
    }

    pub async fn pause(&self) -> anyhow::Result<EngineState> {
        let mut state = self.state.lock().await;
        if state.status == TimerStatus::Running {
            state.status = TimerStatus::Paused;
        }
        let s = state.clone();
        self.tx.send(s.clone()).ok();
        Ok(s)
    }

    pub async fn resume(&self) -> anyhow::Result<EngineState> {
        let mut state = self.state.lock().await;
        if state.status == TimerStatus::Paused {
            state.status = TimerStatus::Running;
        }
        let s = state.clone();
        self.tx.send(s.clone()).ok();
        Ok(s)
    }

    pub async fn stop(&self) -> anyhow::Result<EngineState> {
        let mut state = self.state.lock().await;
        self.stop_current_session(&mut state, "interrupted").await;
        *state = EngineState {
            session_count: state.session_count,
            daily_completed: state.daily_completed,
            daily_goal: state.daily_goal,
            duration_s: state.duration_s,
            ..Default::default()
        };
        let s = state.clone();
        self.tx.send(s.clone()).ok();
        Ok(s)
    }

    pub async fn skip(&self) -> anyhow::Result<EngineState> {
        let mut state = self.state.lock().await;
        self.stop_current_session(&mut state, "skipped").await;
        *state = EngineState {
            session_count: state.session_count,
            daily_completed: state.daily_completed,
            daily_goal: state.daily_goal,
            duration_s: state.duration_s,
            ..Default::default()
        };
        let s = state.clone();
        self.tx.send(s.clone()).ok();
        Ok(s)
    }

    pub async fn tick(&self) -> anyhow::Result<Option<EngineState>> {
        let mut state = self.state.lock().await;
        if state.status != TimerStatus::Running {
            return Ok(None);
        }

        state.elapsed_s += 1;

        if state.elapsed_s >= state.duration_s {
            // Session completed
            let completed_session_id = state.current_session_id;
            if let Some(sid) = completed_session_id {
                db::end_session(&self.pool, sid, "completed").await?;
            }

            let was_work = state.phase == TimerPhase::Work;
            if was_work {
                state.session_count += 1;
                state.daily_completed += 1;

                if let Some(tid) = state.current_task_id {
                    db::increment_task_actual(&self.pool, tid).await.ok();
                    let hours = state.duration_s as f64 / 3600.0;
                    let sprint_id = db::find_task_active_sprint(&self.pool, tid).await.unwrap_or(None);
                    db::log_burn(&self.pool, sprint_id, tid, completed_session_id, state.current_user_id, 0.0, hours, "timer", None).await.ok();
                }
            }

            let config = self.config.lock().await.clone();
            let next_phase = if was_work {
                if state.session_count % config.long_break_interval == 0 {
                    TimerPhase::LongBreak
                } else {
                    TimerPhase::ShortBreak
                }
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
            state.current_session_id = None;

            if auto_start {
                let session_type = match next_phase {
                    TimerPhase::Work => "work",
                    TimerPhase::ShortBreak => "short_break",
                    TimerPhase::LongBreak => "long_break",
                    TimerPhase::Idle => "work",
                };
                let session = db::create_session(&self.pool, state.current_user_id, state.current_task_id, session_type).await?;
                state.current_session_id = Some(session.id);
                state.status = TimerStatus::Running;
            } else {
                state.status = TimerStatus::Idle;
            }

            let s = state.clone();
            self.tx.send(s.clone()).ok();
            return Ok(Some(s));
        }

        let s = state.clone();
        self.tx.send(s.clone()).ok();
        Ok(None)
    }

    pub async fn get_state(&self) -> EngineState {
        self.state.lock().await.clone()
    }

    /// Check if a task is currently being focused on
    pub async fn is_task_active(&self, task_id: i64) -> bool {
        let state = self.state.lock().await;
        state.current_task_id == Some(task_id) && state.status != TimerStatus::Idle
    }

    pub async fn update_config(&self, config: Config) -> anyhow::Result<()> {
        config.save()?;
        *self.config.lock().await = config;
        Ok(())
    }

    pub async fn get_config(&self) -> Config {
        self.config.lock().await.clone()
    }

    pub fn notify(&self, event: ChangeEvent) {
        let _ = self.changes.send(event);
    }
}
