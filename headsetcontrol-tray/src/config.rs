use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub discharge_level: Option<u8>, // 10, 20, ..., 90 or None (Disable)
    pub charge_level: Option<u8>,    // 10, 20, ..., 100 or None (Disable)
    pub sidetone_level: u8,
    pub inactive_time: u8,
    pub reapply_on_startup: bool,
    pub notifications_enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            discharge_level: Some(20),
            charge_level: Some(80),
            sidetone_level: 64,
            inactive_time: 0,
            reapply_on_startup: true,
            notifications_enabled: true,
        }
    }
}

pub fn get_config_path() -> PathBuf {
    let mut path = dirs_next::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    path.push(".config");
    path.push("headsetcontrol-widget");
    path.push("config.json");
    path
}

pub fn load_config() -> Config {
    let path = get_config_path();
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(config) = serde_json::from_str(&content) {
            return config;
        }
    }
    Config::default()
}

pub fn save_config(config: &Config) -> Result<(), String> {
    let path = get_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())?;
    Ok(())
}
