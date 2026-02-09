use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::path::PathBuf;
use std::time::Instant;

use iced::widget::pane_grid;
use iced::{Subscription, Task, Theme, application};
use reqwest::Client;

use crate::model::{Environment, HttpFile, RequestDraft, RequestId};
use crate::parser::{scan_env_files, scan_http_files};
use crate::pathing::{GlobalEnvRoot, ProjectRoot};
use crate::state::AppState;

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

pub struct Zagel {
    pub(super) http_files: HashMap<PathBuf, HttpFile>,
    pub(super) http_file_order: Vec<PathBuf>,
    pub(super) selection: Option<RequestId>,
    pub(super) edit_state: EditState,
    pub(super) draft: RequestDraft,
    pub(super) body_editor: iced::widget::text_editor::Content,
    pub(super) status_line: String,
    pub(super) response: Option<crate::app::view::ResponseData>,
    pub(super) all_environments: Vec<Environment>,
    pub(super) environments: Vec<Environment>,
    pub(super) active_environment: usize,
    pub(super) project_roots: Vec<ProjectRoot>,
    pub(super) global_env_roots: Vec<GlobalEnvRoot>,
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

fn load_configured_roots(state: &AppState) -> (Vec<ProjectRoot>, Vec<GlobalEnvRoot>, Vec<String>) {
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

    (project_roots, global_env_roots, startup_warnings)
}

fn startup_status_line(
    startup_warnings: &[String],
    project_roots: &[ProjectRoot],
    global_env_roots: &[GlobalEnvRoot],
) -> String {
    let startup_warning_summary = (!startup_warnings.is_empty()).then(|| {
        format!(
            "Ignored {} invalid saved folder(s).",
            startup_warnings.len()
        )
    });
    let has_any_root = !(project_roots.is_empty() && global_env_roots.is_empty());

    match (startup_warning_summary, has_any_root) {
        (Some(summary), false) => format!("{summary} Add a project folder to start."),
        (Some(summary), true) => summary,
        (None, false) => "No projects configured. Add a project folder to start.".to_string(),
        (None, true) => "Ready".to_string(),
    }
}

impl Zagel {
    pub(super) fn init() -> (Self, Task<Message>) {
        let state = AppState::load();
        let (project_roots, global_env_roots, startup_warnings) = load_configured_roots(&state);
        let initial_status_line =
            startup_status_line(&startup_warnings, &project_roots, &global_env_roots);

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

        let mut app = Self {
            http_files: HashMap::new(),
            http_file_order: state.http_file_order.clone(),
            selection: None,
            edit_state: EditState::default(),
            draft: RequestDraft::default(),
            body_editor: iced::widget::text_editor::Content::with_text(""),
            status_line: initial_status_line,
            response: None,
            all_environments: Vec::new(),
            environments: vec![default_environment()],
            active_environment: 0,
            project_roots,
            global_env_roots,
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
        state.http_file_order.clone_from(&self.http_file_order);
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
            .all_environments
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

    pub(super) fn project_root_paths(&self) -> Vec<PathBuf> {
        self.project_roots
            .iter()
            .map(ProjectRoot::to_path_buf)
            .collect()
    }

    pub(super) fn global_env_root_paths(&self) -> Vec<PathBuf> {
        self.global_env_roots
            .iter()
            .map(GlobalEnvRoot::to_path_buf)
            .collect()
    }

    fn watch_roots_paths(&self) -> Vec<PathBuf> {
        let mut roots = self.project_root_paths();
        roots.extend(self.global_env_root_paths());
        roots.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
        roots.dedup();
        roots
    }

    pub(super) fn default_project_root(&self) -> Option<&ProjectRoot> {
        self.project_roots.first()
    }

    #[allow(clippy::missing_const_for_fn)]
    pub(super) fn should_scan(&self) -> bool {
        !(self.project_roots.is_empty() && self.global_env_roots.is_empty())
    }

    pub(super) fn selected_project_root(&self) -> Option<&ProjectRoot> {
        let RequestId::HttpFile { path, .. } = self.selection.as_ref()?;
        self.project_root_for_path(path)
    }

    pub(super) fn project_root_for_path(&self, path: &Path) -> Option<&ProjectRoot> {
        self.project_roots
            .iter()
            .filter(|root| path.starts_with(root.as_path()))
            .max_by_key(|root| root.as_path().components().count())
    }

    pub(super) fn apply_selection(&mut self, id: &RequestId) {
        let RequestId::HttpFile { path, index } = id;
        self.selection = Some(id.clone());
        let maybe_request = self
            .http_files
            .get(path)
            .and_then(|file| file.requests.get(*index));

        let draft = maybe_request.cloned().unwrap_or_else(|| RequestDraft {
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
        .title("Zagel  REST workbench")
        .subscription(Zagel::subscription)
        .theme(Zagel::theme)
        .run()
}
