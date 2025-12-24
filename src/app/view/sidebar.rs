use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use iced::widget::{Space, button, column, row, scrollable, text};
use iced::{Element, Length};

use super::super::Message;
use crate::model::{Collection, HttpFile, RequestDraft, RequestId};

const INDENT: i16 = 10;

#[derive(Default)]
struct TreeNode {
    children: BTreeMap<String, Self>,
    requests: Vec<RequestItem>,
    file_path: Option<PathBuf>,
}

struct RequestItem {
    id: RequestId,
    draft: RequestDraft,
}

pub fn sidebar<'a>(
    collections: &'a [Collection],
    http_files: &'a HashMap<PathBuf, HttpFile>,
    selection: Option<&RequestId>,
    collapsed: &BTreeSet<String>,
    http_root: &Path,
) -> Element<'a, Message> {
    let mut items = column![
        row![
            text("Requests").size(18),
            button("Add").on_press(Message::AddRequest)
        ]
        .spacing(6)
    ];

    let mut tree = TreeNode::default();

    for (idx, collection) in collections.iter().enumerate() {
        let segments: Vec<&str> = collection
            .name
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();
        insert_collection(
            &mut tree,
            &segments,
            None,
            collection
                .requests
                .iter()
                .enumerate()
                .map(|(r_idx, draft)| RequestItem {
                    id: RequestId::Collection {
                        collection: idx,
                        index: r_idx,
                    },
                    draft: draft.clone(),
                }),
        );
    }

    for file in http_files.values() {
        let rel_path = file.path.strip_prefix(http_root).unwrap_or(&file.path);
        let mut segments: Vec<String> = rel_path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();
        if let Some(last) = segments.last_mut()
            && let Some(stem) = Path::new(last).file_stem().and_then(|s| s.to_str())
        {
            *last = stem.to_string();
        }
        insert_collection(
            &mut tree,
            &segments.iter().map(String::as_str).collect::<Vec<_>>(),
            Some(&file.path),
            file.requests
                .iter()
                .enumerate()
                .map(|(r_idx, draft)| RequestItem {
                    id: RequestId::HttpFile {
                        path: file.path.clone(),
                        index: r_idx,
                    },
                    draft: draft.clone(),
                }),
        );
    }

    items = items.push(text("Collections").size(14));
    items = render_tree(items, &tree, "", 0, selection, collapsed);

    scrollable(items.spacing(4).padding(6))
        .width(Length::Fill)
        .into()
}

fn insert_collection(
    root: &mut TreeNode,
    segments: &[&str],
    file_path: Option<&PathBuf>,
    requests: impl Iterator<Item = RequestItem>,
) {
    if segments.is_empty() {
        return;
    }

    let mut node = root;
    for segment in &segments[..segments.len() - 1] {
        node = node.children.entry((*segment).to_string()).or_default();
    }
    let leaf = node
        .children
        .entry(segments[segments.len() - 1].to_string())
        .or_default();
    if leaf.file_path.is_none() {
        leaf.file_path = file_path.cloned();
    }
    leaf.requests.extend(requests);
}

fn render_tree<'a>(
    mut column: iced::widget::Column<'a, Message>,
    node: &TreeNode,
    path: &str,
    depth: usize,
    selection: Option<&RequestId>,
    collapsed: &BTreeSet<String>,
) -> iced::widget::Column<'a, Message> {
    let mut children: Vec<(String, &TreeNode)> = node
        .children
        .iter()
        .map(|(name, child)| (name.clone(), child))
        .collect();
    children.sort_by(|a, b| a.0.cmp(&b.0));

    for (name, child) in children {
        let full_path = if path.is_empty() {
            name.clone()
        } else {
            format!("{path}/{name}")
        };
        let is_collapsed = collapsed.contains(&full_path);
        let toggle_label = if is_collapsed { "▶" } else { "▼" };
        let toggle = button(text(toggle_label))
            .style(button::secondary)
            .padding(2)
            .on_press(Message::ToggleCollection(full_path.clone()));

        let mut row_widgets = row![Space::new().width(Length::Fixed(indent_px(depth))), toggle];

        if let Some(file_path) = &child.file_path {
            let is_selected = selection
                .and_then(|id| match id {
                    RequestId::HttpFile { path, .. } => Some(path),
                    RequestId::Collection { .. } => None,
                })
                .is_some_and(|p| p == file_path);

            let select_id = RequestId::HttpFile {
                path: file_path.clone(),
                index: 0,
            };

            let select_button = button(text(name).size(14))
                .style(if is_selected {
                    button::primary
                } else {
                    button::secondary
                })
                .width(Length::Fill)
                .on_press(Message::Select(select_id));
            row_widgets = row_widgets.push(select_button);
        } else {
            row_widgets = row_widgets.push(text(name).size(14));
        }

        column = column.push(row_widgets.spacing(4));

        if !is_collapsed {
            column = render_tree(column, child, &full_path, depth + 1, selection, collapsed);
        }
    }

    if !node.requests.is_empty() {
        for item in &node.requests {
            let is_selected = selection.is_some_and(|s| *s == item.id);
            let label = if is_selected {
                format!("▶ {} • {}", item.draft.method, item.draft.title)
            } else {
                format!("{} • {}", item.draft.method, item.draft.title)
            };
            column = column.push(
                row![
                    Space::new().width(Length::Fixed(indent_px(depth + 1))),
                    button(text(label))
                        .width(Length::Fill)
                        .on_press(Message::Select(item.id.clone())),
                ]
                .spacing(4),
            );
        }
    }

    column
}

fn indent_px(depth: usize) -> f32 {
    let depth_i16 = i16::try_from(depth).unwrap_or(i16::MAX);
    f32::from(depth_i16.saturating_mul(INDENT))
}
