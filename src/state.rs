use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppState {
    pub active_environment: Option<String>,
    pub http_root: Option<PathBuf>,
}

impl AppState {
    pub fn load() -> Self {
        let Some(path) = state_file_path() else {
            return Self::default();
        };

        fs::read_to_string(&path).map_or_else(
            |_| Self::default(),
            |raw| toml::from_str(&raw).unwrap_or_default(),
        )
    }

    pub fn save(&self) {
        let Some(path) = state_file_path() else {
            return;
        };

        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }

        if let Ok(raw) = toml::to_string(self) {
            let _ = fs::write(path, raw);
        }
    }
}

fn state_file_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("zagel").join("state.toml"))
}
