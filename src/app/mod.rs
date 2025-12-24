mod headers;
mod hotkeys;
mod messages;
mod options;
mod view;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::time::Duration;

use iced::clipboard;
use iced::{Subscription, Task, Theme, application, time};
use reqwest::Client;

use crate::model::{
    Collection, Environment, HttpFile, Method, RequestDraft, RequestId, ResponsePreview, UnsavedTab,
};
use crate::net::send_request;
use crate::parser::{persist_request, scan_env_files, scan_http_files, suggest_http_path};
use crate::state::AppState;
pub use messages::Message;
use options::{AuthState, RequestMode, apply_auth_headers, build_graphql_body};

const FILE_SCAN_MAX_DEPTH: usize = 6;
const FILE_SCAN_COOLDOWN: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub struct HeaderRow {
    pub name: String,
    pub value: String,
}

pub fn run() -> iced::Result {
    application(Zagel::init, Zagel::update, view::view)
        .title("Zagel • REST workbench")
        .subscription(Zagel::subscription)
        .theme(Zagel::theme)
        .run()
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
}

impl Zagel {
    fn init() -> (Self, Task<Message>) {
        let state = AppState::load();
        let http_root = state
            .http_root
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

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
        };

        let task = app.rescan_files();
        app.persist_state();
        app.update_status_with_missing("Ready");
        (app, task)
    }

    fn subscription(_state: &Self) -> Subscription<Message> {
        Subscription::batch([
            time::every(FILE_SCAN_COOLDOWN).map(|_| Message::Tick),
            hotkeys::subscription(),
        ])
    }

    const fn theme(_: &Self) -> Theme {
        Theme::Nord
    }

    #[allow(clippy::too_many_lines)]
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => self.rescan_files(),
            Message::HttpFilesLoaded(files) => {
                self.http_files = files;
                Task::none()
            }
            Message::EnvironmentsLoaded(envs) => {
                self.environments = with_default_environment(envs);
                self.apply_saved_environment();
                self.persist_state();
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::Select(id) => {
                self.apply_selection(&id);
                Task::none()
            }
            Message::MethodSelected(method) => {
                self.draft.method = method;
                Task::none()
            }
            Message::UrlChanged(url) => {
                self.draft.url = url;
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::TitleChanged(title) => {
                self.draft.title = title;
                Task::none()
            }
            Message::ModeChanged(mode) => {
                self.mode = mode;
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::BodyEdited(action) => {
                self.body_editor.perform(action);
                self.draft.body = self.body_editor.text();
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::GraphqlQueryEdited(action) => {
                self.graphql_query.perform(action);
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::GraphqlVariablesEdited(action) => {
                self.graphql_variables.perform(action);
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::AuthChanged(new_auth) => {
                self.auth = new_auth;
                Task::none()
            }
            Message::HeaderNameChanged(idx, value) => {
                if let Some(row) = self.header_rows.get_mut(idx) {
                    row.name = value;
                    self.rebuild_headers_from_rows();
                }
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::HeaderValueChanged(idx, value) => {
                if let Some(row) = self.header_rows.get_mut(idx) {
                    row.value = value;
                    self.rebuild_headers_from_rows();
                }
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::HeaderAdded => {
                self.header_rows.push(HeaderRow {
                    name: String::new(),
                    value: String::new(),
                });
                self.rebuild_headers_from_rows();
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::HeaderRemoved(idx) => {
                if idx < self.header_rows.len() {
                    self.header_rows.remove(idx);
                    self.rebuild_headers_from_rows();
                }
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::ResponseViewChanged(display) => {
                self.response_display = display;
                self.update_response_viewer();
                Task::none()
            }
            Message::ResponseTabChanged(tab) => {
                self.response_tab = tab;
                Task::none()
            }
            Message::CopyResponseBody => {
                clipboard::write(self.response_viewer.text()).map(|()| Message::CopyComplete)
            }
            Message::CopyComplete => Task::none(),
            Message::AddUnsavedTab => {
                let id = self.next_unsaved_id;
                self.next_unsaved_id += 1;
                self.unsaved_tabs.push(UnsavedTab {
                    id,
                    title: format!("Unsaved {id}"),
                });
                let new_id = RequestId::Unsaved(id);
                self.apply_selection(&new_id);
                Task::none()
            }
            Message::Send => {
                let env = self.environments.get(self.active_environment).cloned();
                let mut draft = self.draft.clone();
                let mut extra_inputs: Vec<String> = Vec::new();
                if self.mode == RequestMode::GraphQl {
                    draft.method = Method::Post;
                    let query = self.graphql_query.text();
                    let variables = self.graphql_variables.text();
                    extra_inputs.push(query.clone());
                    extra_inputs.push(variables.clone());
                    draft.body = build_graphql_body(&query, &variables);
                    if !draft.headers.contains("Content-Type") {
                        draft.headers.push_str("\nContent-Type: application/json");
                    }
                }
                draft.headers = apply_auth_headers(&draft.headers, &self.auth);
                let extra_refs: Vec<&str> = extra_inputs.iter().map(|s| s.as_str()).collect();
                self.status_line =
                    status_with_missing("Sending...", &draft, env.as_ref(), &extra_refs);
                Task::perform(
                    send_request(self.client.clone(), draft, env),
                    Message::ResponseReady,
                )
            }
            Message::ResponseReady(result) => {
                match result {
                    Ok(resp) => {
                        self.update_status_with_missing("Received response");
                        self.last_response = Some(resp);
                    }
                    Err(err) => {
                        self.update_status_with_missing("Request failed");
                        self.last_response = Some(ResponsePreview::error(err));
                    }
                }
                self.update_response_viewer();
                Task::none()
            }
            Message::EnvironmentChanged(name) => {
                if let Some((idx, _)) = self
                    .environments
                    .iter()
                    .enumerate()
                    .find(|(_, env)| env.name == name)
                {
                    self.active_environment = idx;
                    self.state.active_environment = Some(name);
                    self.persist_state();
                }
                self.update_status_with_missing("Ready");
                Task::none()
            }
            Message::Save => {
                let selection = self.selection.clone();
                let draft = self.draft.clone();
                let root = self.http_root.clone();
                let explicit_path = if let Some(RequestId::HttpFile { .. }) = selection {
                    None
                } else {
                    let path = self.save_path.trim();
                    if path.is_empty() {
                        self.update_status_with_missing(
                            "Choose a path to save the request (Ctrl/Cmd+S)",
                        );
                        return Task::none();
                    }
                    Some(PathBuf::from(path))
                };
                self.update_status_with_missing("Saving...");
                Task::perform(
                    async move {
                        persist_request(root, selection, draft, explicit_path)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    Message::Saved,
                )
            }
            Message::Saved(result) => match result {
                Ok((path, index)) => {
                    if let Some(RequestId::Unsaved(id)) = self.selection.clone() {
                        self.unsaved_tabs.retain(|tab| tab.id != id);
                    }
                    let id = RequestId::HttpFile {
                        path: path.clone(),
                        index,
                    };
                    self.selection = Some(id);
                    self.update_status_with_missing(&format!("Saved to {}", path.display()));
                    Task::batch([Task::none(), self.rescan_files()])
                }
                Err(err) => {
                    self.update_status_with_missing(&format!("Save failed: {err}"));
                    Task::none()
                }
            },
            Message::SavePathChanged(path) => {
                self.save_path = path;
                Task::none()
            }
        }
    }

    fn rescan_files(&self) -> Task<Message> {
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

    fn persist_state(&self) {
        let mut state = self.state.clone();
        state.http_root = Some(self.http_root.clone());
        state.save();
    }

    fn apply_saved_environment(&mut self) {
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

    fn apply_selection(&mut self, id: &RequestId) {
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

    fn set_header_rows_from_draft(&mut self) {
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

    fn rebuild_headers_from_rows(&mut self) {
        let lines: Vec<String> = self
            .header_rows
            .iter()
            .filter(|row| !row.name.trim().is_empty())
            .map(|row| format!("{}: {}", row.name.trim(), row.value.trim()))
            .collect();
        self.draft.headers = lines.join("\n");
    }

    fn update_response_viewer(&mut self) {
        let body_text = self
            .last_response
            .as_ref()
            .and_then(|resp| resp.error.clone().or_else(|| resp.body.clone()))
            .unwrap_or_else(|| "No response yet".to_string());
        let display_text = match (
            self.response_display,
            crate::app::view::pretty_json(&body_text),
        ) {
            (crate::app::view::ResponseDisplay::Pretty, Some(pretty)) => pretty,
            _ => body_text,
        };
        self.response_viewer = iced::widget::text_editor::Content::with_text(&display_text);
    }

    fn update_status_with_missing(&mut self, base: &str) {
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

fn default_environment() -> Environment {
    Environment {
        name: "No environment".to_string(),
        vars: BTreeMap::new(),
    }
}

fn with_default_environment(mut envs: Vec<Environment>) -> Vec<Environment> {
    let mut all = Vec::with_capacity(envs.len() + 1);
    all.push(default_environment());
    all.append(&mut envs);
    all
}

fn status_with_missing(
    base: &str,
    draft: &RequestDraft,
    env: Option<&Environment>,
    extra_inputs: &[&str],
) -> String {
    let missing = missing_env_vars(draft, env, extra_inputs);
    if missing.is_empty() {
        base.to_string()
    } else {
        let env_name = env.map(|e| e.name.as_str()).unwrap_or("environment");
        format!("{base} — Missing variables in {env_name}: {}", missing.join(", "))
    }
}

fn missing_env_vars(
    draft: &RequestDraft,
    env: Option<&Environment>,
    extra_inputs: &[&str],
) -> Vec<String> {
    let mut placeholders = BTreeSet::new();
    for text in [&draft.url, &draft.headers, &draft.body] {
        for name in collect_placeholders(text) {
            placeholders.insert(name);
        }
    }
    for text in extra_inputs {
        for name in collect_placeholders(text) {
            placeholders.insert(name);
        }
    }

    let env_vars = env.map(|e| &e.vars);
    placeholders
        .into_iter()
        .filter(|name| env_vars.map_or(true, |vars| !vars.contains_key(name)))
        .collect()
}

fn collect_placeholders(input: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut search_start = 0;

    while let Some(open_rel) = input[search_start..].find("{{") {
        let open = search_start + open_rel;
        let after_open = open + 2;
        if let Some(close_rel) = input[after_open..].find("}}") {
            let close = after_open + close_rel;
            let candidate = input[after_open..close].trim();
            if !candidate.is_empty() {
                names.push(candidate.to_string());
            }
            search_start = close + 2;
        } else {
            break;
        }
    }

    names
}
