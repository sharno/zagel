mod auth;
mod response;
mod sidebar;
mod workspace;

use iced::widget::pane_grid::{self, PaneGrid};
use iced::widget::{container, text};
use iced::{Element, Length};

use super::{Message, Zagel};
use sidebar::sidebar;
use workspace::workspace;

pub use response::{ResponseDisplay, ResponseTab, pretty_json};

#[derive(Debug, Clone, Copy)]
pub enum PaneContent {
    Sidebar,
    Workspace,
}

pub fn view(app: &Zagel) -> Element<'_, Message> {
    let app_ref = app;

    let grid = PaneGrid::new(&app_ref.panes, move |_, pane, _| match pane {
        PaneContent::Sidebar => pane_grid::Content::new(sidebar(
            &app_ref.unsaved_tabs,
            &app_ref.collections,
            &app_ref.http_files,
            app_ref.selection.as_ref(),
        ))
        .title_bar(pane_grid::TitleBar::new(text("Collections"))),
        PaneContent::Workspace => pane_grid::Content::new(workspace(app_ref))
            .title_bar(pane_grid::TitleBar::new(text("Request Builder"))),
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .spacing(12.0)
    .on_resize(6, Message::PaneResized);

    container(grid).into()
}
