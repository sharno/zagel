use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length};

use crate::pathing::{GlobalEnvRoot, ProjectRoot};
use crate::theme;

use super::super::{EditState, EditTarget, Message};
use super::section;
use crate::model::{HttpFile, RequestDraft, RequestId};

const INDENT: i16 = 14;

#[derive(Debug, Clone, Copy)]
pub enum IconSet {
    Unicode,
    Ascii,
}

impl IconSet {
    pub fn from_env() -> Self {
        match std::env::var("ZAGEL_ICON_SET")
            .ok()
            .map(|value| value.to_ascii_lowercase())
        {
            Some(value) if value == "ascii" => Self::Ascii,
            Some(value) if value == "unicode" => Self::Unicode,
            _ => Self::Unicode,
        }
    }

    pub const fn icons(self) -> Icons {
        match self {
            Self::Unicode => Icons {
                collapsed: "▸",
                expanded: "▾",
                checked: "☑",
                unchecked: "☐",
                move_up: "↑",
                move_down: "↓",
                selected: "→",
            },
            Self::Ascii => Icons {
                collapsed: ">",
                expanded: "v",
                checked: "[x]",
                unchecked: "[ ]",
                move_up: "^",
                move_down: "v",
                selected: ">",
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Icons {
    collapsed: &'static str,
    expanded: &'static str,
    checked: &'static str,
    unchecked: &'static str,
    move_up: &'static str,
    move_down: &'static str,
    selected: &'static str,
}

#[derive(Clone, Copy)]
pub struct SidebarContext<'a> {
    pub http_files: &'a HashMap<PathBuf, HttpFile>,
    pub http_file_order: &'a [PathBuf],
    pub selection: Option<&'a RequestId>,
    pub collapsed: &'a BTreeSet<String>,
    pub project_roots: &'a [ProjectRoot],
    pub global_env_roots: &'a [GlobalEnvRoot],
    pub project_path_input: &'a str,
    pub global_env_path_input: &'a str,
    pub edit_state: &'a EditState,
    pub icon_set: IconSet,
}

struct RenderContext<'a> {
    selection: Option<&'a RequestId>,
    collapsed: &'a BTreeSet<String>,
    editing: bool,
    edit_selection: Option<&'a HashSet<EditTarget>>,
    icons: Icons,
}

#[derive(Default)]
struct TreeNode {
    children: Vec<TreeChild>,
    requests: Vec<RequestItem>,
    file_path: Option<PathBuf>,
}

struct TreeChild {
    name: String,
    node: TreeNode,
}

struct RequestItem {
    id: RequestId,
    draft: RequestDraft,
}

#[allow(clippy::too_many_lines)]
pub fn sidebar(ctx: SidebarContext<'_>) -> Element<'_, Message> {
    let (editing, edit_selection) = match ctx.edit_state {
        EditState::On { selection } => (true, Some(selection)),
        EditState::Off => (false, None),
    };
    let icons = ctx.icon_set.icons();

    let project_input = text_input("C:/path/to/project", ctx.project_path_input)
        .on_input(Message::ProjectPathInputChanged)
        .padding(6)
        .width(Length::FillPortion(4));
    let add_project = button("Add project")
        .on_press(Message::AddProject)
        .padding([5, 10]);

    let mut project_roots = column![row![project_input, add_project].spacing(6)].spacing(6);
    if ctx.project_roots.is_empty() {
        project_roots = project_roots.push(text("No projects configured").size(12));
    } else {
        for root in ctx.project_roots {
            project_roots = project_roots.push(
                row![
                    text(root.as_path().display().to_string()).size(12),
                    button("Remove")
                        .on_press(Message::RemoveProject(root.clone()))
                        .padding([3, 8]),
                ]
                .align_y(Alignment::Center)
                .spacing(6),
            );
        }
    }

    let global_env_input = text_input("C:/path/to/global/envs", ctx.global_env_path_input)
        .on_input(Message::GlobalEnvPathInputChanged)
        .padding(6)
        .width(Length::FillPortion(4));
    let add_global = button("Add global envs")
        .on_press(Message::AddGlobalEnvRoot)
        .padding([5, 10]);

    let mut global_env_roots = column![row![global_env_input, add_global].spacing(6)].spacing(6);
    if ctx.global_env_roots.is_empty() {
        global_env_roots = global_env_roots.push(text("No global env folders").size(12));
    } else {
        for root in ctx.global_env_roots {
            global_env_roots = global_env_roots.push(
                row![
                    text(root.as_path().display().to_string()).size(12),
                    button("Remove")
                        .on_press(Message::RemoveGlobalEnvRoot(root.clone()))
                        .padding([3, 8]),
                ]
                .align_y(Alignment::Center)
                .spacing(6),
            );
        }
    }

    let mut header = row![
        text("Requests").size(16),
        button("Add")
            .on_press(Message::AddRequest)
            .padding([4, 10])
    ]
    .align_y(Alignment::Center)
    .spacing(6);
    if editing {
        let selection_empty = edit_selection.is_none_or(HashSet::is_empty);
        let delete_button = if selection_empty {
            button("Delete").padding([4, 10])
        } else {
            button("Delete")
                .on_press(Message::DeleteSelected)
                .padding([4, 10])
        };
        header = header
            .push(delete_button)
            .push(
                button("Done")
                    .on_press(Message::ToggleEditMode)
                    .padding([4, 10]),
            );
    } else {
        header = header.push(
            button("Edit")
                .on_press(Message::ToggleEditMode)
                .padding([4, 10]),
        );
    }

    let mut tree = TreeNode::default();
    for root in ctx.project_roots {
        let label = root.as_path().display().to_string();
        insert_collection(
            &mut tree,
            &[label.as_str()],
            None,
            std::iter::empty::<RequestItem>(),
        );
    }

    for path in ctx.http_file_order {
        let Some(file) = ctx.http_files.get(path) else {
            continue;
        };
        let Some(project_root) = project_root_for_file(&file.path, ctx.project_roots) else {
            continue;
        };
        let rel_path = file
            .path
            .strip_prefix(project_root.as_path())
            .unwrap_or(&file.path);
        let mut segments: Vec<String> = rel_path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();
        segments.insert(0, project_root.as_path().display().to_string());
        if let Some(last) = segments.last_mut()
            && let Some(stem) = std::path::Path::new(last)
                .file_stem()
                .and_then(|s| s.to_str())
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

    let render_ctx = RenderContext {
        selection: ctx.selection,
        collapsed: ctx.collapsed,
        editing,
        edit_selection,
        icons,
    };
    let list = render_tree(column![], &tree, "", 0, &render_ctx).spacing(3);
    let project_section = section("Projects", project_roots.into());
    let global_env_section = section("Global Environments", global_env_roots.into());
    let collections_section = section("Collections", list.into());

    let list = scrollable(
        column![
            project_section,
            global_env_section,
            header,
            collections_section
        ]
        .spacing(10),
    )
    .width(Length::Fill)
    .height(Length::Fill);

    container(list)
        .padding(10)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::sidebar_container_style)
        .into()
}

fn project_root_for_file<'a>(
    file_path: &std::path::Path,
    project_roots: &'a [ProjectRoot],
) -> Option<&'a ProjectRoot> {
    project_roots
        .iter()
        .filter(|root| file_path.starts_with(root.as_path()))
        .max_by_key(|root| root.as_path().components().count())
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
        node = child_mut(node, segment);
    }
    let leaf = child_mut(node, segments[segments.len() - 1]);
    if leaf.file_path.is_none() {
        leaf.file_path = file_path.cloned();
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
        column = render_child(column, child, path, depth, ctx);
    }

