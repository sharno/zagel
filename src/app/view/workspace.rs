use iced::widget::{
    button, column, container, pick_list, row, rule, scrollable, text, text_editor, text_input,
};
use iced::{Element, Length, Theme};

use super::super::{Message, Zagel, headers};
use super::auth::auth_editor;
use super::response::{response_panel, response_tab_toggle, response_view_toggle};
use crate::app::options::RequestMode;
use crate::model::Method;

pub fn workspace(app: &Zagel) -> Element<'_, Message> {
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
        .padding(8)
        .width(Length::Fill);

    let title_input = text_input("Title", &app.draft.title)
        .on_input(Message::TitleChanged)
        .padding(6)
        .width(Length::Fill);

    let body_editor: iced::widget::TextEditor<'_, _, _, Theme> = text_editor(&app.body_editor)
        .on_action(Message::BodyEdited)
        .height(Length::Fixed(200.0));

    let response_view = response_panel(
        app.last_response.as_ref(),
        &app.response_viewer,
        app.response_display,
        app.response_tab,
    );

    let save_path_row: Element<'_, Message> = match &app.selection {
        Some(crate::model::RequestId::HttpFile { path, .. }) => row![
            text("Saving to").size(14),
            text(path.display().to_string()).size(14)
        ]
        .spacing(8)
        .into(),
        _ => row![
            text("Save as").size(14),
            text_input("path/to/request.http", &app.save_path)
                .on_input(Message::SavePathChanged)
                .padding(6)
                .width(Length::Fill),
        ]
        .spacing(8)
        .into(),
    };

    let mode_pick = pick_list(
        RequestMode::ALL.to_vec(),
        Some(app.mode),
        Message::ModeChanged,
    );

    let auth_view = auth_editor(&app.auth);

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
        RequestMode::Rest => column![text("Body"), body_editor].spacing(6).into(),
    };

    let mut status_row = row![
        text(format!("Status: {}", app.status_line.clone())),
        response_view_toggle(app.response_display),
        response_tab_toggle(app.response_tab),
    ]
    .spacing(12);

    if matches!(app.response_tab, super::response::ResponseTab::Body) {
        status_row = status_row.push(button("Copy body").on_press(Message::CopyResponseBody));
    }

    let workspace_content = column![
        row![env_pick, title_input, mode_pick].spacing(12),
        save_path_row,
        row![
            method_pick,
            url_input,
            button("Save").on_press(Message::Save),
            button("Send").on_press(Message::Send)
        ]
        .spacing(8),
        rule::horizontal(1),
        text("Headers"),
        headers::editor(&app.header_rows),
        text("Auth"),
        auth_view,
        rule::horizontal(1),
        graphql_panel,
        rule::horizontal(1),
        status_row,
        response_view,
    ]
    .padding(12)
    .spacing(8);

    scrollable(container(workspace_content).width(Length::Fill)).into()
}
