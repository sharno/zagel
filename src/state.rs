use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::theme::ThemeChoice;

static STATE_FILE_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppState {
    pub active_environment: Option<String>,
    #[serde(default)]
    pub project_roots: Vec<PathBuf>,
    #[serde(default)]
    pub global_env_roots: Vec<PathBuf>,
    #[serde(default)]
    pub http_root: Option<PathBuf>,
    #[serde(default)]
    pub theme: ThemeChoice,
    #[serde(default)]
    pub http_file_order: Vec<PathBuf>,
}

impl AppState {
    pub fn load() -> Self {
        let Some(path) = state_file_path() else {
            return Self::default();
        };

        let mut state = fs::read_to_string(&path).map_or_else(
            |_| Self::default(),
            |raw| toml::from_str(&raw).unwrap_or_default(),
        );

        if state.project_roots.is_empty()
            && let Some(root) = state.http_root.take()
        {
            state.project_roots.push(root);
        }

        state
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

pub fn set_state_file_override(path: PathBuf) -> Result<(), PathBuf> {
    STATE_FILE_OVERRIDE.set(path)
}

fn state_file_path() -> Option<PathBuf> {
    STATE_FILE_OVERRIDE
        .get()
        .cloned()
        .or_else(|| dirs::config_dir().map(|dir| dir.join("zagel").join("state.toml")))
}
