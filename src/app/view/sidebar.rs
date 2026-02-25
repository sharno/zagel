use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

use iced::widget::{Space, button, column, container, row, scrollable, text, text_input, tooltip};
use iced::{Alignment, Element, Length};

use crate::icons;
use crate::pathing::{GlobalEnvRoot, ProjectRoot};
use crate::theme::{self, spacing, typo};

use super::super::{EditState, EditTarget, Message};
use super::section;
use crate::model::{HttpFile, RequestDraft, RequestId};

const INDENT: i16 = 14;

/// Icon set selection is now purely cosmetic (bootstrap vs ascii fallback).
/// The default is `Bootstrap`; set `ZAGEL_ICON_SET=ascii` to fall back.
#[derive(Debug, Clone, Copy)]
pub enum IconSet {
    Bootstrap,
    Ascii,
}

impl IconSet {
    pub fn from_env() -> Self {
        match std::env::var("ZAGEL_ICON_SET")
            .ok()
            .map(|value| value.to_ascii_lowercase())
        {
            Some(value) if value == "ascii" => Self::Ascii,
            _ => Self::Bootstrap,
        }
    }
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
    icon_set: IconSet,
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

    let project_input = text_input("C:/path/to/project", ctx.project_path_input)
        .on_input(Message::ProjectPathInputChanged)
        .padding(spacing::XS)
        .width(Length::FillPortion(4));
    let add_project = tooltip(
        button(
            row![icons::plus_circle().size(typo::BODY), text("Add").size(typo::BODY)]
                .spacing(spacing::XXS)
                .align_y(Alignment::Center),
        )
        .on_press(Message::AddProject)
        .padding([spacing::XXS, spacing::SM]),
        "Add project folder",
        tooltip::Position::Top,
    );

    let mut project_roots = column![row![project_input, add_project].spacing(spacing::XS)]
        .spacing(spacing::XS);
    if ctx.project_roots.is_empty() {
        project_roots = project_roots.push(text("No projects configured").size(typo::CAPTION));
    } else {
        for root in ctx.project_roots {
            project_roots = project_roots.push(
                row![
                    text(root.as_path().display().to_string()).size(typo::CAPTION),
                    tooltip(
                        button(icons::dash_circle().size(typo::BODY))
                            .on_press(Message::RemoveProject(root.clone()))
                            .padding([spacing::XXXS, spacing::XXS])
                            .style(theme::ghost_button_style),
                        "Remove project",
                        tooltip::Position::Top,
                    ),
                ]
                .align_y(Alignment::Center)
                .spacing(spacing::XS),
            );
        }
    }

    let global_env_input = text_input("C:/path/to/global/envs", ctx.global_env_path_input)
        .on_input(Message::GlobalEnvPathInputChanged)
        .padding(spacing::XS)
        .width(Length::FillPortion(4));
    let add_global = tooltip(
        button(
            row![icons::plus_circle().size(typo::BODY), text("Add").size(typo::BODY)]
                .spacing(spacing::XXS)
                .align_y(Alignment::Center),
        )
        .on_press(Message::AddGlobalEnvRoot)
        .padding([spacing::XXS, spacing::SM]),
        "Add global env folder",
        tooltip::Position::Top,
    );

    let mut global_env_roots = column![row![global_env_input, add_global].spacing(spacing::XS)]
        .spacing(spacing::XS);
    if ctx.global_env_roots.is_empty() {
        global_env_roots = global_env_roots.push(text("No global env folders").size(typo::CAPTION));
    } else {
        for root in ctx.global_env_roots {
            global_env_roots = global_env_roots.push(
                row![
                    text(root.as_path().display().to_string()).size(typo::CAPTION),
                    tooltip(
                        button(icons::dash_circle().size(typo::BODY))
                            .on_press(Message::RemoveGlobalEnvRoot(root.clone()))
                            .padding([spacing::XXXS, spacing::XXS])
                            .style(theme::ghost_button_style),
                        "Remove env folder",
                        tooltip::Position::Top,
                    ),
                ]
                .align_y(Alignment::Center)
                .spacing(spacing::XS),
            );
        }
    }

    let mut header = row![
        text("Requests").size(typo::TITLE),
        tooltip(
            button(
                row![icons::plus_circle().size(typo::BODY), text("Add").size(typo::BODY)]
                    .spacing(spacing::XXS)
                    .align_y(Alignment::Center),
            )
            .on_press(Message::AddRequest)
            .padding([spacing::XXS, spacing::SM]),
            "Add new request",
            tooltip::Position::Top,
        ),
    ]
    .align_y(Alignment::Center)
    .spacing(spacing::XS);

