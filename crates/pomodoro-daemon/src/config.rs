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
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub cors_origins: Vec<String>,
    #[serde(default = "default_auto_archive_days")]
    pub auto_archive_days: u32,
    #[serde(default = "default_true")]
    pub allow_registration: bool,
}

fn default_bind_address() -> String { "127.0.0.1".to_string() }
fn default_bind_port() -> u16 { 9090 }
fn default_estimation_mode() -> String { "hours".to_string() }
fn default_theme() -> String { "dark".to_string() }
fn default_auto_archive_days() -> u32 { 90 }
fn default_true() -> bool { true }

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
            bind_address: "127.0.0.1".to_string(),
            bind_port: 9090,
            estimation_mode: "hours".to_string(),
            leaf_only_mode: false,
            theme: "dark".to_string(),
            cors_origins: vec![],
            auto_archive_days: 90,
            allow_registration: true,
        }
    }
}

impl Config {
    pub fn config_path() -> PathBuf {
        let dir = match std::env::var("POMODORO_CONFIG_DIR") {
            Ok(d) if !d.is_empty() => PathBuf::from(d),
            _ => dirs::config_dir()
                .or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config")))
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join("pomodoro"),
        };
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
        let tmp = path.with_extension("toml.tmp");
        let data = toml::to_string_pretty(self)?;
        {
            let f = std::fs::File::create(&tmp)?;
            std::io::Write::write_all(&mut &f, data.as_bytes())?;
            f.sync_all()?;
        }
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600)).ok();
        }
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }
}
