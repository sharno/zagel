use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::mem;
use std::path::{Path, PathBuf};

use crate::model::{Environment, HttpFile, RequestDraft, RequestId};
use crate::pathing::{GlobalEnvRoot, ProjectRoot, SavePathError};
use vec1::Vec1;

fn vec1_from_vec<T>(values: Vec<T>) -> Option<Vec1<T>> {
    let mut iter = values.into_iter();
    let first = iter.next()?;
    let mut non_empty = Vec1::new(first);
    for value in iter {
        non_empty.push(value);
    }
    Some(non_empty)
}

#[derive(Debug, Clone)]
pub enum ProjectConfiguration {
    Unconfigured {
        global_env_roots: Vec<GlobalEnvRoot>,
    },
    Configured {
        project_roots: Vec1<ProjectRoot>,
        global_env_roots: Vec<GlobalEnvRoot>,
    },
}

impl ProjectConfiguration {
    pub fn from_loaded(
        project_roots: Vec<ProjectRoot>,
        global_env_roots: Vec<GlobalEnvRoot>,
    ) -> Self {
        match vec1_from_vec(project_roots) {
            Some(project_roots) => Self::Configured {
                project_roots,
                global_env_roots,
            },
            None => Self::Unconfigured { global_env_roots },
        }
    }

    pub const fn should_scan(&self) -> bool {
        matches!(self, Self::Configured { .. })
    }

    pub fn project_roots(&self) -> &[ProjectRoot] {
        match self {
            Self::Unconfigured { .. } => &[],
            Self::Configured { project_roots, .. } => project_roots.as_ref(),
        }
    }

    pub fn global_env_roots(&self) -> &[GlobalEnvRoot] {
        match self {
            Self::Unconfigured { global_env_roots }
            | Self::Configured {
                global_env_roots, ..
            } => global_env_roots.as_slice(),
        }
    }

    pub fn project_root_paths(&self) -> Vec<PathBuf> {
        self.project_roots()
            .iter()
            .map(ProjectRoot::to_path_buf)
            .collect()
    }

    pub fn global_env_root_paths(&self) -> Vec<PathBuf> {
        self.global_env_roots()
            .iter()
            .map(GlobalEnvRoot::to_path_buf)
            .collect()
    }

    pub fn watch_root_paths(&self) -> Vec<PathBuf> {
        if !self.should_scan() {
            return Vec::new();
        }
        let mut roots = self.project_root_paths();
        roots.extend(self.global_env_root_paths());
        roots.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
        roots.dedup();
        roots
    }

    pub fn default_project_root(&self) -> Option<&ProjectRoot> {
        match self {
            Self::Unconfigured { .. } => None,
            Self::Configured { project_roots, .. } => Some(project_roots.first()),
        }
    }

    pub fn project_root_for_path(&self, path: &Path) -> Option<&ProjectRoot> {
        self.project_roots()
            .iter()
            .filter(|root| path.starts_with(root.as_path()))
            .max_by_key(|root| root.as_path().components().count())
    }

    pub fn add_project(&mut self, root: ProjectRoot) -> Result<ProjectChangeOutcome, RootOpError> {
        match self {
            Self::Unconfigured { global_env_roots } => {
                let globals = mem::take(global_env_roots);
                *self = Self::Configured {
                    project_roots: Vec1::new(root),
                    global_env_roots: globals,
                };
                Ok(ProjectChangeOutcome::AddedAndScan)
            }
            Self::Configured { project_roots, .. } => {
                if project_roots.contains(&root) {
                    return Err(RootOpError::ProjectAlreadyExists);
                }
                project_roots.push(root);
                Ok(ProjectChangeOutcome::AddedAndScan)
            }
        }
    }

