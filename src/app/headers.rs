use iced::widget::{button, column, container, row, text, text_input, tooltip};
use iced::{Alignment, Element, Length};

use super::{HeaderRow, Message};
use crate::icons;
use crate::theme::{self, spacing, typo};

pub fn editor(rows: &[HeaderRow]) -> Element<'_, Message> {
    let mut list = column![];
    for (idx, row_data) in rows.iter().enumerate() {
        let idx_name = idx;
        let idx_value = idx;
        let name_input = container(
            text_input("Name", &row_data.name)
                .on_input(move |val| Message::HeaderNameChanged(idx_name, val))
                .padding(spacing::XS)
                .width(Length::Fill),
        )
        .width(Length::FillPortion(2))
        .max_width(220.0);
        list = list.push(
            row![
                name_input,
                text_input("Value", &row_data.value)
                    .on_input(move |val| Message::HeaderValueChanged(idx_value, val))
                    .padding(spacing::XS)
                    .width(Length::FillPortion(5)),
                tooltip(
                    button(icons::x_lg().size(typo::BODY))
                        .on_press(Message::HeaderRemoved(idx))
                        .padding([spacing::XXS, spacing::XS])
                        .style(theme::ghost_button_style),
                    "Remove header",
                    tooltip::Position::Top,
                ),
            ]
            .spacing(spacing::XS)
            .align_y(Alignment::Center),
        );
    }
    list = list.push(
        button(
            row![
                icons::plus_lg().size(typo::BODY),
                text("Add header").size(typo::BODY)
            ]
            .spacing(spacing::XXS)
            .align_y(Alignment::Center),
        )
        .on_press(Message::HeaderAdded)
        .padding([spacing::XXS, spacing::SM]),
    );
    list.spacing(spacing::XS).into()
}