    render_requests(column, node, depth, ctx)
}

fn indent_px(depth: usize) -> f32 {
    let depth_i16 = i16::try_from(depth).unwrap_or(i16::MAX);
    f32::from(depth_i16.saturating_mul(INDENT))
}

fn render_child<'a>(
    mut column: iced::widget::Column<'a, Message>,
    child: &TreeChild,
    path: &str,
    depth: usize,
    ctx: &RenderContext<'a>,
) -> iced::widget::Column<'a, Message> {
    let full_path = if path.is_empty() {
        child.name.clone()
    } else {
        format!("{path}/{}", child.name)
    };
    let is_collapsed = ctx.collapsed.contains(&full_path);
    let row_widgets = collection_row(child, depth, ctx, &full_path, is_collapsed);
    column = column.push(row_widgets);

    if !is_collapsed {
        column = render_tree(column, &child.node, &full_path, depth + 1, ctx);
    }

    column
}

fn render_requests<'a>(
    mut column: iced::widget::Column<'a, Message>,
    node: &TreeNode,
    depth: usize,
    ctx: &RenderContext<'a>,
) -> iced::widget::Column<'a, Message> {
    for item in &node.requests {
        let row_widgets = request_row(item, depth, ctx);
        column = column.push(row_widgets);
    }

    column
}

