use iced::widget::{column, pick_list, text, text_input};
use iced::{Element, Length};

use super::super::Message;
use crate::app::options::{AuthKind, AuthState};

pub fn auth_editor(auth: &AuthState) -> Element<'_, Message> {
    let kind_pick = pick_list(AuthKind::ALL.to_vec(), Some(auth.kind), |kind| {
        Message::AuthChanged(AuthState {
            kind,
            ..auth.clone()
        })
    });

    let fields: Element<'_, Message> = match auth.kind {
        AuthKind::None => text("No authentication").into(),
        AuthKind::Bearer => text_input("Bearer token", &auth.bearer_token)
            .on_input(|val| {
                let mut new = auth.clone();
                new.bearer_token = val;
                Message::AuthChanged(new)
            })
            .padding(4)
            .width(Length::Fill)
            .into(),
        AuthKind::ApiKey => column![
            text_input("Header name", &auth.api_key_name).on_input(|val| {
                let mut new = auth.clone();
                new.api_key_name = val;
                Message::AuthChanged(new)
            }),
            text_input("Header value", &auth.api_key_value)
                .on_input(|val| {
                    let mut new = auth.clone();
                    new.api_key_value = val;
                    Message::AuthChanged(new)
                })
                .padding(6)
                .width(Length::Fill),
        ]
        .spacing(4)
        .into(),
        AuthKind::Basic => column![
            text_input("Username", &auth.basic_username).on_input(|val| {
                let mut new = auth.clone();
                new.basic_username = val;
                Message::AuthChanged(new)
            }),
            text_input("Password", &auth.basic_password)
                .on_input(|val| {
                    let mut new = auth.clone();
                    new.basic_password = val;
                    Message::AuthChanged(new)
                })
                .padding(4)
                .width(Length::Fill),
        ]
        .spacing(4)
        .into(),
    };

    column![kind_pick, fields].spacing(4).into()
}
