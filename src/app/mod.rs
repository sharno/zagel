mod hotkeys;
mod messages;
mod view;

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::time::Duration;

use iced::{application, time, Subscription, Task, Theme};
use reqwest::Client;

use crate::model::{
    Collection, Environment, HttpFile, RequestDraft, RequestId, ResponsePreview, UnsavedTab,
};
use crate::net::send_request;
use crate::parser::{persist_request, scan_env_files, scan_http_files, suggest_http_path};
use crate::state::AppState;
pub use messages::Message;

const FILE_SCAN_MAX_DEPTH: usize = 6;
const FILE_SCAN_COOLDOWN: Duration = Duration::from_secs(2);

pub fn run() -> iced::Result {
    application("Zagel • REST workbench", Zagel::update, view::view)
        .subscription(Zagel::subscription)
        .theme(Zagel::theme)
        .run_with(Zagel::init)
}

pub struct Zagel {
    pub(super) collections: Vec<Collection>,
    pub(super) http_files: HashMap<PathBuf, HttpFile>,
    pub(super) unsaved_tabs: Vec<UnsavedTab>,
    pub(super) selection: Option<RequestId>,
    pub(super) draft: RequestDraft,
    pub(super) headers_editor: iced::widget::text_editor::Content,
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
            headers_editor: iced::widget::text_editor::Content::with_text(""),
            body_editor: iced::widget::text_editor::Content::with_text(""),
            status_line: "Ready".to_string(),
            last_response: None,
            environments: vec![default_environment()],
            active_environment: 0,
            http_root,
            state,
            client: Client::new(),
            next_unsaved_id: 1,
            response_viewer: iced::widget::text_editor::Content::with_text(""),
            save_path: String::new(),
        };

        let task = app.rescan_files();
        app.persist_state();
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
                        let body_text = resp.body.clone().unwrap_or_else(|| "No body".to_string());
                        self.response_viewer =
                            iced::widget::text_editor::Content::with_text(&body_text);
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
            Message::Save => {
                let selection = self.selection.clone();
                let draft = self.draft.clone();
                let root = self.http_root.clone();
                let explicit_path = if let Some(RequestId::HttpFile { .. }) = selection {
                    None
                } else {
                    let path = self.save_path.trim();
                    if path.is_empty() {
                        self.status_line =
                            "Choose a path to save the request (Ctrl/Cmd+S)".to_string();
                        return Task::none();
                    }
                    Some(PathBuf::from(path))
                };
                self.status_line = "Saving...".to_string();
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
                    self.status_line = format!("Saved to {}", path.display());
                    Task::batch([Task::none(), self.rescan_files()])
                }
                Err(err) => {
                    self.status_line = format!("Save failed: {err}");
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
        self.headers_editor = iced::widget::text_editor::Content::with_text(&draft.headers);
        self.body_editor = iced::widget::text_editor::Content::with_text(&draft.body);
        self.save_path = match id {
            RequestId::HttpFile { path, .. } => path.display().to_string(),
            _ => suggest_http_path(&self.http_root, &draft.title)
                .display()
                .to_string(),
        };
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
