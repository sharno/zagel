use iced::widget::pane_grid::{self, PaneGrid};
use iced::widget::{
    button, column, container, pick_list, row, scrollable, stack, text, text_editor,
    text_input,
};
use iced::{alignment, Alignment, Element, Length, Theme};

use super::super::{Message, Zagel, headers};
use super::auth::auth_editor;
use super::response::{response_panel, response_tab_toggle, response_view_toggle};
use super::section;
use crate::app::options::RequestMode;
use crate::model::{Method, RequestId};
use crate::theme;

#[derive(Debug, Clone, Copy)]
pub enum WorkspacePane {
    Builder,
    Response,
}

#[derive(Debug, Clone, Copy)]
pub enum BuilderPane {
    Form,
    Body,
}

const ENV_PICK_MAX_WIDTH: f32 = 180.0;
const MODE_PICK_MAX_WIDTH: f32 = 150.0;
const METHOD_PICK_MAX_WIDTH: f32 = 120.0;
const ACTION_WIDTH: f32 = 84.0;
const LABEL_WIDTH: f32 = 80.0;

pub fn workspace(app: &Zagel) -> Element<'_, Message> {
    let workspace_grid = PaneGrid::new(&app.view.workspace_panes, move |_, pane, _| match pane {
        WorkspacePane::Builder => pane_grid::Content::new(builder(app)),
        WorkspacePane::Response => pane_grid::Content::new(response(app)),
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .spacing(8.0)
    .on_resize(6, Message::WorkspacePaneResized);

    let header = text("Request Builder").size(20);

    container(
        column![header, workspace_grid]
            .spacing(8)
            .height(Length::Fill),
    )
    .padding(8)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn builder(app: &Zagel) -> Element<'_, Message> {
    let builder_grid = PaneGrid::new(&app.view.builder_panes, move |_, pane, _| match pane {
        BuilderPane::Form => pane_grid::Content::new(builder_form(app)),
        BuilderPane::Body => pane_grid::Content::new(builder_body(app)),
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .spacing(4.0)
    .on_resize(6, Message::BuilderPaneResized);

    builder_grid.into()
}

#[allow(clippy::too_many_lines)]
fn builder_form(app: &Zagel) -> Element<'_, Message> {
    let env_pick = container(
        pick_list(
            app.view
                .environments
                .iter()
                .map(|e| e.name.clone())
                .collect::<Vec<_>>(),
            Some(app.view.active_environment_name()),
            Message::EnvironmentChanged,
        )
        .width(Length::Fill),
    )
    .width(Length::FillPortion(2))
    .max_width(ENV_PICK_MAX_WIDTH);

    let method_pick = container(
        pick_list(
            Method::ALL.to_vec(),
            Some(app.model.draft.method),
            Message::MethodSelected,
        )
        .width(Length::Fill),
    )
    .width(Length::FillPortion(2))
    .max_width(METHOD_PICK_MAX_WIDTH);

    let url_input = text_input("https://api.example.com", &app.model.draft.url)
        .on_input(Message::UrlChanged)
        .padding(6)
        .width(Length::FillPortion(6));

    let title_input = text_input("Title", &app.model.draft.title)
        .on_input(Message::TitleChanged)
        .padding(4)
        .width(Length::FillPortion(5));

    let save_path_row: Element<'_, Message> = match &app.view.selection {
        Some(RequestId::HttpFile { path, .. }) => row![
            container(text("Saving to").size(14)).width(Length::Fixed(LABEL_WIDTH)),
            container(text(path.display().to_string()).size(14)).width(Length::Fill)
        ]
        .align_y(Alignment::Center)
        .spacing(6)
        .into(),
        _ => row![
            container(text("Save as").size(14)).width(Length::Fixed(LABEL_WIDTH)),
            text_input("path/to/request.http", &app.model.save_path)
                .on_input(Message::SavePathChanged)
                .padding(4)
                .width(Length::Fill),
        ]
        .align_y(Alignment::Center)
        .spacing(6)
        .into(),
    };

    let mode_pick = container(
        pick_list(
            RequestMode::ALL.to_vec(),
            Some(app.model.mode),
            Message::ModeChanged,
        )
        .width(Length::Fill),
    )
    .width(Length::FillPortion(2))
    .max_width(MODE_PICK_MAX_WIDTH);

    let auth_view = auth_editor(&app.model.auth);

    let meta_section = column![
        row![
            title_input,
            button("Save")
                .on_press(Message::Save)
                .width(Length::Fixed(ACTION_WIDTH)),
        ]
        .align_y(Alignment::Center)
        .spacing(6),
        save_path_row,
        row![env_pick, mode_pick]
            .align_y(Alignment::Center)
            .spacing(6),
    ]
    .spacing(6);

    let request_section = row![
        method_pick,
        url_input,
        button("Send")
            .on_press(Message::Send)
            .width(Length::Fixed(ACTION_WIDTH)),
    ]
    .align_y(Alignment::Center)
    .spacing(6);

    let form_content = column![
        section("Meta", meta_section.into()),
        section("Request", request_section.into()),
        section("Headers", headers::editor(&app.model.header_rows)),
        section("Auth", auth_view),
    ]
    .spacing(10);

    let form_scroll = scrollable(form_content)
        .width(Length::Fill)
        .height(Length::Fill);

    container(form_scroll)
        .padding(8)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn builder_body(app: &Zagel) -> Element<'_, Message> {
    let body_title = match app.model.mode {
        RequestMode::GraphQl => "GraphQL",
        RequestMode::Rest => "Body",
    };

    let body_panel: Element<'_, Message> = match app.model.mode {
        RequestMode::GraphQl => {
            let query_editor: iced::widget::TextEditor<'_, _, _, Theme> =
                text_editor(&app.model.graphql_query)
                    .on_action(Message::GraphqlQueryEdited)
                    .height(Length::FillPortion(3));
            let vars_editor: iced::widget::TextEditor<'_, _, _, Theme> =
                text_editor(&app.model.graphql_variables)
                    .on_action(Message::GraphqlVariablesEdited)
                    .height(Length::FillPortion(2));
            column![text("Query"), query_editor, text("Variables"), vars_editor]
                .height(Length::Fill)
                .spacing(6)
                .into()
        }
        RequestMode::Rest => {
            let body_editor: iced::widget::TextEditor<'_, _, _, Theme> =
                text_editor(&app.model.body_editor)
                    .on_action(Message::BodyEdited)
                    .height(Length::Fill);
            column![text("Body"), body_editor]
                .height(Length::Fill)
                .spacing(6)
                .into()
        }
    };

    let body_section = section(body_title, body_panel);

    container(column![body_section].padding(8).height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn response(app: &Zagel) -> Element<'_, Message> {
    let mut status_row = row![
        response_view_toggle(app.view.response_display),
        response_tab_toggle(app.view.response_tab),
    ]
    .spacing(8);

    if matches!(app.view.response_tab, super::response::ResponseTab::Body) {
        status_row = status_row.push(button("Copy body").on_press(Message::CopyResponseBody));
    }

    let response_view = response_panel(
        app.view.last_response.as_ref(),
        &app.view.response_viewer,
        app.view.response_display,
        app.view.response_tab,
        app.runtime.state.theme.highlight_theme(),
    );

    let response_section = section(
        "Response",
        column![status_row, response_view]
            .spacing(6)
            .height(Length::Fill)
            .into(),
    );

    let response_scroll = scrollable(response_section)
        .width(Length::Fill)
        .height(Length::Fill);

    let base = container(response_scroll)
        .padding(8)
        .width(Length::Fill)
        .height(Length::Fill)
        .into();

    if app.view.show_shortcuts {
        let overlay = container(shortcuts_panel())
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(alignment::Horizontal::Right)
            .align_y(alignment::Vertical::Top)
            .padding(12)
            .into();

        return stack([base, overlay]).into();
    }

    base
}

fn shortcuts_panel() -> Element<'static, Message> {
    let header = row![
        text("Keyboard shortcuts").size(16),
        button("Close").on_press(Message::ToggleShortcutsHelp)
    ]
    .spacing(8);

    let shortcuts = column![
        text("? - Toggle shortcuts help").size(14),
        text("Ctrl/Cmd+Z - Undo").size(14),
        text("Ctrl/Cmd+Shift+Z - Redo").size(14),
        text("Ctrl/Cmd+Y - Redo").size(14),
        text("Ctrl/Cmd+S - Save request").size(14),
        text("Ctrl/Cmd+Enter - Send request").size(14),
    ]
    .spacing(2);

    container(column![header, shortcuts].spacing(6))
        .padding(10)
        .style(theme::overlay_container_style)
        .into()
}