    pub fn remove_project(
        &mut self,
        root: &ProjectRoot,
    ) -> Result<ProjectChangeOutcome, RootOpError> {
        match self {
            Self::Unconfigured { .. } => Err(RootOpError::ProjectMissing),
            Self::Configured {
                project_roots,
                global_env_roots,
            } => {
                let mut remaining = project_roots.iter().cloned().collect::<Vec<_>>();
                let before_len = remaining.len();
                remaining.retain(|candidate| candidate != root);
                if remaining.len() == before_len {
                    return Err(RootOpError::ProjectMissing);
                }
                if remaining.is_empty() {
                    let globals = mem::take(global_env_roots);
                    *self = Self::Unconfigured {
                        global_env_roots: globals,
                    };
                    Ok(ProjectChangeOutcome::RemovedLastProject)
                } else {
                    *project_roots = vec1_from_vec(remaining).expect("non-empty");
                    Ok(ProjectChangeOutcome::RemovedAndScan)
                }
            }
        }
    }

    pub fn add_global_env(
        &mut self,
        root: GlobalEnvRoot,
    ) -> Result<GlobalEnvChangeOutcome, RootOpError> {
        let roots = match self {
            Self::Unconfigured { global_env_roots }
            | Self::Configured {
                global_env_roots, ..
            } => global_env_roots,
        };
        if roots.contains(&root) {
            return Err(RootOpError::GlobalEnvAlreadyExists);
        }
        roots.push(root);
        if self.should_scan() {
            Ok(GlobalEnvChangeOutcome::AddedRescan)
        } else {
            Ok(GlobalEnvChangeOutcome::AddedIdle)
        }
    }

