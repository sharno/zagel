mod auth;
mod response;
mod sidebar;
mod workspace;

use iced::widget::pane_grid::{self, PaneGrid};
use iced::widget::{column, container, row, rule, space, text};
use iced::{Element, Length};

use super::{Message, Zagel};
use sidebar::{SidebarContext, sidebar};
use workspace::workspace;

pub use response::{ResponseData, ResponseDisplay, ResponseTab};
pub use sidebar::IconSet;
pub use workspace::{BuilderPane, WorkspacePane};

#[derive(Debug, Clone, Copy)]
pub enum PaneContent {
    Sidebar,
    Workspace,
}

pub fn section<'a, Message: 'a>(
    title: &'a str,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    container(column![text(title).size(15), rule::horizontal(1), content].spacing(6))
        .padding(12)
        .width(Length::Fill)
        .style(container::bordered_box)
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
    .spacing(8.0)
    .on_resize(6, Message::PaneResized);

    column![
        container(grid).height(Length::Fill),
        rule::horizontal(1),
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
        text(hint).size(12),
        space().width(Length::Fill),
        text(format!("Status: {}", app.status_line)).size(12),
    ]
    .spacing(8);

    container(content).padding([6, 12]).into()
}
