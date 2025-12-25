use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use iced::widget::pane_grid;
use iced::{Subscription, Task, Theme, application, time};
use reqwest::Client;

use crate::model::{Collection, Environment, HttpFile, RequestDraft, RequestId, ResponsePreview};
use crate::parser::{scan_env_files, scan_http_files, suggest_http_path};
use crate::state::AppState;

use super::options::{AuthState, RequestMode};
use super::status::{default_environment, status_with_missing};
use super::{Message, hotkeys, view};

const FILE_SCAN_MAX_DEPTH: usize = 6;
const FILE_SCAN_COOLDOWN: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub struct HeaderRow {
    pub name: String,
    pub value: String,
}

pub struct Zagel {
    pub(super) collections: Vec<Collection>,
    pub(super) http_files: HashMap<PathBuf, HttpFile>,
    pub(super) http_file_order: Vec<PathBuf>,
    pub(super) selection: Option<RequestId>,
    pub(super) editing: bool,
    pub(super) edit_selection: HashSet<super::EditTarget>,
    pub(super) draft: RequestDraft,
    pub(super) body_editor: iced::widget::text_editor::Content,
    pub(super) status_line: String,
    pub(super) last_response: Option<ResponsePreview>,
    pub(super) environments: Vec<Environment>,
    pub(super) active_environment: usize,
    pub(super) http_root: PathBuf,
    pub(super) state: AppState,
    pub(super) client: Client,
    pub(super) response_viewer: iced::widget::text_editor::Content,
    pub(super) save_path: String,
    pub(super) mode: RequestMode,
    pub(super) auth: AuthState,
    pub(super) graphql_query: iced::widget::text_editor::Content,
    pub(super) graphql_variables: iced::widget::text_editor::Content,
    pub(super) header_rows: Vec<HeaderRow>,
    pub(super) response_display: crate::app::view::ResponseDisplay,
    pub(super) response_tab: crate::app::view::ResponseTab,
    pub(super) panes: pane_grid::State<crate::app::view::PaneContent>,
    pub(super) workspace_panes: pane_grid::State<crate::app::view::WorkspacePane>,
    pub(super) builder_panes: pane_grid::State<crate::app::view::BuilderPane>,
    pub(super) collapsed_collections: BTreeSet<String>,
}

impl Zagel {
    pub(super) fn init() -> (Self, Task<Message>) {
        let state = AppState::load();
        let http_root = state
            .http_root
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

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
            pane_grid::Axis::Vertical,
            builder,
            super::view::WorkspacePane::Response,
        ) {
            workspace_panes.resize(split, 0.6);
        }

        let (mut builder_panes, form) = pane_grid::State::new(super::view::BuilderPane::Form);
        if let Some((_, split)) = builder_panes.split(
            pane_grid::Axis::Horizontal,
            form,
            super::view::BuilderPane::Body,
        ) {
            builder_panes.resize(split, 0.55);
        }

        let mut app = Self {
            collections: Vec::new(),
            http_files: HashMap::new(),
            http_file_order: Vec::new(),
            selection: None,
            editing: false,
            edit_selection: HashSet::new(),
            draft: RequestDraft::default(),
            body_editor: iced::widget::text_editor::Content::with_text(""),
            status_line: "Ready".to_string(),
            last_response: None,
            environments: vec![default_environment()],
            active_environment: 0,
            http_root,
            state,
            client: Client::new(),
            response_viewer: iced::widget::text_editor::Content::with_text("No response yet"),
            save_path: String::new(),
            mode: RequestMode::Rest,
            auth: AuthState::default(),
            graphql_query: iced::widget::text_editor::Content::with_text(""),
            graphql_variables: iced::widget::text_editor::Content::with_text("{}"),
            header_rows: Vec::new(),
            response_display: crate::app::view::ResponseDisplay::Pretty,
            response_tab: crate::app::view::ResponseTab::Body,
            panes,
            workspace_panes,
            builder_panes,
            collapsed_collections: BTreeSet::new(),
        };

        let task = app.rescan_files();
        app.persist_state();
        app.update_status_with_missing("Ready");
        (app, task)
    }

    pub(super) fn subscription(_state: &Self) -> Subscription<Message> {
        Subscription::batch([
            time::every(FILE_SCAN_COOLDOWN).map(|_| Message::Tick),
            hotkeys::subscription(),
        ])
    }

    pub(super) const fn theme(_: &Self) -> Theme {
        Theme::Nord
    }

    pub(super) fn rescan_files(&self) -> Task<Message> {
        Task::batch([
            Task::perform(
                scan_http_files(self.http_root.clone(), FILE_SCAN_MAX_DEPTH),
                Message::HttpFilesLoaded,
            ),
            Task::perform(
                scan_env_files(self.http_root.clone(), FILE_SCAN_MAX_DEPTH),
                Message::EnvironmentsLoaded,
            ),
        ])
    }

    pub(super) fn persist_state(&self) {
        let mut state = self.state.clone();
        state.http_root = Some(self.http_root.clone());
        state.save();
    }

    pub(super) fn apply_saved_environment(&mut self) {
        if let Some(saved) = self.state.active_environment.clone()
            && let Some((idx, _)) = self
                .environments
                .iter()
                .enumerate()
                .find(|(_, env)| env.name == saved)
        {
            self.active_environment = idx;
            self.state.active_environment =
                Some(self.environments[self.active_environment].name.clone());
            return;
        }
        self.active_environment = 0;
        if let Some(env) = self.environments.get(self.active_environment) {
            self.state.active_environment = Some(env.name.clone());
        }
    }

    pub(super) fn apply_selection(&mut self, id: &RequestId) {
        self.selection = Some(id.clone());
        let maybe_request = match id {
            RequestId::Collection { collection, index } => self
                .collections
                .get(*collection)
                .and_then(|c| c.requests.get(*index)),
            RequestId::HttpFile { path, index } => self
                .http_files
                .get(path)
                .and_then(|file| file.requests.get(*index)),
        };

        let draft = maybe_request.cloned().unwrap_or_else(|| RequestDraft {
            title: "New request".to_string(),
            ..Default::default()
        });
        self.draft = draft.clone();
        self.body_editor = iced::widget::text_editor::Content::with_text(&draft.body);
        self.set_header_rows_from_draft();
        self.save_path = match id {
            RequestId::HttpFile { path, .. } => path.display().to_string(),
            RequestId::Collection { .. } => suggest_http_path(&self.http_root, &draft.title)
                .display()
                .to_string(),
        };
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
        let body_text = self
            .last_response
            .as_ref()
            .and_then(|resp| resp.error.clone().or_else(|| resp.body.clone()))
            .unwrap_or_else(|| "No response yet".to_string());
        let display_text = match (self.response_display, super::view::pretty_json(&body_text)) {
            (super::view::ResponseDisplay::Pretty, Some(pretty)) => pretty,
            _ => body_text,
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
