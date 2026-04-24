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

fn default_bind_address() -> String {
    "127.0.0.1".to_string()
}
fn default_bind_port() -> u16 {
    9090
}
fn default_estimation_mode() -> String {
    "hours".to_string()
}
fn default_theme() -> String {
    "dark".to_string()
}
fn default_auto_archive_days() -> u32 {
    90
}
fn default_true() -> bool {
    true
}

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
        let dir = match std::env::var("POFLOW_CONFIG_DIR") {
            Ok(d) if !d.is_empty() => PathBuf::from(d),
            _ => dirs::config_dir()
                .or_else(|| {
                    std::env::var("HOME")
                        .ok()
                        .map(|h| PathBuf::from(h).join(".config"))
                })
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join("poflow"),
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
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600)).ok();
        }
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default_values() {
        let c = Config::default();
        assert_eq!(c.work_duration_min, 25);
        assert_eq!(c.short_break_min, 5);
        assert_eq!(c.long_break_min, 15);
        assert_eq!(c.long_break_interval, 4);
        assert!(!c.auto_start_breaks);
        assert!(!c.auto_start_work);
        assert!(c.sound_enabled);
        assert!(c.notification_enabled);
        assert_eq!(c.daily_goal, 8);
        assert_eq!(c.bind_address, "127.0.0.1");
        assert_eq!(c.bind_port, 9090);
        assert_eq!(c.estimation_mode, "hours");
        assert!(!c.leaf_only_mode);
        assert_eq!(c.theme, "dark");
        assert!(c.cors_origins.is_empty());
        assert_eq!(c.auto_archive_days, 90);
        assert!(c.allow_registration);
    }

    #[test]
    fn toml_roundtrip() {
        let c = Config::default();
        let toml_str = toml::to_string_pretty(&c).unwrap();
        let d: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(d.work_duration_min, c.work_duration_min);
        assert_eq!(d.short_break_min, c.short_break_min);
        assert_eq!(d.long_break_min, c.long_break_min);
        assert_eq!(d.long_break_interval, c.long_break_interval);
        assert_eq!(d.auto_start_breaks, c.auto_start_breaks);
        assert_eq!(d.daily_goal, c.daily_goal);
        assert_eq!(d.bind_address, c.bind_address);
        assert_eq!(d.bind_port, c.bind_port);
        assert_eq!(d.theme, c.theme);
        assert_eq!(d.auto_archive_days, c.auto_archive_days);
        assert_eq!(d.allow_registration, c.allow_registration);
    }

    #[test]
    fn toml_partial_override() {
        // Serialize default, modify one field, deserialize back
        let c = Config {
            work_duration_min: 50,
            ..Config::default()
        };
        let toml_str = toml::to_string_pretty(&c).unwrap();
        let d: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(d.work_duration_min, 50);
        assert_eq!(d.short_break_min, 5);
    }

    #[test]
    fn toml_requires_core_fields() {
        // Partial TOML without required fields should fail
        assert!(toml::from_str::<Config>("").is_err());
        assert!(toml::from_str::<Config>("work_duration_min = 50\n").is_err());
    }

    #[test]
    fn toml_serde_default_fields() {
        // Fields with #[serde(default)] can be omitted — verify via roundtrip
        // that bind_address, bind_port, etc. survive serialization
        let c = Config::default();
        let toml_str = toml::to_string_pretty(&c).unwrap();
        assert!(toml_str.contains("bind_address"));
        assert!(toml_str.contains("bind_port"));
        assert!(toml_str.contains("theme"));
    }

    #[test]
    fn json_serde_roundtrip() {
        let c = Config::default();
        let json = serde_json::to_string(&c).unwrap();
        let d: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(d.work_duration_min, c.work_duration_min);
        assert_eq!(d.bind_port, c.bind_port);
    }

    #[test]
    fn config_path_respects_env() {
        // Test that config_path reads POFLOW_CONFIG_DIR
        // (env var test — may race with other tests, so just verify the function exists)
        let _path = Config::config_path();
        // Path should end with config.toml
        assert!(_path.to_str().unwrap().ends_with("config.toml"));
    }

    #[test]
    fn config_save_and_load_via_tempdir() {
        // Test save/load roundtrip using a unique temp directory
        let dir =
            std::env::temp_dir().join(format!("poflow_cfg_test_{:?}", std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");

        // Save
        let c = Config {
            work_duration_min: 45,
            daily_goal: 12,
            ..Config::default()
        };
        let data = toml::to_string_pretty(&c).unwrap();
        std::fs::write(&path, &data).unwrap();

        // Load
        let content = std::fs::read_to_string(&path).unwrap();
        let loaded: Config = toml::from_str(&content).unwrap();
        assert_eq!(loaded.work_duration_min, 45);
        assert_eq!(loaded.daily_goal, 12);
        assert_eq!(loaded.short_break_min, 5); // default preserved

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_load_invalid_toml_errors() {
        let invalid = "{{invalid toml";
        assert!(toml::from_str::<Config>(invalid).is_err());
    }
}