    pub fn remove_global_env(
        &mut self,
        root: &GlobalEnvRoot,
    ) -> Result<GlobalEnvChangeOutcome, RootOpError> {
        let roots = match self {
            Self::Unconfigured { global_env_roots }
            | Self::Configured {
                global_env_roots, ..
            } => global_env_roots,
        };
        let before_len = roots.len();
        roots.retain(|candidate| candidate != root);
        if roots.len() == before_len {
            return Err(RootOpError::GlobalEnvMissing);
        }
        if self.should_scan() {
            Ok(GlobalEnvChangeOutcome::RemovedRescan)
        } else {
            Ok(GlobalEnvChangeOutcome::RemovedIdle)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectChangeOutcome {
    AddedAndScan,
    RemovedAndScan,
    RemovedLastProject,
}

impl ProjectChangeOutcome {
    pub const fn status_message(self) -> &'static str {
        match self {
            Self::AddedAndScan => "Project added. Scanning...",
            Self::RemovedAndScan => "Project removed. Rescanning...",
            Self::RemovedLastProject => "No projects configured. Add a project folder to start.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalEnvChangeOutcome {
    AddedRescan,
    AddedIdle,
    RemovedRescan,
    RemovedIdle,
}

impl GlobalEnvChangeOutcome {
    pub const fn status_message(self) -> &'static str {
        match self {
            Self::AddedRescan => "Global env folder added. Scanning...",
            Self::AddedIdle => {
                "Global env folder added. Add a project folder to scan requests."
            }
            Self::RemovedRescan => "Global env folder removed. Rescanning...",
            Self::RemovedIdle => {
                "Global env folder removed. Add a project folder to scan requests."
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootOpError {
    ProjectAlreadyExists,
    ProjectMissing,
    GlobalEnvAlreadyExists,
    GlobalEnvMissing,
}

impl Display for RootOpError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProjectAlreadyExists => f.write_str("Project already configured"),
            Self::ProjectMissing => f.write_str("Project is not configured"),
            Self::GlobalEnvAlreadyExists => {
                f.write_str("Global environment folder already configured")
            }
            Self::GlobalEnvMissing => {
                f.write_str("Global environment folder is not configured")
            }
        }
    }
}

impl std::error::Error for RootOpError {}

#[derive(Debug, Clone, Default)]
pub struct UnconfiguredWorkspace {
    http_files: HashMap<PathBuf, HttpFile>,
    http_file_order: Vec<PathBuf>,
    all_environments: Vec<Environment>,
}

#[derive(Debug, Clone, Default)]
pub struct ConfiguredWorkspace {
    pub http_files: HashMap<PathBuf, HttpFile>,
    pub http_file_order: Vec<PathBuf>,
    pub selection: Option<RequestId>,
    pub all_environments: Vec<Environment>,
}

#[derive(Debug, Clone)]
pub enum WorkspaceState {
    Unconfigured(UnconfiguredWorkspace),
    Configured(ConfiguredWorkspace),
}

#[allow(clippy::missing_const_for_fn)]
impl WorkspaceState {
    pub fn from_config(configuration: &ProjectConfiguration, saved_http_order: Vec<PathBuf>) -> Self {
        if configuration.should_scan() {
            Self::Configured(ConfiguredWorkspace {
                http_file_order: saved_http_order,
                ..ConfiguredWorkspace::default()
            })
        } else {
            Self::Unconfigured(UnconfiguredWorkspace::default())
        }
    }

    pub fn sync_with_configuration(&mut self, configuration: &ProjectConfiguration) {
        if configuration.should_scan() {
            self.ensure_configured();
        } else {
            self.clear_for_unconfigured();
        }
    }

    pub fn ensure_configured(&mut self) -> &mut ConfiguredWorkspace {
        if matches!(self, Self::Unconfigured(_)) {
            let next = ConfiguredWorkspace::default();
            *self = Self::Configured(next);
        }
        match self {
            Self::Configured(workspace) => workspace,
            Self::Unconfigured(_) => unreachable!("workspace forced to configured"),
        }
    }

    pub const fn configured(&self) -> Option<&ConfiguredWorkspace> {
        match self {
            Self::Configured(workspace) => Some(workspace),
            Self::Unconfigured(_) => None,
        }
    }

    pub fn configured_mut(&mut self) -> Option<&mut ConfiguredWorkspace> {
        match self {
            Self::Configured(workspace) => Some(workspace),
            Self::Unconfigured(_) => None,
        }
    }

    pub fn clear_for_unconfigured(&mut self) {
        *self = Self::Unconfigured(UnconfiguredWorkspace::default());
    }

    pub fn http_files(&self) -> &HashMap<PathBuf, HttpFile> {
        match self {
            Self::Configured(workspace) => &workspace.http_files,
            Self::Unconfigured(workspace) => &workspace.http_files,
        }
    }

    pub fn http_file_order(&self) -> &[PathBuf] {
        match self {
            Self::Configured(workspace) => &workspace.http_file_order,
            Self::Unconfigured(workspace) => &workspace.http_file_order,
        }
    }

    pub fn selection(&self) -> Option<&RequestId> {
        self.configured().and_then(|workspace| workspace.selection.as_ref())
    }

    pub fn selection_cloned(&self) -> Option<RequestId> {
        self.selection().cloned()
    }

    pub fn set_selection(&mut self, selection: Option<RequestId>) {
        if let Some(workspace) = self.configured_mut() {
            workspace.selection = selection;
        }
    }

    pub fn clear_selection(&mut self) {
        self.set_selection(None);
    }

    pub fn all_environments(&self) -> &[Environment] {
        match self {
            Self::Configured(workspace) => &workspace.all_environments,
            Self::Unconfigured(workspace) => &workspace.all_environments,
        }
    }

    pub fn set_all_environments(&mut self, envs: Vec<Environment>) {
        match self {
            Self::Configured(workspace) => workspace.all_environments = envs,
            Self::Unconfigured(workspace) => workspace.all_environments = envs,
        }
    }

    pub fn clear_scan_cache(&mut self) {
        match self {
            Self::Configured(workspace) => {
                workspace.selection = None;
                workspace.http_files.clear();
                workspace.http_file_order.clear();
                workspace.all_environments.clear();
            }
            Self::Unconfigured(workspace) => {
                workspace.http_files.clear();
                workspace.http_file_order.clear();
                workspace.all_environments.clear();
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum SaveTarget {
    ExistingSelection(RequestId),
    ExplicitPath(PathBuf),
}

#[derive(Debug, Clone)]
pub struct SavePlan {
    pub root: PathBuf,
    pub target: SaveTarget,
    pub draft: RequestDraft,
}

impl SavePlan {
    pub fn into_persist_args(self) -> (PathBuf, Option<RequestId>, RequestDraft, Option<PathBuf>) {
        match self.target {
            SaveTarget::ExistingSelection(id) => (self.root, Some(id), self.draft, None),
            SaveTarget::ExplicitPath(path) => (self.root, None, self.draft, Some(path)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SavePlanError {
    InvalidPath(SavePathError),
    MissingProjectRoot,
    SelectedRequestOutsideConfiguredProjects(PathBuf),
}

impl Display for SavePlanError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPath(err) => Display::fmt(err, f),
            Self::MissingProjectRoot => {
                f.write_str("No project configured. Add a project folder first.")
            }
            Self::SelectedRequestOutsideConfiguredProjects(path) => write!(
                f,
                "Cannot save: selected request is outside configured projects: {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for SavePlanError {}

#[derive(Debug, Clone)]
pub struct AddRequestPlan {
    pub file_path: PathBuf,
    pub project_root: PathBuf,
    pub new_draft: RequestDraft,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddRequestPlanError {
    NoSelectedFile,
    SelectedFileNotLoaded(PathBuf),
    SelectedFileOutsideConfiguredProjects(PathBuf),
}

impl Display for AddRequestPlanError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSelectedFile => f.write_str("Select a file to add a request"),
            Self::SelectedFileNotLoaded(path) => write!(
                f,
                "Cannot add request: selected file is not loaded: {}",
                path.display()
            ),
            Self::SelectedFileOutsideConfiguredProjects(path) => write!(
                f,
                "Cannot resolve project for selected file: {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for AddRequestPlanError {}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tempfile::tempdir;

    use super::*;
    use crate::model::RequestDraft;

    #[test]
    fn remove_last_project_transitions_to_unconfigured() {
        let project_dir = tempdir().expect("project dir");
        let env_dir = tempdir().expect("env dir");
        let project = ProjectRoot::from_stored(project_dir.path().to_path_buf()).expect("project root");
        let global_env =
            GlobalEnvRoot::from_stored(env_dir.path().to_path_buf()).expect("global env root");

        let mut config =
            ProjectConfiguration::from_loaded(vec![project.clone()], vec![global_env.clone()]);
        let outcome = config.remove_project(&project).expect("remove project");

        assert_eq!(outcome, ProjectChangeOutcome::RemovedLastProject);
        assert!(!config.should_scan());
        assert!(config.project_roots().is_empty());
        assert_eq!(config.global_env_roots(), [global_env]);
    }

    #[test]
    fn workspace_sync_drops_selection_when_unconfigured() {
        let mut workspace = WorkspaceState::Configured(ConfiguredWorkspace {
            http_files: HashMap::new(),
            http_file_order: vec![PathBuf::from("a.http")],
            selection: Some(RequestId::HttpFile {
                path: PathBuf::from("a.http"),
                index: 0,
            }),
            all_environments: Vec::new(),
        });
        let config = ProjectConfiguration::Unconfigured {
            global_env_roots: Vec::new(),
        };

        workspace.sync_with_configuration(&config);

        assert!(workspace.selection().is_none());
        assert!(workspace.http_files().is_empty());
        assert!(workspace.http_file_order().is_empty());
        assert!(workspace.all_environments().is_empty());
    }

    #[test]
    fn save_plan_target_prevents_mixed_selection_and_explicit_path() {
        let plan = SavePlan {
            root: PathBuf::from("root"),
            target: SaveTarget::ExistingSelection(RequestId::HttpFile {
                path: PathBuf::from("request.http"),
                index: 0,
            }),
            draft: RequestDraft::default(),
        };

        let (_root, selection, _draft, explicit_path) = plan.into_persist_args();
        assert!(selection.is_some());
        assert!(explicit_path.is_none());
    }
}
