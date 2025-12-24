use iced::widget::pane_grid::{self, PaneGrid};
use iced::widget::{
    button, column, container, pick_list, row, rule, scrollable, text, text_editor, text_input,
};
use iced::{Element, Length, Theme};

use super::super::{Message, Zagel, headers};
use super::auth::auth_editor;
use super::response::{response_panel, response_tab_toggle, response_view_toggle};
use crate::app::options::RequestMode;
use crate::model::{Method, RequestId};

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

pub fn workspace(app: &Zagel) -> Element<'_, Message> {
    let workspace_grid = PaneGrid::new(&app.workspace_panes, move |_, pane, _| match pane {
        WorkspacePane::Builder => pane_grid::Content::new(builder(app)),
        WorkspacePane::Response => pane_grid::Content::new(response(app)),
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .spacing(8.0)
    .on_resize(6, Message::WorkspacePaneResized);

    container(workspace_grid)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn builder(app: &Zagel) -> Element<'_, Message> {
    let builder_grid = PaneGrid::new(&app.builder_panes, move |_, pane, _| match pane {
        BuilderPane::Form => pane_grid::Content::new(builder_form(app)),
        BuilderPane::Body => pane_grid::Content::new(builder_body(app)),
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .spacing(4.0)
    .on_resize(6, Message::BuilderPaneResized);

    builder_grid.into()
}

fn builder_form(app: &Zagel) -> Element<'_, Message> {
    let env_pick = pick_list(
        app.environments
            .iter()
            .map(|e| e.name.clone())
            .collect::<Vec<_>>(),
        Some(app.environments[app.active_environment].name.clone()),
        Message::EnvironmentChanged,
    );

    let method_pick = pick_list(
        Method::ALL.to_vec(),
        Some(app.draft.method),
        Message::MethodSelected,
    );

    let url_input = text_input("https://api.example.com", &app.draft.url)
        .on_input(Message::UrlChanged)
        .padding(6)
        .width(Length::Fill);

    let title_input = text_input("Title", &app.draft.title)
        .on_input(Message::TitleChanged)
        .padding(4)
        .width(Length::Fill);

    let save_path_row: Element<'_, Message> = match &app.selection {
        Some(RequestId::HttpFile { path, .. }) => row![
            text("Saving to").size(14),
            text(path.display().to_string()).size(14)
        ]
        .spacing(6)
        .into(),
        _ => row![
            text("Save as").size(14),
            text_input("path/to/request.http", &app.save_path)
                .on_input(Message::SavePathChanged)
                .padding(4)
                .width(Length::Fill),
        ]
        .spacing(6)
        .into(),
    };

    let mode_pick = pick_list(
        RequestMode::ALL.to_vec(),
        Some(app.mode),
        Message::ModeChanged,
    );

    let auth_view = auth_editor(&app.auth);

    let form_content = column![
        row![env_pick, title_input, mode_pick].spacing(8),
        save_path_row,
        row![
            method_pick,
            url_input,
            button("Save").on_press(Message::Save),
            button("Send").on_press(Message::Send)
        ]
        .spacing(6),
        rule::horizontal(1),
        text("Headers"),
        headers::editor(&app.header_rows),
        text("Auth"),
        auth_view,
    ]
    .padding(8)
    .spacing(6);

    scrollable(form_content).into()
}

fn builder_body(app: &Zagel) -> Element<'_, Message> {
    let graphql_panel: Element<'_, Message> = match app.mode {
        RequestMode::GraphQl => {
            let query_editor: iced::widget::TextEditor<'_, _, _, Theme> =
                text_editor(&app.graphql_query)
                    .on_action(Message::GraphqlQueryEdited)
                    .height(Length::Fixed(180.0));
            let vars_editor: iced::widget::TextEditor<'_, _, _, Theme> =
                text_editor(&app.graphql_variables)
                    .on_action(Message::GraphqlVariablesEdited)
                    .height(Length::Fixed(120.0));
            column![
                text("GraphQL query"),
                query_editor,
                text("Variables (JSON)"),
                vars_editor,
            ]
            .spacing(6)
            .into()
        }
        RequestMode::Rest => {
            let body_editor: iced::widget::TextEditor<'_, _, _, Theme> =
                text_editor(&app.body_editor)
                    .on_action(Message::BodyEdited)
                    .height(Length::Fill);
            column![text("Body"), body_editor].spacing(6).into()
        }
    };

    container(column![graphql_panel].padding(8).spacing(6))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn response(app: &Zagel) -> Element<'_, Message> {
    let mut status_row = row![
        text(format!("Status: {}", app.status_line.clone())),
        response_view_toggle(app.response_display),
        response_tab_toggle(app.response_tab),
    ]
    .spacing(8);

    if matches!(app.response_tab, super::response::ResponseTab::Body) {
        status_row = status_row.push(button("Copy body").on_press(Message::CopyResponseBody));
    }

    let response_view = response_panel(
        app.last_response.as_ref(),
        &app.response_viewer,
        app.response_display,
        app.response_tab,
    );

    scrollable(
        column![status_row, response_view]
            .padding(8)
            .spacing(6)
            .width(Length::Fill),
    )
    .into()
}
