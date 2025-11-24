use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::time::Duration;

use iced::widget::{
    button, column, container, horizontal_rule, pick_list, row, scrollable, text, text_editor,
    text_input,
};
use iced::{Element, Length, Subscription, Task, Theme, application, time};
use reqwest::Client;

use crate::model::{
    Collection, Environment, HttpFile, Method, RequestDraft, RequestId, ResponsePreview, UnsavedTab,
};
use crate::net::send_request;
use crate::parser::{scan_env_files, scan_http_files};
use crate::state::AppState;

const FILE_SCAN_MAX_DEPTH: usize = 6;
const FILE_SCAN_COOLDOWN: Duration = Duration::from_secs(2);

pub fn run() -> iced::Result {
    application("Zagel • REST workbench", Zagel::update, Zagel::view)
        .subscription(Zagel::subscription)
        .theme(Zagel::theme)
        .run_with(Zagel::init)
}

#[derive(Debug, Clone)]
enum Message {
    HttpFilesLoaded(HashMap<PathBuf, HttpFile>),
    EnvironmentsLoaded(Vec<Environment>),
    Tick,
    Select(RequestId),
    MethodSelected(Method),
    UrlChanged(String),
    TitleChanged(String),
    HeadersEdited(text_editor::Action),
    BodyEdited(text_editor::Action),
    AddUnsavedTab,
    Send,
    ResponseReady(Result<ResponsePreview, String>),
    EnvironmentChanged(String),
}

struct Zagel {
    collections: Vec<Collection>,
    http_files: HashMap<PathBuf, HttpFile>,
    unsaved_tabs: Vec<UnsavedTab>,
    selection: Option<RequestId>,
    draft: RequestDraft,
    headers_editor: text_editor::Content,
    body_editor: text_editor::Content,
    status_line: String,
    last_response: Option<ResponsePreview>,
    environments: Vec<Environment>,
    active_environment: usize,
    http_root: PathBuf,
    state: AppState,
    client: Client,
    next_unsaved_id: u32,
}

impl Zagel {
    fn init() -> (Self, Task<Message>) {
        let state = AppState::load();
        let http_root = state
            .http_root
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let app = Self {
            collections: Vec::new(),
            http_files: HashMap::new(),
            unsaved_tabs: Vec::new(),
            selection: None,
            draft: RequestDraft::default(),
            headers_editor: text_editor::Content::with_text(""),
            body_editor: text_editor::Content::with_text(""),
            status_line: "Ready".to_string(),
            last_response: None,
            environments: vec![default_environment()],
            active_environment: 0,
            http_root,
            state,
            client: Client::new(),
            next_unsaved_id: 1,
        };

        let task = app.rescan_files();
        app.persist_state();
        (app, task)
    }

    fn subscription(_state: &Self) -> Subscription<Message> {
        time::every(FILE_SCAN_COOLDOWN).map(|_| Message::Tick)
    }

    const fn theme(_: &Self) -> Theme {
        Theme::Nord
    }

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
                Task::none()
            }
            Message::TitleChanged(title) => {
                self.draft.title = title;
                Task::none()
            }
            Message::HeadersEdited(action) => {
                self.headers_editor.perform(action);
                self.draft.headers = self.headers_editor.text();
                Task::none()
            }
            Message::BodyEdited(action) => {
                self.body_editor.perform(action);
                self.draft.body = self.body_editor.text();
                Task::none()
            }
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
                let draft = self.draft.clone();
                self.status_line = "Sending...".to_string();
                Task::perform(
                    send_request(self.client.clone(), draft, env),
                    Message::ResponseReady,
                )
            }
            Message::ResponseReady(result) => {
                match result {
                    Ok(resp) => {
                        self.status_line = "Received response".to_string();
                        self.last_response = Some(resp);
                    }
                    Err(err) => {
                        self.status_line = "Request failed".to_string();
                        self.last_response = Some(ResponsePreview::error(err));
                    }
                }
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
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let sidebar = build_sidebar(
            &self.unsaved_tabs,
            &self.collections,
            &self.http_files,
            self.selection.as_ref(),
        );

        let env_pick = pick_list(
            self.environments
                .iter()
                .map(|e| e.name.clone())
                .collect::<Vec<_>>(),
            Some(self.environments[self.active_environment].name.clone()),
            Message::EnvironmentChanged,
        );

        let method_pick = pick_list(
            Method::ALL.to_vec(),
            Some(self.draft.method),
            Message::MethodSelected,
        );

        let url_input = text_input("https://api.example.com", &self.draft.url)
            .on_input(Message::UrlChanged)
            .padding(8)
            .width(Length::Fill);

        let title_input = text_input("Title", &self.draft.title)
            .on_input(Message::TitleChanged)
            .padding(6)
            .width(Length::Fill);

        let headers_editor = text_editor(&self.headers_editor)
            .on_action(Message::HeadersEdited)
            .height(Length::Fixed(140.0));
        let body_editor = text_editor(&self.body_editor)
            .on_action(Message::BodyEdited)
            .height(Length::Fixed(200.0));

        let response_view = build_response(self.last_response.as_ref());

        let workspace = column![
            row![env_pick, title_input].spacing(12),
            row![
                method_pick,
                url_input,
                button("Send").on_press(Message::Send)
            ]
            .spacing(8),
            horizontal_rule(1),
            text("Headers"),
            headers_editor,
            text("Body"),
            body_editor,
            horizontal_rule(1),
            text(format!("Status: {}", self.status_line.clone())),
            response_view,
        ]
        .padding(12)
        .spacing(8);

        row![
            container(sidebar).width(Length::Fixed(260.0)),
            container(workspace).width(Length::Fill)
        ]
        .spacing(12)
        .into()
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
        self.headers_editor = text_editor::Content::with_text(&draft.headers);
        self.body_editor = text_editor::Content::with_text(&draft.body);
    }
}

