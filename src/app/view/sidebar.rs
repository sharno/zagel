use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Element, Length};

use super::section;
use super::super::{CollectionRef, EditTarget, Message};
use crate::model::{Collection, HttpFile, RequestDraft, RequestId};

const INDENT: i16 = 10;

#[derive(Clone, Copy)]
pub struct SidebarContext<'a> {
    pub collections: &'a [Collection],
    pub http_files: &'a HashMap<PathBuf, HttpFile>,
    pub http_file_order: &'a [PathBuf],
    pub selection: Option<&'a RequestId>,
    pub collapsed: &'a BTreeSet<String>,
    pub http_root: &'a Path,
    pub editing: bool,
    pub edit_selection: &'a HashSet<EditTarget>,
}

struct RenderContext<'a> {
    selection: Option<&'a RequestId>,
    collapsed: &'a BTreeSet<String>,
    editing: bool,
    edit_selection: &'a HashSet<EditTarget>,
}

#[derive(Default)]
struct TreeNode {
    children: Vec<TreeChild>,
    requests: Vec<RequestItem>,
    file_path: Option<PathBuf>,
    collection_index: Option<usize>,
}

struct TreeChild {
    name: String,
    node: TreeNode,
}

struct RequestItem {
    id: RequestId,
    draft: RequestDraft,
}

