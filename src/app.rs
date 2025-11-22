use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use iced::executor;
use iced::widget::{
    button, column, container, horizontal_rule, pick_list, row, scrollable, text, text_editor,
    text_input,
};
use iced::{time, Application, Command, Element, Length, Settings, Subscription, Theme};
use reqwest::Client;

use crate::model::{
    Collection, Environment, HttpFile, Method, RequestDraft, RequestId, ResponsePreview, UnsavedTab,
};
use crate::net::send_request;
use crate::parser::scan_http_files;

const HTTP_SCAN_MAX_DEPTH: usize = 6;
const HTTP_SCAN_COOLDOWN: Duration = Duration::from_secs(2);

pub fn run() -> iced::Result {
    Zagel::run(Settings::default())
}

#[derive(Debug, Clone)]
enum Message {
    HttpFilesLoaded(HashMap<PathBuf, HttpFile>),
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
    client: Client,
    next_unsaved_id: u32,
}

impl Application for Zagel {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Self::Message>) {
        let http_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        let mut app = Self {
            collections: demo_collections(),
            http_files: HashMap::new(),
            unsaved_tabs: Vec::new(),
            selection: None,
            draft: RequestDraft::default(),
            headers_editor: text_editor::Content::with_text(""),
            body_editor: text_editor::Content::with_text(""),
            status_line: "Ready".to_string(),
            last_response: None,
            environments: demo_environments(),
            active_environment: 0,
            http_root,
            client: Client::new(),
            next_unsaved_id: 1,
        };

        app.apply_selection(RequestId::Collection {
            collection: 0,
            index: 0,
        });
        let root = app.http_root.clone();

        (
            app,
            Command::perform(scan_http_files(root, HTTP_SCAN_MAX_DEPTH), Message::HttpFilesLoaded),
        )
    }

    fn title(&self) -> String {
        "Zagel • REST workbench".into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        time::every(HTTP_SCAN_COOLDOWN).map(|_| Message::Tick)
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::Tick => Command::perform(
                scan_http_files(self.http_root.clone(), HTTP_SCAN_MAX_DEPTH),
                Message::HttpFilesLoaded,
            ),
            Message::HttpFilesLoaded(files) => {
                self.http_files = files;
                Command::none()
            }
            Message::Select(id) => {
                self.apply_selection(id);
                Command::none()
            }
            Message::MethodSelected(method) => {
                self.draft.method = method;
                Command::none()
            }
            Message::UrlChanged(url) => {
                self.draft.url = url;
                Command::none()
            }
            Message::TitleChanged(title) => {
                self.draft.title = title;
                Command::none()
            }
            Message::HeadersEdited(action) => {
                self.headers_editor.perform(action);
                self.draft.headers = self.headers_editor.text();
                Command::none()
            }
            Message::BodyEdited(action) => {
                self.body_editor.perform(action);
                self.draft.body = self.body_editor.text();
                Command::none()
            }
            Message::AddUnsavedTab => {
                let id = self.next_unsaved_id;
                self.next_unsaved_id += 1;
                self.unsaved_tabs.push(UnsavedTab {
                    id,
                    title: format!("Unsaved {}", id),
                });
                self.apply_selection(RequestId::Unsaved(id));
                Command::none()
            }
            Message::Send => {
                let env = self.environments.get(self.active_environment).cloned();
                let draft = self.draft.clone();
                self.status_line = "Sending...".to_string();
                Command::perform(send_request(self.client.clone(), draft, env), |res| {
                    Message::ResponseReady(res)
                })
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
                Command::none()
            }
            Message::EnvironmentChanged(name) => {
                if let Some((idx, _)) = self
                    .environments
                    .iter()
                    .enumerate()
                    .find(|(_, env)| env.name == name)
                {
                    self.active_environment = idx;
                }
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let sidebar = build_sidebar(
            &self.unsaved_tabs,
            &self.collections,
            &self.http_files,
            self.selection.clone(),
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
            row![method_pick, url_input, button("Send").on_press(Message::Send)].spacing(8),
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
}

fn demo_collections() -> Vec<Collection> {
    vec![Collection {
        name: "Samples".to_string(),
        requests: vec![
            RequestDraft {
                title: "Hello world".to_string(),
                method: Method::Get,
                url: "https://httpbin.org/get".to_string(),
                headers: "Accept: application/json".to_string(),
                body: String::new(),
            },
            RequestDraft {
                title: "Post JSON".to_string(),
                method: Method::Post,
                url: "https://httpbin.org/post".to_string(),
                headers: "Content-Type: application/json".to_string(),
                body: r#"{"hello": "world"}"#.to_string(),
            },
        ],
    }]
}

fn demo_environments() -> Vec<Environment> {
    vec![
        Environment {
            name: "No environment".to_string(),
            vars: std::collections::BTreeMap::new(),
        },
        Environment {
            name: "Local dev".to_string(),
            vars: std::collections::BTreeMap::from([
                ("host".to_string(), "http://localhost:3000".to_string()),
                ("token".to_string(), "dev-token-123".to_string()),
            ]),
        },
    ]
}

fn build_sidebar(
    unsaved_tabs: &[UnsavedTab],
    collections: &[Collection],
    http_files: &HashMap<PathBuf, HttpFile>,
    selection: Option<RequestId>,
) -> Element<'static, Message> {
    let mut items = column![row![
        text("Requests").size(18),
        button("+").on_press(Message::AddUnsavedTab)
    ]
    .spacing(8)
    .align_items(iced::Alignment::Center)];

    if !unsaved_tabs.is_empty() {
        items = items.push(text("Unsaved tabs").size(14));
        for tab in unsaved_tabs {
            let is_selected = selection
                .as_ref()
                .map(|id| *id == RequestId::Unsaved(tab.id))
                .unwrap_or(false);
            items = items.push(
                button(text(tab.title.clone()))
                    .width(Length::Fill)
                    .style(if is_selected {
                        iced::theme::Button::Primary
                    } else {
                        iced::theme::Button::Secondary
                    })
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
                let is_selected = selection.as_ref().map(|s| *s == id).unwrap_or(false);
                let label = format!(
                    "{} • {}",
                    req.title,
                    file.path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_default()
                );
                items = items.push(
                    button(text(label))
                        .width(Length::Fill)
                        .style(if is_selected {
                            iced::theme::Button::Primary
                        } else {
                            iced::theme::Button::Secondary
                        })
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
            let is_selected = selection.as_ref().map(|s| *s == id).unwrap_or(false);
            items = items.push(
                button(text(format!("{} • {}", req.method, req.title)))
                    .width(Length::Fill)
                    .style(if is_selected {
                        iced::theme::Button::Primary
                    } else {
                        iced::theme::Button::Secondary
                    })
                    .on_press(Message::Select(id)),
            );
        }
    }

    scrollable(items.spacing(6).padding(8))
        .width(Length::Fill)
        .into()
}

fn build_response(response: Option<&ResponsePreview>) -> Element<'static, Message> {
    match response {
        Some(resp) => {
            let header = match (resp.status, resp.duration) {
                (Some(status), Some(duration)) => {
                    format!("HTTP {} in {} ms", status, duration.as_millis())
                }
                (Some(status), None) => format!("HTTP {}", status),
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
        }
        None => container(text("No response yet")).padding(8).into(),
    }
}

impl Zagel {
    fn apply_selection(&mut self, id: RequestId) {
        self.selection = Some(id.clone());
        let maybe_request = match &id {
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
