use iced::widget::{button, column, container, row, text_input};
use iced::{Alignment, Element, Length};

use super::{HeaderRow, Message};

pub fn editor(rows: &[HeaderRow]) -> Element<'_, Message> {
    let mut list = column![];
    for (idx, row_data) in rows.iter().enumerate() {
        let idx_name = idx;
        let idx_value = idx;
        let name_input = container(
            text_input("Name", &row_data.name)
                .on_input(move |val| Message::HeaderNameChanged(idx_name, val))
                .padding(6)
                .width(Length::Fill),
        )
        .width(Length::FillPortion(2))
        .max_width(220.0);
        list = list.push(
            row![
                name_input,
                text_input("Value", &row_data.value)
                    .on_input(move |val| Message::HeaderValueChanged(idx_value, val))
                    .padding(6)
                    .width(Length::FillPortion(5)),
                button("x")
                    .on_press(Message::HeaderRemoved(idx))
                    .padding([5, 10]),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        );
    }
    list = list.push(
        button("Add header")
            .on_press(Message::HeaderAdded)
            .padding([5, 10]),
    );
    list.spacing(6).into()
}
