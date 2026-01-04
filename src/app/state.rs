use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

use iced::widget::{pane_grid, text_editor};
use reqwest::Client;

use crate::model::{Environment, HttpFile, RequestDraft, RequestId, ResponsePreview};
use crate::state::AppState;

use super::messages::EditTarget;
use super::options::{AuthState, RequestMode};
use super::status::{default_environment, status_with_missing, with_default_environment};
use super::view::{BuilderPane, PaneContent, ResponseDisplay, ResponseTab, WorkspacePane};

const MIN_SPLIT_RATIO: f32 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SplitRatio(f32);

impl SplitRatio {
    pub fn new(raw: f32) -> Self {
        let clamped = raw.clamp(MIN_SPLIT_RATIO, 1.0 - MIN_SPLIT_RATIO);
        Self(clamped)
    }

    pub const fn get(self) -> f32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnvironmentIndex(usize);

impl EnvironmentIndex {
    pub const fn new(index: usize, envs: &[Environment]) -> Option<Self> {
        if index < envs.len() {
            Some(Self(index))
        } else {
            None
        }
    }

    pub fn find(name: &str, envs: &[Environment]) -> Option<Self> {
        envs.iter().position(|env| env.name == name).map(Self)
    }

    pub const fn get(self) -> usize {
        self.0
    }
}

#[derive(Debug, Default)]
pub enum EditState {
    #[default]
    Off,
    On { selection: HashSet<EditTarget> },
}

#[derive(Debug, Clone)]
pub struct HeaderRow {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct LoadedDraft {
    pub draft: RequestDraft,
    pub save_path: String,
}

#[derive(Debug, Clone)]
pub struct AppModel {
    pub draft: RequestDraft,
    pub body_editor: text_editor::Content,
    pub save_path: String,
    pub mode: RequestMode,
    pub auth: AuthState,
    pub graphql_query: text_editor::Content,
    pub graphql_variables: text_editor::Content,
    pub header_rows: Vec<HeaderRow>,
}

impl Default for AppModel {
    fn default() -> Self {
        let draft = RequestDraft::default();
        let body_editor = text_editor::Content::with_text(&draft.body);
        Self {
            draft,
            body_editor,
            save_path: String::new(),
            mode: RequestMode::Rest,
            auth: AuthState::default(),
            graphql_query: text_editor::Content::with_text(""),
            graphql_variables: text_editor::Content::with_text("{}"),
            header_rows: vec![HeaderRow {
                name: String::new(),
                value: String::new(),
            }],
        }
    }
}

impl AppModel {
    pub fn load_draft(&mut self, loaded: LoadedDraft) {
        self.draft = loaded.draft;
        self.save_path = loaded.save_path;
        self.body_editor = text_editor::Content::with_text(&self.draft.body);
        self.set_header_rows_from_draft();
    }

    pub fn set_header_rows_from_draft(&mut self) {
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
        if self.header_rows.is_empty() {
            self.header_rows.push(HeaderRow {
                name: String::new(),
                value: String::new(),
            });
        }
    }