fn build_sidebar<'a>(
    unsaved_tabs: &[UnsavedTab],
    collections: &'a [Collection],
    http_files: &'a HashMap<PathBuf, HttpFile>,
    selection: Option<&RequestId>,
) -> Element<'a, Message> {
    let mut items = column![
        row![
            text("Requests").size(18),
            button("+").on_press(Message::AddUnsavedTab)
        ]
        .spacing(8)
    ];

    if !unsaved_tabs.is_empty() {
        items = items.push(text("Unsaved tabs").size(14));
        for tab in unsaved_tabs {
            let is_selected = selection.is_some_and(|id| *id == RequestId::Unsaved(tab.id));
            let label = if is_selected {
                format!("▶ {}", tab.title)
            } else {
                tab.title.clone()
            };
            items = items.push(
                button(text(label))
                    .width(Length::Fill)
                    .on_press(Message::Select(RequestId::Unsaved(tab.id))),
            );
        }
    }

    if !http_files.is_empty() {
        items = items.push(text("HTTP files").size(14));
        for file in http_files.values() {
            for (idx, req) in file.requests.iter().enumerate() {
                let id = RequestId::HttpFile {
                    path: file.path.clone(),
                    index: idx,
                };
                let is_selected = selection.is_some_and(|s| *s == id);
                let label = format!(
                    "{} • {}",
                    req.title,
                    file.path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_default()
                );
                let label = if is_selected {
                    format!("▶ {label}")
                } else {
                    label
                };
                items = items.push(
                    button(text(label))
                        .width(Length::Fill)
                        .on_press(Message::Select(id)),
                );
            }
        }
    }

    for (c_idx, collection) in collections.iter().enumerate() {
        items = items.push(text(&collection.name).size(14));
        for (r_idx, req) in collection.requests.iter().enumerate() {
            let id = RequestId::Collection {
                collection: c_idx,
                index: r_idx,
            };
            let is_selected = selection.is_some_and(|s| *s == id);
            let label = if is_selected {
                format!("▶ {} • {}", req.method, req.title)
            } else {
                format!("{} • {}", req.method, req.title)
            };
            items = items.push(
                button(text(label))
                    .width(Length::Fill)
                    .on_press(Message::Select(id)),
            );
        }
    }

    scrollable(items.spacing(6).padding(8))
        .width(Length::Fill)
        .into()
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

fn build_response(response: Option<&ResponsePreview>) -> Element<'static, Message> {
    response.map_or_else(
        || container(text("No response yet")).padding(8).into(),
        |resp| {
            let header = match (resp.status, resp.duration) {
                (Some(status), Some(duration)) => {
                    format!("HTTP {status} in {} ms", duration.as_millis())
                }
                (Some(status), None) => format!("HTTP {status}"),
                _ => "No response".to_string(),
            };

            let body_text = resp
                .error
                .clone()
                .or_else(|| resp.body.clone())
                .unwrap_or_else(|| "No body".to_string());

            container(
                column![
                    text(header).size(16),
                    horizontal_rule(1),
                    text(body_text)
                        .size(14)
                        .width(Length::Fill)
                        .shaping(iced::widget::text::Shaping::Advanced),
                ]
                .spacing(8),
            )
            .padding(8)
            .into()
        },
    )
}
