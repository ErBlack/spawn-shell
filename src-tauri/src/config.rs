use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub shortcut: String,
    pub font_family: String,
    pub font_size: u32,
    pub height: u32,
    pub command: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            shortcut: "Control+Backquote".to_string(),
            font_family: "ui-monospace".to_string(),
            font_size: 16,
            height: 50,
            command: String::new(),
        }
    }
}

fn config_path() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".config/summon/config.toml"))
}

pub fn load() -> Config {
    let Some(path) = config_path() else {
        return Config::default();
    };
    match fs::read_to_string(&path) {
        Ok(text) => toml::from_str(&text).unwrap_or_default(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let config = Config::default();
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(text) = toml::to_string_pretty(&config) {
                let _ = fs::write(&path, text);
            }
            config
        }
        Err(_) => Config::default(),
    }
}

#[tauri::command]
pub fn get_config(config: tauri::State<'_, Config>) -> Config {
    config.inner().clone()
}