    if editing {
        let selection_empty = edit_selection.is_none_or(HashSet::is_empty);
        let delete_button = if selection_empty {
            button(
                row![icons::trash().size(typo::BODY), text("Delete").size(typo::BODY)]
                    .spacing(spacing::XXS)
                    .align_y(Alignment::Center),
            )
            .padding([spacing::XXS, spacing::SM])
            .style(theme::destructive_button_style)
        } else {
            button(
                row![icons::trash().size(typo::BODY), text("Delete").size(typo::BODY)]
                    .spacing(spacing::XXS)
                    .align_y(Alignment::Center),
            )
            .on_press(Message::DeleteSelected)
            .padding([spacing::XXS, spacing::SM])
            .style(theme::destructive_button_style)
        };
        header = header
            .push(delete_button)
            .push(
                button(
                    row![icons::check_lg().size(typo::BODY), text("Done").size(typo::BODY)]
                        .spacing(spacing::XXS)
                        .align_y(Alignment::Center),
                )
                .on_press(Message::ToggleEditMode)
                .padding([spacing::XXS, spacing::SM]),
            );
    } else {
        header = header.push(
            tooltip(
                button(
                    row![icons::pencil().size(typo::BODY), text("Edit").size(typo::BODY)]
                        .spacing(spacing::XXS)
                        .align_y(Alignment::Center),
                )
                .on_press(Message::ToggleEditMode)
                .padding([spacing::XXS, spacing::SM]),
                "Toggle edit mode",
                tooltip::Position::Top,
            ),
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
        icon_set: ctx.icon_set,
    };
    let list = render_tree(column![], &tree, "", 0, &render_ctx).spacing(spacing::XXXS);

    let project_section = section(
        "Projects",
        icons::folder_open().size(typo::BODY),
        project_roots.into(),
    );
    let global_env_section = section(
        "Global Environments",
        icons::globe().size(typo::BODY),
        global_env_roots.into(),
    );
    let collections_section = section(
        "Collections",
        icons::collection().size(typo::BODY),
        list.into(),
    );

    let list = scrollable(
        column![
            project_section,
            global_env_section,
            header,
            collections_section
        ]
        .spacing(spacing::SM),
    )
    .width(Length::Fill)
    .height(Length::Fill);

    container(list)
        .padding(spacing::SM)
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

/// Collapsed/expanded chevron icon element.
fn chevron_icon(collapsed: bool, icon_set: IconSet) -> Element<'static, Message> {
    match icon_set {
        IconSet::Bootstrap => {
            if collapsed {
                icons::chevron_right().size(typo::CAPTION).into()
            } else {
                icons::chevron_down().size(typo::CAPTION).into()
            }
        }
        IconSet::Ascii => {
            let label = if collapsed { ">" } else { "v" };
            text(label).size(typo::CAPTION).into()
        }
    }
}

/// Checkbox icon element.
fn checkbox_icon(checked: bool, icon_set: IconSet) -> Element<'static, Message> {
    match icon_set {
        IconSet::Bootstrap => {
            if checked {
                icons::check_square().size(typo::BODY).into()
            } else {
                icons::square().size(typo::BODY).into()
            }
        }
        IconSet::Ascii => {
            let label = if checked { "[x]" } else { "[ ]" };
            text(label).size(typo::CAPTION).into()
        }
    }
}