pub fn sidebar(ctx: SidebarContext<'_>) -> Element<'_, Message> {
    let mut header = row![
        text("Requests").size(20),
        button("Add").on_press(Message::AddRequest)
    ]
    .align_y(Alignment::Center)
    .spacing(6);
    if ctx.editing {
        let delete_button = if ctx.edit_selection.is_empty() {
            button("Delete")
        } else {
            button("Delete").on_press(Message::DeleteSelected)
        };
        header = header
            .push(delete_button)
            .push(button("Done").on_press(Message::ToggleEditMode));
    } else {
        header = header.push(button("Edit").on_press(Message::ToggleEditMode));
    }

    let mut tree = TreeNode::default();

    for (idx, collection) in ctx.collections.iter().enumerate() {
        let segments: Vec<&str> = collection
            .name
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();
        insert_collection(
            &mut tree,
            &segments,
            None,
            Some(idx),
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

    for path in ctx.http_file_order {
        let Some(file) = ctx.http_files.get(path) else {
            continue;
        };
        let rel_path = file.path.strip_prefix(ctx.http_root).unwrap_or(&file.path);
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
            None,
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

    let render_ctx = RenderContext {
        selection: ctx.selection,
        collapsed: ctx.collapsed,
        editing: ctx.editing,
        edit_selection: ctx.edit_selection,
    };
    let list = render_tree(column![], &tree, "", 0, &render_ctx).spacing(4);
    let collections_section = section("Collections", list.into());

    let list = scrollable(column![header, collections_section].spacing(10))
        .width(Length::Fill)
        .height(Length::Fill);

    container(list)
        .padding(8)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn insert_collection(
    root: &mut TreeNode,
    segments: &[&str],
    file_path: Option<&PathBuf>,
    collection_index: Option<usize>,
    requests: impl Iterator<Item = RequestItem>,
) {
    if segments.is_empty() {
        return;
    }

    let mut node = root;
    for segment in &segments[..segments.len() - 1] {
        node = child_mut(node, segment);
    }
    let leaf = child_mut(node, segments[segments.len() - 1]);
    if leaf.file_path.is_none() {
        leaf.file_path = file_path.cloned();
    }
    if leaf.collection_index.is_none() {
        leaf.collection_index = collection_index;
    }
    leaf.requests.extend(requests);
}

fn child_mut<'a>(node: &'a mut TreeNode, name: &str) -> &'a mut TreeNode {
    if let Some(pos) = node.children.iter().position(|child| child.name == name) {
        return &mut node.children[pos].node;
    }
    node.children.push(TreeChild {
        name: name.to_string(),
        node: TreeNode::default(),
    });
    let idx = node.children.len() - 1;
    &mut node.children[idx].node
}

fn render_tree<'a>(
    mut column: iced::widget::Column<'a, Message>,
    node: &TreeNode,
    path: &str,
    depth: usize,
    ctx: &RenderContext<'a>,
) -> iced::widget::Column<'a, Message> {
    for child in &node.children {
        let full_path = if path.is_empty() {
            child.name.clone()
        } else {
            format!("{path}/{}", child.name)
        };
        let is_collapsed = ctx.collapsed.contains(&full_path);
        let toggle_label = if is_collapsed { "▶" } else { "▼" };
        let toggle = button(text(toggle_label))
            .style(button::secondary)
            .padding(2)
            .on_press(Message::ToggleCollection(full_path.clone()));

        let mut row_widgets = row![Space::new().width(Length::Fixed(indent_px(depth))), toggle];

        let collection_ref = child
            .node
            .file_path
            .as_ref()
            .map(|file_path| CollectionRef::HttpFile(file_path.clone()))
            .or_else(|| {
                child
                    .node
                    .collection_index
                    .map(CollectionRef::CollectionIndex)
            });

        if ctx.editing && let Some(collection_ref) = collection_ref.clone() {
            let target = EditTarget::Collection(collection_ref.clone());
            let selected = ctx.edit_selection.contains(&target);
            let label = if selected { "[x]" } else { "[ ]" };
            row_widgets = row_widgets
                .push(button(text(label)).on_press(Message::ToggleEditSelection(target)))
                .push(button(text("^")).on_press(Message::MoveCollectionUp(collection_ref.clone())))
                .push(button(text("v")).on_press(Message::MoveCollectionDown(collection_ref)));
        }

        if let Some(file_path) = &child.node.file_path {
            let is_selected = ctx
                .selection
                .and_then(|id| match id {
                    RequestId::HttpFile { path, .. } => Some(path),
                    RequestId::Collection { .. } => None,
                })
                .is_some_and(|p| p == file_path);

            let select_id = RequestId::HttpFile {
                path: file_path.clone(),
                index: 0,
            };

            let select_button = button(text(child.name.clone()).size(14))
                .style(if is_selected {
                    button::primary
                } else {
                    button::secondary
                })
                .width(Length::Fill)
                .on_press(Message::Select(select_id));
            row_widgets = row_widgets.push(select_button);
        } else {
            row_widgets = row_widgets.push(text(child.name.clone()).size(14));
        }

        column = column.push(row_widgets.spacing(4));

        if !is_collapsed {
            column = render_tree(column, &child.node, &full_path, depth + 1, ctx);
        }
    }

    if !node.requests.is_empty() {
        for item in &node.requests {
            let is_selected = ctx.selection.is_some_and(|s| *s == item.id);
            let label = if is_selected {
                format!("▶ {} • {}", item.draft.method, item.draft.title)
            } else {
                format!("{} • {}", item.draft.method, item.draft.title)
            };
            let mut row_widgets =
                row![Space::new().width(Length::Fixed(indent_px(depth + 1)))];
            if ctx.editing {
                let target = EditTarget::Request(item.id.clone());
                let selected = ctx.edit_selection.contains(&target);
                let select_label = if selected { "[x]" } else { "[ ]" };
                row_widgets = row_widgets
                    .push(button(text(select_label)).on_press(Message::ToggleEditSelection(target)))
                    .push(button(text("^")).on_press(Message::MoveRequestUp(item.id.clone())))
                    .push(button(text("v")).on_press(Message::MoveRequestDown(item.id.clone())));
            }
            row_widgets = row_widgets.push(
                button(text(label))
                    .style(if is_selected {
                        button::primary
                    } else {
                        button::secondary
                    })
                    .width(Length::Fill)
                    .on_press(Message::Select(item.id.clone())),
            );
            column = column.push(row_widgets.spacing(4));
        }
    }

    column
}

fn indent_px(depth: usize) -> f32 {
    let depth_i16 = i16::try_from(depth).unwrap_or(i16::MAX);
    f32::from(depth_i16.saturating_mul(INDENT))
}
