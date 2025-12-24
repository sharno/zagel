use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use iced::widget::pane_grid;
use iced::{Subscription, Task, Theme, application, time};
use reqwest::Client;

use crate::model::{
    Collection, Environment, HttpFile, RequestDraft, RequestId, ResponsePreview, UnsavedTab,
};
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
    pub(super) unsaved_tabs: Vec<UnsavedTab>,
    pub(super) selection: Option<RequestId>,
    pub(super) draft: RequestDraft,
    pub(super) body_editor: iced::widget::text_editor::Content,
    pub(super) status_line: String,
    pub(super) last_response: Option<ResponsePreview>,
    pub(super) environments: Vec<Environment>,
    pub(super) active_environment: usize,
    pub(super) http_root: PathBuf,
    pub(super) state: AppState,
    pub(super) client: Client,
    pub(super) next_unsaved_id: u32,
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
            panes.resize(split, 0.32);
        }

        let mut app = Self {
            collections: Vec::new(),
            http_files: HashMap::new(),
            unsaved_tabs: Vec::new(),
            selection: None,
            draft: RequestDraft::default(),
            body_editor: iced::widget::text_editor::Content::with_text(""),
            status_line: "Ready".to_string(),
            last_response: None,
            environments: vec![default_environment()],
            active_environment: 0,
            http_root,
            state,
            client: Client::new(),
            next_unsaved_id: 1,
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
            RequestId::Unsaved(_) => None,
        };

        let draft = maybe_request.cloned().unwrap_or_else(|| RequestDraft {
            title: "Unsaved request".to_string(),
            ..Default::default()
        });
        self.draft = draft.clone();
        self.body_editor = iced::widget::text_editor::Content::with_text(&draft.body);
        self.set_header_rows_from_draft();
        self.save_path = match id {
            RequestId::HttpFile { path, .. } => path.display().to_string(),
            _ => suggest_http_path(&self.http_root, &draft.title)
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
        let extra_refs: Vec<&str> = extras.iter().map(|s| s.as_str()).collect();
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
