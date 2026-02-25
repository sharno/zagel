mod auth;
mod response;
mod sidebar;
mod workspace;

use iced::widget::pane_grid::{self, PaneGrid};
use iced::widget::{column, container, row, space, text};
use iced::{Alignment, Element, Length};

use super::{Message, Zagel};
use sidebar::{SidebarContext, sidebar};
use workspace::workspace;

pub use response::{ResponseData, ResponseDisplay, ResponseTab};
pub use sidebar::IconSet;
pub use workspace::{BuilderPane, WorkspacePane};

use crate::theme::{self, spacing, typo};

#[derive(Debug, Clone, Copy)]
pub enum PaneContent {
    Sidebar,
    Workspace,
}

/// Section card with an icon next to the title.
pub fn section<'a, M: 'a>(
    title: &'a str,
    icon: impl Into<Element<'a, M>>,
    content: Element<'a, M>,
) -> Element<'a, M> {
    let title_row = row![
        icon.into(),
        text(title).size(typo::BODY),
    ]
    .spacing(spacing::XXS)
    .align_y(Alignment::Center);

    container(
        column![title_row, content].spacing(spacing::SM),
    )
    .padding(spacing::MD)
    .width(Length::Fill)
    .style(theme::section_container_style)
    .into()
}

pub fn view(app: &Zagel) -> Element<'_, Message> {
    let app_ref = app;

    let grid = PaneGrid::new(&app_ref.panes, move |_, pane, _| match pane {
        PaneContent::Sidebar => pane_grid::Content::new(sidebar(SidebarContext {
            http_files: app_ref.workspace.http_files(),
            http_file_order: app_ref.workspace.http_file_order(),
            selection: app_ref.workspace.selection(),
            collapsed: &app_ref.collapsed_collections,
            project_roots: app_ref.project_roots(),
            global_env_roots: app_ref.global_env_roots(),
            project_path_input: &app_ref.project_path_input,
            global_env_path_input: &app_ref.global_env_path_input,
            edit_state: &app_ref.edit_state,
            icon_set: app_ref.icon_set,
        })),
        PaneContent::Workspace => pane_grid::Content::new(workspace(app_ref)),
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .spacing(spacing::XXXS)
    .on_resize(6, Message::PaneResized);

    column![
        container(grid).height(Length::Fill),
        status_bar(app_ref)
    ]
    .into()
}

fn status_bar(app: &Zagel) -> Element<'_, Message> {
    let hint = if app.show_shortcuts {
        "Press ? to hide shortcuts"
    } else {
        "Press ? for shortcuts"
    };

    let content = row![
        text(hint).size(typo::CAPTION),
        space().width(Length::Fill),
        text(format!("Status: {}", app.status_line)).size(typo::CAPTION),
    ]
    .spacing(spacing::MD);

    container(content)
        .padding([spacing::XXS, spacing::LG])
        .width(Length::Fill)
        .style(theme::status_bar_style)
        .into()
}