fn collection_row<'a>(
    child: &TreeChild,
    depth: usize,
    ctx: &RenderContext<'a>,
    full_path: &str,
    is_collapsed: bool,
) -> iced::widget::Row<'a, Message> {
    let toggle_label = if is_collapsed {
        ctx.icons.collapsed
    } else {
        ctx.icons.expanded
    };
    let toggle = button(text(toggle_label))
        .style(button::secondary)
        .padding([3, 6])
        .on_press(Message::ToggleCollection(full_path.to_string()));

    let mut row_widgets = row![Space::new().width(Length::Fixed(indent_px(depth))), toggle];

    let collection_path = child.node.file_path.clone();
    if ctx.editing
        && let (Some(edit_selection), Some(collection_path)) = (ctx.edit_selection, collection_path)
    {
        let target = EditTarget::Collection(collection_path.clone());
        let label = if edit_selection.contains(&target) {
            ctx.icons.checked
        } else {
            ctx.icons.unchecked
        };
        row_widgets = row_widgets
            .push(
                button(text(label))
                    .padding([3, 6])
                    .on_press(Message::ToggleEditSelection(target)),
            )
            .push(
                button(text(ctx.icons.move_up))
                    .padding([3, 6])
                    .on_press(Message::MoveCollectionUp(collection_path.clone())),
            )
            .push(
                button(text(ctx.icons.move_down))
                    .padding([3, 6])
                    .on_press(Message::MoveCollectionDown(collection_path)),
            );
    }

    row_widgets = if let Some(file_path) = &child.node.file_path {
        let is_selected = ctx
            .selection
            .is_some_and(|id| matches!(id, RequestId::HttpFile { path, .. } if path == file_path));

        let select_id = RequestId::HttpFile {
            path: file_path.clone(),
            index: 0,
        };

        row_widgets.push(
            button(text(child.name.clone()).size(13))
                .style(if is_selected {
                    button::primary
                } else {
                    button::secondary
                })
                .padding([4, 8])
                .width(Length::Fill)
                .on_press(Message::Select(select_id)),
        )
    } else {
        row_widgets.push(text(child.name.clone()).size(13))
    };

    row_widgets
        .spacing(4)
        .align_y(Alignment::Center)
}

fn request_row<'a>(
    item: &RequestItem,
    depth: usize,
    ctx: &RenderContext<'a>,
) -> iced::widget::Row<'a, Message> {
    let is_selected = ctx.selection.is_some_and(|s| *s == item.id);
    let label = if is_selected {
        format!(
            "{} {} • {}",
            ctx.icons.selected, item.draft.method, item.draft.title
        )
    } else {
        format!("{} • {}", item.draft.method, item.draft.title)
    };
    let mut row_widgets = row![Space::new().width(Length::Fixed(indent_px(depth + 1)))];
    if ctx.editing
        && let Some(edit_selection) = ctx.edit_selection
    {
        let target = EditTarget::Request(item.id.clone());
        let select_label = if edit_selection.contains(&target) {
            ctx.icons.checked
        } else {
            ctx.icons.unchecked
        };
        row_widgets = row_widgets
            .push(
                button(text(select_label))
                    .padding([3, 6])
                    .on_press(Message::ToggleEditSelection(target)),
            )
            .push(
                button(text(ctx.icons.move_up))
                    .padding([3, 6])
                    .on_press(Message::MoveRequestUp(item.id.clone())),
            )
            .push(
                button(text(ctx.icons.move_down))
                    .padding([3, 6])
                    .on_press(Message::MoveRequestDown(item.id.clone())),
            );
    }
    row_widgets = row_widgets.push(
        button(text(label).size(13))
            .style(if is_selected {
                button::primary
            } else {
                button::secondary
            })
            .padding([4, 8])
            .width(Length::Fill)
            .on_press(Message::Select(item.id.clone())),
    );

    row_widgets
        .spacing(4)
        .align_y(Alignment::Center)
}
