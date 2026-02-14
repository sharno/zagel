use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use iced::widget::pane_grid;
use iced::{Subscription, Task, Theme, application};
use reqwest::Client;

use crate::model::{RequestDraft, RequestId};
use crate::parser::{scan_env_files, scan_http_files};
use crate::pathing::{GlobalEnvRoot, ProjectRoot, SaveFilePath};
use crate::state::AppState;

use super::domain::{
    AddRequestPlan, AddRequestPlanError, ProjectConfiguration, SavePlan, SavePlanError,
    SaveTarget, WorkspaceState,
};
use super::options::{AuthState, RequestMode};
use super::status::{default_environment, status_with_missing};
use super::{EditTarget, Message, hotkeys, view, watcher};

const FILE_SCAN_MAX_DEPTH: usize = 6;

#[derive(Debug, Clone)]
pub struct HeaderRow {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Default)]
pub enum EditState {
    #[default]
    Off,
    On {
        selection: HashSet<EditTarget>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartupStatus {
    Ready,
    NeedsProject,
    WarningReady { ignored: usize },
    WarningNeedsProject { ignored: usize },
}

impl StartupStatus {
    const fn from_context(startup_warnings: &[String], configuration: &ProjectConfiguration) -> Self {
        match (startup_warnings.is_empty(), configuration.should_scan()) {
            (true, true) => Self::Ready,
            (true, false) => Self::NeedsProject,
            (false, true) => Self::WarningReady {
                ignored: startup_warnings.len(),
            },
            (false, false) => Self::WarningNeedsProject {
                ignored: startup_warnings.len(),
            },
        }
    }

    fn status_line(self) -> String {
        match self {
            Self::Ready => "Ready".to_string(),
            Self::NeedsProject => "No projects configured. Add a project folder to start.".to_string(),
            Self::WarningReady { ignored } => {
                format!("Ignored {ignored} invalid saved folder(s).")
            }
            Self::WarningNeedsProject { ignored } => {
                format!("Ignored {ignored} invalid saved folder(s). Add a project folder to start.")
            }
        }
    }
}

pub struct Zagel {
    pub(super) workspace: WorkspaceState,
    pub(super) configuration: ProjectConfiguration,
    pub(super) edit_state: EditState,
    pub(super) draft: RequestDraft,
    pub(super) body_editor: iced::widget::text_editor::Content,
    pub(super) status_line: String,
    pub(super) response: Option<crate::app::view::ResponseData>,
    pub(super) environments: Vec<crate::model::Environment>,
    pub(super) active_environment: usize,
    pub(super) state: AppState,
    pub(super) client: Client,
    pub(super) response_viewer: iced::widget::text_editor::Content,
    pub(super) save_path: String,
    pub(super) project_path_input: String,
    pub(super) global_env_path_input: String,
    pub(super) mode: RequestMode,
    pub(super) auth: AuthState,
    pub(super) graphql_query: iced::widget::text_editor::Content,
    pub(super) graphql_variables: iced::widget::text_editor::Content,
    pub(super) header_rows: Vec<HeaderRow>,
    pub(super) response_display: crate::app::view::ResponseDisplay,
    pub(super) response_tab: crate::app::view::ResponseTab,
    pub(super) icon_set: crate::app::view::IconSet,
    pub(super) show_shortcuts: bool,
    pub(super) pending_rescan: bool,
    pub(super) last_scan: Option<Instant>,
    pub(super) panes: pane_grid::State<crate::app::view::PaneContent>,
    pub(super) workspace_panes: pane_grid::State<crate::app::view::WorkspacePane>,
    pub(super) builder_panes: pane_grid::State<crate::app::view::BuilderPane>,
    pub(super) collapsed_collections: BTreeSet<String>,
}

fn load_configured_roots(state: &AppState) -> (ProjectConfiguration, Vec<String>) {
    let mut startup_warnings = Vec::new();

    let mut project_roots = Vec::new();
    for path in &state.project_roots {
        match ProjectRoot::from_stored(path.clone()) {
            Ok(root) => project_roots.push(root),
            Err(err) => startup_warnings.push(format!(
                "Ignoring saved project folder {}: {err}",
                path.display()
            )),
        }
    }

    let mut global_env_roots = Vec::new();
    for path in &state.global_env_roots {
        match GlobalEnvRoot::from_stored(path.clone()) {
            Ok(root) => global_env_roots.push(root),
            Err(err) => startup_warnings.push(format!(
                "Ignoring saved global env folder {}: {err}",
                path.display()
            )),
        }
    }

    (
        ProjectConfiguration::from_loaded(project_roots, global_env_roots),
        startup_warnings,
    )
}

impl Zagel {
    pub(super) fn init() -> (Self, Task<Message>) {
        let state = AppState::load();
        let (configuration, startup_warnings) = load_configured_roots(&state);
        let startup_status = StartupStatus::from_context(&startup_warnings, &configuration);
        let initial_status_line = startup_status.status_line();

        let (mut panes, sidebar) = pane_grid::State::new(super::view::PaneContent::Sidebar);
        let split = panes.split(
            pane_grid::Axis::Vertical,
            sidebar,
            super::view::PaneContent::Workspace,
        );
        if let Some((_, split)) = split {
            panes.resize(split, 0.26);
        }

        let (mut workspace_panes, builder) =
            pane_grid::State::new(super::view::WorkspacePane::Builder);
        if let Some((_, split)) = workspace_panes.split(
            pane_grid::Axis::Horizontal,
            builder,
            super::view::WorkspacePane::Response,
        ) {
            workspace_panes.resize(split, 0.62);
        }

        let (mut builder_panes, form) = pane_grid::State::new(super::view::BuilderPane::Form);
        if let Some((_, split)) = builder_panes.split(
            pane_grid::Axis::Vertical,
            form,
            super::view::BuilderPane::Body,
        ) {
            builder_panes.resize(split, 0.45);
        }

        let workspace = WorkspaceState::from_config(&configuration, state.http_file_order.clone());

        let mut app = Self {
            workspace,
            configuration,
            edit_state: EditState::default(),
            draft: RequestDraft::default(),
            body_editor: iced::widget::text_editor::Content::with_text(""),
            status_line: initial_status_line,
            response: None,
            environments: vec![default_environment()],
            active_environment: 0,
            state,
            client: Client::new(),
            response_viewer: iced::widget::text_editor::Content::with_text("No response yet"),
            save_path: String::new(),
            project_path_input: String::new(),
            global_env_path_input: String::new(),
            mode: RequestMode::Rest,
            auth: AuthState::default(),
            graphql_query: iced::widget::text_editor::Content::with_text(""),
            graphql_variables: iced::widget::text_editor::Content::with_text("{}"),
            header_rows: Vec::new(),
            response_display: crate::app::view::ResponseDisplay::Pretty,
            response_tab: crate::app::view::ResponseTab::Body,
            icon_set: crate::app::view::IconSet::from_env(),
            show_shortcuts: false,
            pending_rescan: false,
            last_scan: None,
            panes,
            workspace_panes,
            builder_panes,
            collapsed_collections: BTreeSet::new(),
        };

        for warning in &startup_warnings {
            eprintln!("startup: {warning}");
        }

        app.refresh_visible_environments();
        let task = if app.should_scan() {
            if startup_warnings.is_empty() {
                app.update_status_with_missing("Ready");
            }
            app.rescan_files()
        } else {
            Task::none()
        };
        app.persist_state();
        (app, task)
    }

    pub(super) fn subscription(state: &Self) -> Subscription<Message> {
        let watch_roots = state.watch_roots_paths();
        Subscription::batch([
            hotkeys::subscription(),
            watcher::subscription_many(watch_roots),
        ])
    }

    pub(super) const fn theme(state: &Self) -> Theme {
        state.state.theme.iced_theme()
    }

    pub(super) fn rescan_files(&self) -> Task<Message> {
        Task::batch([
            Task::perform(
                scan_http_files(self.project_root_paths(), FILE_SCAN_MAX_DEPTH),
                Message::HttpFilesLoaded,
            ),
            Task::perform(
                scan_env_files(
                    self.project_root_paths(),
                    self.global_env_root_paths(),
                    FILE_SCAN_MAX_DEPTH,
                ),
                Message::EnvironmentsLoaded,
            ),
        ])
    }

    pub(super) fn persist_state(&mut self) {
        let mut state = self.state.clone();
        state.project_roots = self.project_root_paths();
        state.global_env_roots = self.global_env_root_paths();
        state.http_root = state.project_roots.first().cloned();
        state
            .http_file_order
            .clone_from(self.workspace.http_file_order());
        self.state = state.clone();
        state.save();
    }

    pub(super) fn refresh_visible_environments(&mut self) {
        let previous_name = self
            .environments
            .get(self.active_environment)
            .map(|env| env.name.clone());
        let selected_project = self.selected_project_root().map(ProjectRoot::as_path);

        let mut visible = self
            .workspace
            .all_environments()
            .iter()
            .filter(|env| env.visible_for_project(selected_project))
            .cloned()
            .collect::<Vec<_>>();
        visible.sort_by(|a, b| a.name.cmp(&b.name));

        self.environments = super::status::with_default_environment(visible);

        let target_name = previous_name.or_else(|| self.state.active_environment.clone());
        if let Some(name) = target_name
            && let Some((idx, _)) = self
                .environments
                .iter()
                .enumerate()
                .find(|(_, env)| env.name == name)
        {
            self.active_environment = idx;
        } else {
            self.active_environment = 0;
        }

        if let Some(active) = self.environments.get(self.active_environment) {
            self.state.active_environment = Some(active.name.clone());
        }
    }

    pub(super) fn project_roots(&self) -> &[ProjectRoot] {
        self.configuration.project_roots()
    }

    pub(super) fn global_env_roots(&self) -> &[GlobalEnvRoot] {
        self.configuration.global_env_roots()
    }

    pub(super) fn project_root_paths(&self) -> Vec<PathBuf> {
        self.configuration.project_root_paths()
    }

    pub(super) fn global_env_root_paths(&self) -> Vec<PathBuf> {
        self.configuration.global_env_root_paths()
    }

    fn watch_roots_paths(&self) -> Vec<PathBuf> {
        self.configuration.watch_root_paths()
    }

    pub(super) fn default_project_root(&self) -> Option<&ProjectRoot> {
        self.configuration.default_project_root()
    }

    pub(super) const fn should_scan(&self) -> bool {
        self.configuration.should_scan()
    }

    pub(super) fn selected_project_root(&self) -> Option<&ProjectRoot> {
        let RequestId::HttpFile { path, .. } = self.workspace.selection()?;
        self.project_root_for_path(path)
    }

    pub(super) fn project_root_for_path(&self, path: &Path) -> Option<&ProjectRoot> {
        self.configuration.project_root_for_path(path)
    }

    pub(super) fn apply_selection(&mut self, id: &RequestId) {
        let RequestId::HttpFile { path, index } = id;
        let maybe_request = {
            let Some(workspace) = self.workspace.configured_mut() else {
                return;
            };
            workspace.selection = Some(id.clone());
            workspace
                .http_files
                .get(path)
                .and_then(|file| file.requests.get(*index))
                .cloned()
        };

        let draft = maybe_request.unwrap_or_else(|| RequestDraft {
            title: "New request".to_string(),
            ..Default::default()
        });
        self.draft = draft.clone();
        self.body_editor = iced::widget::text_editor::Content::with_text(&draft.body);
        self.set_header_rows_from_draft();
        self.refresh_visible_environments();
        self.save_path = path.display().to_string();
        self.update_status_with_missing("Ready");
        self.update_response_viewer();
    }

    pub(super) fn build_save_plan(&self) -> Result<SavePlan, SavePlanError> {
        let draft = self.draft.clone();
        if let Some(id) = self.workspace.selection_cloned() {
            let RequestId::HttpFile { path: selected_path, .. } = &id;
            let Some(root) = self.project_root_for_path(selected_path) else {
                return Err(SavePlanError::SelectedRequestOutsideConfiguredProjects(
                    selected_path.clone(),
                ));
            };
            Ok(SavePlan {
                root: root.to_path_buf(),
                target: SaveTarget::ExistingSelection(id),
                draft,
            })
        } else {
            let Some(default_root) = self.default_project_root() else {
                return Err(SavePlanError::MissingProjectRoot);
            };
            let explicit_path = SaveFilePath::parse_user_input(&self.save_path, Some(default_root))
                .map_err(SavePlanError::InvalidPath)?;
            Ok(SavePlan {
                root: default_root.to_path_buf(),
                target: SaveTarget::ExplicitPath(explicit_path.to_path_buf()),
                draft,
            })
        }
    }

    pub(super) fn build_add_request_plan(&self) -> Result<AddRequestPlan, AddRequestPlanError> {
        let Some(RequestId::HttpFile { path, .. }) = self.workspace.selection_cloned() else {
            return Err(AddRequestPlanError::NoSelectedFile);
        };
        let Some(workspace) = self.workspace.configured() else {
            return Err(AddRequestPlanError::NoSelectedFile);
        };
        if !workspace.http_files.contains_key(&path) {
            return Err(AddRequestPlanError::SelectedFileNotLoaded(path));
        }
        let Some(project_root) = self.project_root_for_path(&path) else {
            return Err(AddRequestPlanError::SelectedFileOutsideConfiguredProjects(path));
        };
        Ok(AddRequestPlan {
            file_path: path,
            project_root: project_root.to_path_buf(),
            new_draft: RequestDraft {
                title: "New request".to_string(),
                ..Default::default()
            },
        })
    }

    pub(super) fn set_header_rows_from_draft(&mut self) {
        self.header_rows.clear();
        if self.draft.headers.is_empty() {
            self.header_rows.push(HeaderRow {
                name: String::new(),
                value: String::new(),
            });
            return;
        }
        for line in self.draft.headers.lines() {
            if let Some((name, value)) = line.split_once(':') {
                self.header_rows.push(HeaderRow {
                    name: name.trim().to_string(),
                    value: value.trim().to_string(),
                });
            }
        }
    }

    pub(super) fn rebuild_headers_from_rows(&mut self) {
        let lines: Vec<String> = self
            .header_rows
            .iter()
            .filter(|row| !row.name.trim().is_empty())
            .map(|row| format!("{}: {}", row.name.trim(), row.value.trim()))
            .collect();
        self.draft.headers = lines.join("\n");
    }

    pub(super) fn update_response_viewer(&mut self) {
        let display_text = match (self.response_display, self.response.as_ref()) {
            (super::view::ResponseDisplay::Pretty, Some(response)) => response
                .body
                .pretty_text()
                .unwrap_or_else(|| response.body.raw())
                .to_string(),
            (_, Some(response)) => response.body.raw().to_string(),
            (_, None) => "No response yet".to_string(),
        };

        self.response_viewer = iced::widget::text_editor::Content::with_text(&display_text);
    }

    pub(super) fn update_status_with_missing(&mut self, base: &str) {
        let env = self.environments.get(self.active_environment);
        let extras = if self.mode == RequestMode::GraphQl {
            vec![self.graphql_query.text(), self.graphql_variables.text()]
        } else {
            Vec::new()
        };
        let extra_refs: Vec<&str> = extras.iter().map(std::string::String::as_str).collect();
        self.status_line = status_with_missing(base, &self.draft, env, &extra_refs);
    }
}

pub fn run() -> iced::Result {
    application(Zagel::init, Zagel::update, view::view)
        .title("Zagel - REST workbench")
        .subscription(Zagel::subscription)
        .theme(Zagel::theme)
        .run()
}
