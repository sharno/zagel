use std::path::PathBuf;

use iced::widget::{
    button, column, container, horizontal_rule, pick_list, row, scrollable, text, text_editor,
    text_input,
};
use iced::{Element, Length};

use super::{Message, Zagel};
use crate::model::{Collection, HttpFile, Method, RequestId, ResponsePreview, UnsavedTab};

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

    let headers_editor = text_editor(&app.headers_editor)
        .on_action(Message::HeadersEdited)
        .height(Length::Fixed(140.0));
    let body_editor = text_editor(&app.body_editor)
        .on_action(Message::BodyEdited)
        .height(Length::Fixed(200.0));

    let response_view = build_response(app.last_response.as_ref());

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

    let workspace = column![
        row![env_pick, title_input].spacing(12),
        save_path_row,
        row![
            method_pick,
            url_input,
            button("Save").on_press(Message::Save),
            button("Send").on_press(Message::Send)
        ]
        .spacing(8),
        horizontal_rule(1),
        text("Headers"),
        headers_editor,
        text("Body"),
        body_editor,
        horizontal_rule(1),
        text(format!("Status: {}", app.status_line.clone())),
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

            let mut headers_view = column![];
            if resp.headers.is_empty() {
                headers_view = headers_view.push(text("No headers").size(12));
            } else {
                for (name, value) in &resp.headers {
                    headers_view = headers_view.push(text(format!("{name}: {value}")).size(12));
                }
            }

            container(
                column![
                    text(header).size(16),
                    horizontal_rule(1),
                    text("Headers").size(14),
                    headers_view.spacing(4),
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
