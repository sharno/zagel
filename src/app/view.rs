use std::path::PathBuf;

use iced::widget::text::Wrapping;
use iced::widget::{
    button, column, container, pick_list, row, rule, scrollable, text, text_editor, text_input,
};
use iced::{Element, Length, Theme};
use iced_highlighter::Theme as HighlightTheme;

use super::headers;
use super::options::{AuthKind, AuthState, RequestMode};
use super::{Message, Zagel};
use crate::model::{Collection, HttpFile, Method, RequestId, ResponsePreview, UnsavedTab};

#[allow(clippy::too_many_lines)]
pub fn view(app: &Zagel) -> Element<'_, Message> {
    let sidebar = build_sidebar(
        &app.unsaved_tabs,
        &app.collections,
        &app.http_files,
        app.selection.as_ref(),
    );

    let env_pick = pick_list(
        app.environments
            .iter()
            .map(|e| e.name.clone())
            .collect::<Vec<_>>(),
        Some(app.environments[app.active_environment].name.clone()),
        Message::EnvironmentChanged,
    );

    let method_pick = pick_list(
        Method::ALL.to_vec(),
        Some(app.draft.method),
        Message::MethodSelected,
    );

    let url_input = text_input("https://api.example.com", &app.draft.url)
        .on_input(Message::UrlChanged)
        .padding(8)
        .width(Length::Fill);

    let title_input = text_input("Title", &app.draft.title)
        .on_input(Message::TitleChanged)
        .padding(6)
        .width(Length::Fill);

    let body_editor: iced::widget::TextEditor<'_, _, _, Theme> = text_editor(&app.body_editor)
        .on_action(Message::BodyEdited)
        .height(Length::Fixed(200.0));

    let response_view = build_response(
        app.last_response.as_ref(),
        &app.response_viewer,
        app.response_display,
        app.response_tab,
    );

    let save_path_row: Element<'_, Message> = match &app.selection {
        Some(RequestId::HttpFile { path, .. }) => row![
            text("Saving to").size(14),
            text(path.display().to_string()).size(14)
        ]
        .spacing(8)
        .into(),
        _ => row![
            text("Save as").size(14),
            text_input("path/to/request.http", &app.save_path)
                .on_input(Message::SavePathChanged)
                .padding(6)
                .width(Length::Fill),
        ]
        .spacing(8)
        .into(),
    };

    let mode_pick = pick_list(
        RequestMode::ALL.to_vec(),
        Some(app.mode),
        Message::ModeChanged,
    );

    let auth_editor = build_auth(&app.auth);

    let graphql_panel: Element<'_, Message> = match app.mode {
        RequestMode::GraphQl => {
            let query_editor: iced::widget::TextEditor<'_, _, _, Theme> =
                text_editor(&app.graphql_query)
                    .on_action(Message::GraphqlQueryEdited)
                    .height(Length::Fixed(180.0));
            let vars_editor: iced::widget::TextEditor<'_, _, _, Theme> =
                text_editor(&app.graphql_variables)
                    .on_action(Message::GraphqlVariablesEdited)
                    .height(Length::Fixed(120.0));
            column![
                text("GraphQL query"),
                query_editor,
                text("Variables (JSON)"),
                vars_editor,
            ]
            .spacing(6)
            .into()
        }
        RequestMode::Rest => column![text("Body"), body_editor].spacing(6).into(),
    };

    let mut status_row = row![
        text(format!("Status: {}", app.status_line.clone())),
        response_view_toggle(app.response_display),
        response_tab_toggle(app.response_tab),
    ]
    .spacing(12);

    if app.response_tab == ResponseTab::Body {
        status_row = status_row.push(button("Copy body").on_press(Message::CopyResponseBody));
    }

    let workspace = column![
        row![env_pick, title_input, mode_pick].spacing(12),
        save_path_row,
        row![
            method_pick,
            url_input,
            button("Save").on_press(Message::Save),
            button("Send").on_press(Message::Send)
        ]
        .spacing(8),
        rule::horizontal(1),
        text("Headers"),
        headers::editor(&app.header_rows),
        text("Auth"),
        auth_editor,
        rule::horizontal(1),
        graphql_panel,
        rule::horizontal(1),
        status_row,
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

fn build_sidebar<'a>(
    unsaved_tabs: &[UnsavedTab],
    collections: &'a [Collection],
    http_files: &'a std::collections::HashMap<PathBuf, HttpFile>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseDisplay {
    Raw,
    Pretty,
}

impl ResponseDisplay {
    pub const ALL: [Self; 2] = [Self::Raw, Self::Pretty];
}

