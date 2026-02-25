use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct LaunchOptions {
    pub state_file: Option<PathBuf>,
    pub project_roots: Vec<PathBuf>,
    pub global_env_roots: Vec<PathBuf>,
    pub automation: Option<AutomationOptions>,
}

#[derive(Debug, Clone)]
pub struct AutomationOptions {
    pub scenario_path: PathBuf,
    pub screenshot_dir: PathBuf,
    pub state_output_path: Option<PathBuf>,
    pub exit_when_done: bool,
}