    pub fn rebuild_headers_from_rows(&mut self) {
        let lines: Vec<String> = self
            .header_rows
            .iter()
            .filter(|row| !row.name.trim().is_empty())
            .map(|row| format!("{}: {}", row.name.trim(), row.value.trim()))
            .collect();
        self.draft.headers = lines.join("\n");
    }
}

#[derive(Debug)]
pub struct ViewState {
    pub http_files: HashMap<PathBuf, HttpFile>,
    pub http_file_order: Vec<PathBuf>,
    pub selection: Option<RequestId>,
    pub edit_state: EditState,
    pub status_line: String,
    pub last_response: Option<ResponsePreview>,
    pub environments: Vec<Environment>,
    pub active_environment: EnvironmentIndex,
    pub http_root: PathBuf,
    pub response_viewer: text_editor::Content,
    pub response_display: ResponseDisplay,
    pub response_tab: ResponseTab,
    pub show_shortcuts: bool,
    pub pending_rescan: bool,
    pub last_scan: Option<Instant>,
    pub panes: pane_grid::State<PaneContent>,
    pub workspace_panes: pane_grid::State<WorkspacePane>,
    pub builder_panes: pane_grid::State<BuilderPane>,
    pub collapsed_collections: BTreeSet<String>,
}

impl ViewState {
    pub fn new(
        http_root: PathBuf,
        panes: pane_grid::State<PaneContent>,
        workspace_panes: pane_grid::State<WorkspacePane>,
        builder_panes: pane_grid::State<BuilderPane>,
        http_file_order: Vec<PathBuf>,
    ) -> Self {
        let environments = vec![default_environment()];
        let active_environment =
            EnvironmentIndex::new(0, &environments).expect("default environment index");
        Self {
            http_files: HashMap::new(),
            http_file_order,
            selection: None,
            edit_state: EditState::default(),
            status_line: "Ready".to_string(),
            last_response: None,
            environments,
            active_environment,
            http_root,
            response_viewer: text_editor::Content::with_text("No response yet"),
            response_display: ResponseDisplay::Pretty,
            response_tab: ResponseTab::Body,
            show_shortcuts: false,
            pending_rescan: false,
            last_scan: None,
            panes,
            workspace_panes,
            builder_panes,
            collapsed_collections: BTreeSet::new(),
        }
    }

    pub fn update_status_with_model(&mut self, base: &str, model: &AppModel) {
        let env = self.environments.get(self.active_environment.get());
        let extras = if model.mode == RequestMode::GraphQl {
            vec![model.graphql_query.text(), model.graphql_variables.text()]
        } else {
            Vec::new()
        };
        let extra_refs: Vec<&str> = extras.iter().map(String::as_str).collect();
        self.status_line = status_with_missing(base, &model.draft, env, &extra_refs);
    }

    pub fn update_status_with_draft(
        &mut self,
        base: &str,
        draft: &RequestDraft,
        extra_inputs: &[String],
    ) {
        let env = self.environments.get(self.active_environment.get());
        let extra_refs: Vec<&str> = extra_inputs.iter().map(String::as_str).collect();
        self.status_line = status_with_missing(base, draft, env, &extra_refs);
    }

    pub fn update_response_viewer(&mut self) {
        let body_text = self
            .last_response
            .as_ref()
            .and_then(|resp| resp.error.clone().or_else(|| resp.body.clone()))
            .unwrap_or_else(|| "No response yet".to_string());
        let display_text = match (
            self.response_display,
            super::view::pretty_json(&body_text),
        ) {
            (ResponseDisplay::Pretty, Some(pretty)) => pretty,
            _ => body_text,
        };
        self.response_viewer = text_editor::Content::with_text(&display_text);
    }

    pub fn set_environments(&mut self, envs: Vec<Environment>, state: &mut AppState) {
        self.environments = with_default_environment(envs);
        let next_active = state
            .active_environment
            .clone()
            .and_then(|saved| EnvironmentIndex::find(&saved, &self.environments))
            .unwrap_or_else(|| {
                EnvironmentIndex::new(0, &self.environments)
                    .expect("default environment index")
            });
        self.active_environment = next_active;
        state.active_environment = Some(
            self.environments[self.active_environment.get()]
                .name
                .clone(),
        );
    }

    pub fn active_environment_name(&self) -> String {
        self.environments[self.active_environment.get()].name.clone()
    }

    pub fn resolve_request(&self, id: &RequestId) -> Option<LoadedDraft> {
        let RequestId::HttpFile { path, index } = id else {
            return None;
        };
        let draft = self
            .http_files
            .get(path)
            .and_then(|file| file.requests.get(*index))?;
        let save_path = path.display().to_string();
        Some(LoadedDraft {
            draft: draft.clone(),
            save_path,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Runtime {
    pub client: Client,
    pub state: AppState,
}