impl std::fmt::Display for ResponseDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Raw => f.write_str("Raw"),
            Self::Pretty => f.write_str("Pretty"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseTab {
    Body,
    Headers,
}

impl ResponseTab {
    pub const ALL: [Self; 2] = [Self::Body, Self::Headers];
}

impl std::fmt::Display for ResponseTab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Body => f.write_str("Body"),
            Self::Headers => f.write_str("Headers"),
        }
    }
}

fn response_tab_toggle(current: ResponseTab) -> Element<'static, Message> {
    pick_list(
        ResponseTab::ALL.to_vec(),
        Some(current),
        Message::ResponseTabChanged,
    )
    .into()
}

fn response_view_toggle(current: ResponseDisplay) -> Element<'static, Message> {
    pick_list(
        ResponseDisplay::ALL.to_vec(),
        Some(current),
        Message::ResponseViewChanged,
    )
    .into()
}

fn build_response<'a>(
    response: Option<&ResponsePreview>,
    content: &'a text_editor::Content,
    display: ResponseDisplay,
    tab: ResponseTab,
) -> Element<'a, Message> {
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

            let mut headers_view = column![];
            if resp.headers.is_empty() {
                headers_view = headers_view.push(text("No headers").size(12));
            } else {
                for (name, value) in &resp.headers {
                    headers_view = headers_view.push(text(format!("{name}: {value}")).size(12));
                }
            }

            let body_is_pretty = pretty_json(&body_text).is_some();
            let syntax = response_syntax(resp);
            let body_editor = text_editor(content)
                .height(Length::Fixed(260.0))
                .highlight(syntax, HighlightTheme::SolarizedDark)
                .wrapping(Wrapping::None);

            let body_section: Element<'_, Message> = column![
                text(format!(
                    "Body ({})",
                    match display {
                        ResponseDisplay::Pretty if body_is_pretty => "pretty",
                        ResponseDisplay::Pretty => "pretty (raw shown)",
                        ResponseDisplay::Raw => "raw",
                    }
                ))
                .size(14),
                body_editor,
            ]
            .spacing(8)
            .into();

            let headers_section: Element<'_, Message> =
                column![text("Headers").size(14), headers_view.spacing(4),]
                    .spacing(8)
                    .into();

            let tab_view: Element<'_, Message> = match tab {
                ResponseTab::Body => body_section,
                ResponseTab::Headers => headers_section,
            };

            container(column![text(header).size(16), rule::horizontal(1), tab_view].spacing(8))
                .padding(8)
                .into()
        },
    )
}

pub fn pretty_json(raw: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .map(|v| serde_json::to_string_pretty(&v).unwrap_or_else(|_| raw.to_string()))
}

fn response_syntax(resp: &ResponsePreview) -> &'static str {
    let content_type = resp
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
        .map(|(_, value)| value.to_ascii_lowercase())
        .unwrap_or_default();

    if content_type.contains("json") {
        "json"
    } else if content_type.contains("html") {
        "html"
    } else if content_type.contains("xml") {
        "xml"
    } else if content_type.contains("javascript") {
        "javascript"
    } else if content_type.contains("css") {
        "css"
    } else {
        "text"
    }
}

fn build_auth(auth: &AuthState) -> Element<'_, Message> {
    let kind_pick = pick_list(AuthKind::ALL.to_vec(), Some(auth.kind), |kind| {
        Message::AuthChanged(AuthState {
            kind,
            ..auth.clone()
        })
    });

    let fields: Element<'_, Message> = match auth.kind {
        AuthKind::None => text("No authentication").into(),
        AuthKind::Bearer => text_input("Bearer token", &auth.bearer_token)
            .on_input(|val| {
                let mut new = auth.clone();
                new.bearer_token = val;
                Message::AuthChanged(new)
            })
            .padding(6)
            .width(Length::Fill)
            .into(),
        AuthKind::ApiKey => column![
            text_input("Header name", &auth.api_key_name).on_input(|val| {
                let mut new = auth.clone();
                new.api_key_name = val;
                Message::AuthChanged(new)
            }),
            text_input("Header value", &auth.api_key_value)
                .on_input(|val| {
                    let mut new = auth.clone();
                    new.api_key_value = val;
                    Message::AuthChanged(new)
                })
                .padding(6)
                .width(Length::Fill),
        ]
        .spacing(6)
        .into(),
        AuthKind::Basic => column![
            text_input("Username", &auth.basic_username).on_input(|val| {
                let mut new = auth.clone();
                new.basic_username = val;
                Message::AuthChanged(new)
            }),
            text_input("Password", &auth.basic_password)
                .on_input(|val| {
                    let mut new = auth.clone();
                    new.basic_password = val;
                    Message::AuthChanged(new)
                })
                .padding(6)
                .width(Length::Fill),
        ]
        .spacing(6)
        .into(),
    };

    column![kind_pick, fields].spacing(6).into()
}
