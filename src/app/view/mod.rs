mod auth;
mod response;
mod sidebar;
mod workspace;

use iced::widget::pane_grid::{self, PaneGrid};
use iced::widget::{column, container, rule, text};
use iced::{Element, Length};

use super::{Message, Zagel};
use sidebar::sidebar;
use workspace::workspace;

pub use response::{ResponseDisplay, ResponseTab, pretty_json};
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
    container(
        column![text(title).size(15), rule::horizontal(1), content].spacing(6),
    )
    .padding(12)
    .width(Length::Fill)
    .style(container::bordered_box)
    .into()
}

pub fn view(app: &Zagel) -> Element<'_, Message> {
    let app_ref = app;

    let grid = PaneGrid::new(&app_ref.panes, move |_, pane, _| match pane {
        PaneContent::Sidebar => pane_grid::Content::new(sidebar(
            &app_ref.collections,
            &app_ref.http_files,
            app_ref.selection.as_ref(),
            &app_ref.collapsed_collections,
            &app_ref.http_root,
        )),
        PaneContent::Workspace => pane_grid::Content::new(workspace(app_ref)),
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .spacing(8.0)
    .on_resize(6, Message::PaneResized);

    container(grid).into()
}
