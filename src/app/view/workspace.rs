use iced::widget::pane_grid::{self, PaneGrid};
use iced::widget::{
    button, column, container, pick_list, row, scrollable, stack, text, text_editor, text_input,
    tooltip,
};
use iced::{alignment, Alignment, Element, Length, Theme};

use super::super::{headers, Message, Zagel};
use super::auth::auth_editor;
use super::response::{response_panel, response_tab_toggle, response_view_toggle};
use super::section;
use crate::app::options::RequestMode;
use crate::icons;
use crate::model::{Method, RequestId};
use crate::theme::{self, spacing, typo};

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
const ACTION_WIDTH: f32 = 100.0;
const LABEL_WIDTH: f32 = 72.0;

pub fn workspace(app: &Zagel) -> Element<'_, Message> {
    let workspace_grid = PaneGrid::new(&app.workspace_panes, move |_, pane, _| match pane {
        WorkspacePane::Builder => pane_grid::Content::new(builder(app)),
        WorkspacePane::Response => pane_grid::Content::new(response(app)),
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .spacing(spacing::XXS)
    .on_resize(6, Message::WorkspacePaneResized);

    container(workspace_grid)
        .padding(spacing::XS)
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
    .spacing(spacing::XXS)
    .on_resize(6, Message::BuilderPaneResized);

    builder_grid.into()
}

#[allow(clippy::too_many_lines)]
fn builder_form(app: &Zagel) -> Element<'_, Message> {
    let env_pick = container(
        pick_list(
            app.environments
                .iter()
                .map(|e| e.name.clone())
                .collect::<Vec<_>>(),
            Some(app.environments[app.active_environment].name.clone()),
            Message::EnvironmentChanged,
        )
        .width(Length::Fill),
    )
    .width(Length::FillPortion(2))
    .max_width(ENV_PICK_MAX_WIDTH);

    let method_pick = container(
        pick_list(
            Method::ALL.to_vec(),
            Some(app.draft.method),
            Message::MethodSelected,
        )
        .width(Length::Fill),
    )
    .width(Length::FillPortion(2))
    .max_width(METHOD_PICK_MAX_WIDTH);

    let url_input = text_input("https://api.example.com", &app.draft.url)
        .on_input(Message::UrlChanged)
        .padding(spacing::XS)
        .width(Length::FillPortion(6));

    let title_input = text_input("Title", &app.draft.title)
        .on_input(Message::TitleChanged)
        .padding(spacing::XS)
        .width(Length::FillPortion(5));

    let save_path_row: Element<'_, Message> = match app.workspace.selection() {
        Some(RequestId::HttpFile { path, .. }) => row![
            container(text("Saving to").size(typo::CAPTION)).width(Length::Fixed(LABEL_WIDTH)),
            container(text(path.display().to_string()).size(typo::CAPTION)).width(Length::Fill)
        ]
        .align_y(Alignment::Center)
        .spacing(spacing::SM)
        .into(),
        _ => row![
            container(text("Save as").size(typo::CAPTION)).width(Length::Fixed(LABEL_WIDTH)),
            text_input("path/to/request.http", &app.save_path)
                .on_input(Message::SavePathChanged)
                .padding(spacing::XS)
                .width(Length::Fill),
        ]
        .align_y(Alignment::Center)
        .spacing(spacing::SM)
        .into(),
    };

    let mode_pick = container(
        pick_list(
            RequestMode::ALL.to_vec(),
            Some(app.mode),
            Message::ModeChanged,
        )
        .width(Length::Fill),
    )
    .width(Length::FillPortion(2))
    .max_width(MODE_PICK_MAX_WIDTH);

    let auth_view = auth_editor(&app.auth);

    let meta_section = column![
        row![
            title_input,
            tooltip(
                button(
                    row![
                        icons::save().size(typo::BODY),
                        text("Save").size(typo::BODY)
                    ]
                    .spacing(spacing::XXS)
                    .align_y(Alignment::Center),
                )
                .on_press(Message::Save)
                .padding([spacing::XS, spacing::MD])
                .width(Length::Fixed(ACTION_WIDTH)),
                "Save request (Ctrl+S)",
                tooltip::Position::Top,
            ),
        ]
        .align_y(Alignment::Center)
        .spacing(spacing::SM),
        save_path_row,
        row![env_pick, mode_pick]
            .align_y(Alignment::Center)
            .spacing(spacing::SM),
    ]
    .spacing(spacing::SM);

    let request_section = row![
        method_pick,
        url_input,
        tooltip(
            button(
                row![
                    icons::send().size(typo::BODY),
                    text("Send").size(typo::BODY)
                ]
                .spacing(spacing::XXS)
                .align_y(Alignment::Center),
            )
            .on_press(Message::Send)
            .padding([spacing::XS, spacing::MD])
            .width(Length::Fixed(ACTION_WIDTH))
            .style(theme::accent_button_style),
            "Send request (Ctrl+Enter)",
            tooltip::Position::Top,
        ),
    ]
    .align_y(Alignment::Center)
    .spacing(spacing::SM);

    let form_content = column![
        section(
            "Meta",
            icons::info_circle().size(typo::BODY),
            meta_section.into(),
        ),
        section(
            "Request",
            icons::send_icon().size(typo::BODY),
            request_section.into(),
        ),
        section(
            "Headers",
            icons::list_ul().size(typo::BODY),
            headers::editor(&app.header_rows),
        ),
        section("Auth", icons::key().size(typo::BODY), auth_view,),
    ]
    .spacing(spacing::SM);

    let form_scroll = scrollable(form_content)
        .width(Length::Fill)
        .height(Length::Fill);

    container(form_scroll)
        .padding(spacing::XS)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn builder_body(app: &Zagel) -> Element<'_, Message> {
    let body_title = match app.mode {
        RequestMode::GraphQl => "GraphQL",
        RequestMode::Rest => "REST",
    };

    let body_panel: Element<'_, Message> = match app.mode {
        RequestMode::GraphQl => {
            let query_editor: iced::widget::TextEditor<'_, _, _, Theme> =
                text_editor(&app.graphql_query)
                    .on_action(Message::GraphqlQueryEdited)
                    .height(Length::FillPortion(3));
            let vars_editor: iced::widget::TextEditor<'_, _, _, Theme> =
                text_editor(&app.graphql_variables)
                    .on_action(Message::GraphqlVariablesEdited)
                    .height(Length::FillPortion(2));
            column![
                text("Query").size(typo::CAPTION),
                query_editor,
                text("Variables").size(typo::CAPTION),
                vars_editor,
            ]
            .height(Length::Fill)
            .spacing(spacing::XS)
            .into()
        }
        RequestMode::Rest => {
            let body_editor: iced::widget::TextEditor<'_, _, _, Theme> =
                text_editor(&app.body_editor)
                    .on_action(Message::BodyEdited)
                    .height(Length::Fill);
            column![text("Body").size(typo::CAPTION), body_editor]
                .height(Length::Fill)
                .spacing(spacing::XS)
                .into()
        }
    };

    let body_section = section(body_title, icons::code_slash().size(typo::BODY), body_panel);

    container(
        column![body_section]
            .padding(spacing::XS)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn response(app: &Zagel) -> Element<'_, Message> {
    let mut status_row = row![
        response_view_toggle(app.response_display),
        response_tab_toggle(app.response_tab),
    ]
    .spacing(spacing::SM)
    .align_y(Alignment::Center);

    if matches!(app.response_tab, super::response::ResponseTab::Body) {
        status_row = status_row.push(tooltip(
            button(
                row![
                    icons::clipboard().size(typo::BODY),
                    text("Raw").size(typo::BODY)
                ]
                .spacing(spacing::XXS)
                .align_y(Alignment::Center),
            )
            .on_press(Message::CopyResponseRaw)
            .padding([spacing::XXS, spacing::SM])
            .style(theme::ghost_button_style),
            "Copy raw response body",
            tooltip::Position::Top,
        ));
        if app.response_display == super::response::ResponseDisplay::Pretty
            && app
                .response
                .as_ref()
                .and_then(|response| response.body.pretty_text())
                .is_some()
        {
            status_row = status_row.push(tooltip(
                button(
                    row![
                        icons::clipboard().size(typo::BODY),
                        text("Pretty").size(typo::BODY)
                    ]
                    .spacing(spacing::XXS)
                    .align_y(Alignment::Center),
                )
                .on_press(Message::CopyResponsePretty)
                .padding([spacing::XXS, spacing::SM])
                .style(theme::ghost_button_style),
                "Copy pretty-printed response",
                tooltip::Position::Top,
            ));
        }
    }

    let response_view = response_panel(
        app.response.as_ref(),
        &app.response_viewer,
        app.response_display,
        app.response_tab,
        app.state.theme.highlight_theme(),
    );

    let response_section = section(
        "Response",
        icons::arrow_left_right().size(typo::BODY),
        column![status_row, response_view]
            .spacing(spacing::SM)
            .height(Length::Fill)
            .into(),
    );

    let base = container(response_section)
        .padding(spacing::XS)
        .width(Length::Fill)
        .height(Length::Fill)
        .into();

    if app.show_shortcuts {
        let overlay = container(shortcuts_panel())
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(alignment::Horizontal::Right)
            .align_y(alignment::Vertical::Top)
            .padding(spacing::LG)
            .into();

        return stack([base, overlay]).into();
    }

    base
}

fn shortcuts_panel() -> Element<'static, Message> {
    let header = row![
        icons::question_circle().size(typo::HEADING),
        text("Keyboard shortcuts").size(typo::HEADING),
        button(
            row![
                icons::x_circle().size(typo::BODY),
                text("Close").size(typo::BODY)
            ]
            .spacing(spacing::XXS)
            .align_y(Alignment::Center),
        )
        .on_press(Message::ToggleShortcutsHelp)
        .padding([spacing::XXS, spacing::SM])
        .style(theme::ghost_button_style),
    ]
    .spacing(spacing::SM)
    .align_y(Alignment::Center);

    let shortcuts = column![
        text("?  Toggle shortcuts help").size(typo::BODY),
        text("Ctrl/Cmd+S  Save request").size(typo::BODY),
        text("Ctrl/Cmd+Enter  Send request").size(typo::BODY),
    ]
    .spacing(spacing::XXS);

    container(column![header, shortcuts].spacing(spacing::SM))
        .padding(spacing::LG)
        .style(theme::overlay_container_style)
        .into()
}
