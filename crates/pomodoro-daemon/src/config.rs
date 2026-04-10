use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Config {
    pub work_duration_min: u32,
    pub short_break_min: u32,
    pub long_break_min: u32,
    pub long_break_interval: u32,
    pub auto_start_breaks: bool,
    pub auto_start_work: bool,
    pub sound_enabled: bool,
    pub notification_enabled: bool,
    pub daily_goal: u32,
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    #[serde(default = "default_bind_port")]
    pub bind_port: u16,
    #[serde(default = "default_estimation_mode")]
    pub estimation_mode: String,
    #[serde(default)]
    pub leaf_only_mode: bool,
}

fn default_bind_address() -> String { "0.0.0.0".to_string() }
fn default_bind_port() -> u16 { 9090 }
fn default_estimation_mode() -> String { "hours".to_string() }

impl Default for Config {
    fn default() -> Self {
        Self {
            work_duration_min: 25,
            short_break_min: 5,
            long_break_min: 15,
            long_break_interval: 4,
            auto_start_breaks: false,
            auto_start_work: false,
            sound_enabled: true,
            notification_enabled: true,
            daily_goal: 8,
            bind_address: "0.0.0.0".to_string(),
            bind_port: 9090,
            estimation_mode: "hours".to_string(),
            leaf_only_mode: false,
        }
    }
}

impl Config {
    pub fn config_path() -> PathBuf {
        let dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("pomodoro");
        std::fs::create_dir_all(&dir).ok();
        dir.join("config.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(toml::from_str(&content)?)
        } else {
            let cfg = Self::default();
            cfg.save()?;
            Ok(cfg)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        std::fs::write(&path, toml::to_string_pretty(self)?)?;
        Ok(())
    }
}