fn collection_row<'a>(
    child: &TreeChild,
    depth: usize,
    ctx: &RenderContext<'a>,
    full_path: &str,
    is_collapsed: bool,
) -> iced::widget::Row<'a, Message> {
    let toggle = button(chevron_icon(is_collapsed, ctx.icon_set))
        .style(theme::ghost_button_style)
        .padding([spacing::XXXS, spacing::XXS])
        .on_press(Message::ToggleCollection(full_path.to_string()));

    let mut row_widgets = row![Space::new().width(Length::Fixed(indent_px(depth))), toggle];

    let collection_path = child.node.file_path.clone();
    if ctx.editing
        && let (Some(edit_selection), Some(collection_path)) = (ctx.edit_selection, collection_path)
    {
        let target = EditTarget::Collection(collection_path.clone());
        let checked = edit_selection.contains(&target);
        row_widgets = row_widgets
            .push(
                button(checkbox_icon(checked, ctx.icon_set))
                    .padding([spacing::XXXS, spacing::XXS])
                    .style(theme::ghost_button_style)
                    .on_press(Message::ToggleEditSelection(target)),
            )
            .push(
                button(match ctx.icon_set {
                    IconSet::Bootstrap => icons::arrow_up().size(typo::CAPTION).into(),
                    IconSet::Ascii => Element::from(text("^").size(typo::CAPTION)),
                })
                .padding([spacing::XXXS, spacing::XXS])
                .style(theme::ghost_button_style)
                .on_press(Message::MoveCollectionUp(collection_path.clone())),
            )
            .push(
                button(match ctx.icon_set {
                    IconSet::Bootstrap => icons::arrow_down().size(typo::CAPTION).into(),
                    IconSet::Ascii => Element::from(text("v").size(typo::CAPTION)),
                })
                .padding([spacing::XXXS, spacing::XXS])
                .style(theme::ghost_button_style)
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

        let selected = is_selected;
        row_widgets.push(
            button(text(child.name.clone()).size(typo::BODY))
                .style(move |theme, status| {
                    theme::sidebar_item_style(theme, status, selected)
                })
                .padding([spacing::XXS, spacing::SM])
                .width(Length::Fill)
                .on_press(Message::Select(select_id)),
        )
    } else {
        row_widgets.push(text(child.name.clone()).size(typo::BODY))
    };

    row_widgets
        .spacing(spacing::XXXS)
        .align_y(Alignment::Center)
}

fn request_row<'a>(
    item: &RequestItem,
    depth: usize,
    ctx: &RenderContext<'a>,
) -> iced::widget::Row<'a, Message> {
    let is_selected = ctx.selection.is_some_and(|s| *s == item.id);

    // Build the request label with colored method badge
    let method = item.draft.method;
    let method_color = theme::method_color(method);
    let method_badge = text(method.as_str())
        .size(typo::CAPTION)
        .color(method_color);

    let selected_indicator: Element<'_, Message> = if is_selected {
        match ctx.icon_set {
            IconSet::Bootstrap => icons::arrow_right().size(typo::CAPTION).into(),
            IconSet::Ascii => text(">").size(typo::CAPTION).into(),
        }
    } else {
        Space::new().width(Length::Shrink).into()
    };

    let title = item.draft.title.clone();
    let label_content = row![selected_indicator, method_badge, text(title).size(typo::BODY)]
        .spacing(spacing::XXS)
        .align_y(Alignment::Center);

    let mut row_widgets = row![Space::new().width(Length::Fixed(indent_px(depth + 1)))];
    if ctx.editing
        && let Some(edit_selection) = ctx.edit_selection
    {
        let target = EditTarget::Request(item.id.clone());
        let checked = edit_selection.contains(&target);
        row_widgets = row_widgets
            .push(
                button(checkbox_icon(checked, ctx.icon_set))
                    .padding([spacing::XXXS, spacing::XXS])
                    .style(theme::ghost_button_style)
                    .on_press(Message::ToggleEditSelection(target)),
            )
            .push(
                button(match ctx.icon_set {
                    IconSet::Bootstrap => icons::arrow_up().size(typo::CAPTION).into(),
                    IconSet::Ascii => Element::from(text("^").size(typo::CAPTION)),
                })
                .padding([spacing::XXXS, spacing::XXS])
                .style(theme::ghost_button_style)
                .on_press(Message::MoveRequestUp(item.id.clone())),
            )
            .push(
                button(match ctx.icon_set {
                    IconSet::Bootstrap => icons::arrow_down().size(typo::CAPTION).into(),
                    IconSet::Ascii => Element::from(text("v").size(typo::CAPTION)),
                })
                .padding([spacing::XXXS, spacing::XXS])
                .style(theme::ghost_button_style)
                .on_press(Message::MoveRequestDown(item.id.clone())),
            );
    }

    let selected = is_selected;
    row_widgets = row_widgets.push(
        button(label_content)
            .style(move |theme, status| theme::sidebar_item_style(theme, status, selected))
            .padding([spacing::XXS, spacing::SM])
            .width(Length::Fill)
            .on_press(Message::Select(item.id.clone())),
    );

    row_widgets
        .spacing(spacing::XXXS)
        .align_y(Alignment::Center)
}
