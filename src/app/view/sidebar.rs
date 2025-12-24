use std::collections::HashMap;
use std::path::PathBuf;

use iced::widget::{button, column, row, scrollable, text};
use iced::{Element, Length};

use super::super::Message;
use crate::model::{Collection, HttpFile, RequestId, UnsavedTab};

pub fn sidebar<'a>(
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
                        .and_then(std::ffi::OsStr::to_str)
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
