use iced::widget::{column, pick_list, text, text_input};
use iced::{Element, Length};

use super::super::Message;
use crate::app::options::{
    ApiKeyAuthState, AuthKind, AuthState, BasicAuthState, BearerAuthState, ClientSecretMethod,
    OAuth2ClientCredentialsAuthState,
};

pub fn auth_editor(auth: &AuthState) -> Element<'_, Message> {
    let kind_pick = pick_list(AuthKind::ALL.to_vec(), Some(auth.kind()), |kind| {
        Message::AuthChanged(auth.with_kind(kind))
    });

    let fields: Element<'_, Message> = match auth {
        AuthState::None => text("No authentication").into(),
        AuthState::Bearer(bearer) => bearer_fields(bearer),
        AuthState::ApiKey(api_key) => api_key_fields(api_key),
        AuthState::Basic(basic) => basic_fields(basic),
        AuthState::OAuth2ClientCredentials(oauth) => oauth2_client_credentials_fields(oauth),
    };

    column![kind_pick, fields].spacing(4).into()
}

fn bearer_fields(bearer: &BearerAuthState) -> Element<'_, Message> {
    text_input("Bearer token", &bearer.token)
        .on_input(|token| Message::AuthChanged(AuthState::Bearer(BearerAuthState { token })))
        .padding(4)
        .width(Length::Fill)
        .into()
}

fn api_key_fields(api_key: &ApiKeyAuthState) -> Element<'_, Message> {
    column![
        text_input("Header name", &api_key.header_name)
            .on_input(|header_name| {
                Message::AuthChanged(AuthState::ApiKey(ApiKeyAuthState {
                    header_name,
                    header_value: api_key.header_value.clone(),
                }))
            })
            .padding(4)
            .width(Length::Fill),
        text_input("Header value", &api_key.header_value)
            .on_input(|header_value| {
                Message::AuthChanged(AuthState::ApiKey(ApiKeyAuthState {
                    header_name: api_key.header_name.clone(),
                    header_value,
                }))
            })
            .padding(4)
            .width(Length::Fill),
    ]
    .spacing(4)
    .into()
}

fn basic_fields(basic: &BasicAuthState) -> Element<'_, Message> {
    column![
        text_input("Username", &basic.username)
            .on_input(|username| {
                Message::AuthChanged(AuthState::Basic(BasicAuthState {
                    username,
                    password: basic.password.clone(),
                }))
            })
            .padding(4)
            .width(Length::Fill),
        text_input("Password", &basic.password)
            .on_input(|password| {
                Message::AuthChanged(AuthState::Basic(BasicAuthState {
                    username: basic.username.clone(),
                    password,
                }))
            })
            .padding(4)
            .width(Length::Fill),
    ]
    .spacing(4)
    .into()
}

fn oauth2_client_credentials_fields(
    oauth: &OAuth2ClientCredentialsAuthState,
) -> Element<'_, Message> {
    let method_pick = pick_list(
        ClientSecretMethod::ALL.to_vec(),
        Some(oauth.client_secret_method),
        |client_secret_method| {
            Message::AuthChanged(AuthState::OAuth2ClientCredentials(
                OAuth2ClientCredentialsAuthState {
                    token_url: oauth.token_url.clone(),
                    client_id: oauth.client_id.clone(),
                    client_secret: oauth.client_secret.clone(),
                    scope: oauth.scope.clone(),
                    client_secret_method,
                },
            ))
        },
    )
    .width(Length::Fill);

    column![
        text_input("Token URL", &oauth.token_url)
            .on_input(|token_url| {
                Message::AuthChanged(AuthState::OAuth2ClientCredentials(
                    OAuth2ClientCredentialsAuthState {
                        token_url,
                        client_id: oauth.client_id.clone(),
                        client_secret: oauth.client_secret.clone(),
                        scope: oauth.scope.clone(),
                        client_secret_method: oauth.client_secret_method,
                    },
                ))
            })
            .padding(4)
            .width(Length::Fill),
        text_input("Client ID", &oauth.client_id)
            .on_input(|client_id| {
                Message::AuthChanged(AuthState::OAuth2ClientCredentials(
                    OAuth2ClientCredentialsAuthState {
                        token_url: oauth.token_url.clone(),
                        client_id,
                        client_secret: oauth.client_secret.clone(),
                        scope: oauth.scope.clone(),
                        client_secret_method: oauth.client_secret_method,
                    },
                ))
            })
            .padding(4)
            .width(Length::Fill),
        text_input("Client secret", &oauth.client_secret)
            .on_input(|client_secret| {
                Message::AuthChanged(AuthState::OAuth2ClientCredentials(
                    OAuth2ClientCredentialsAuthState {
                        token_url: oauth.token_url.clone(),
                        client_id: oauth.client_id.clone(),
                        client_secret,
                        scope: oauth.scope.clone(),
                        client_secret_method: oauth.client_secret_method,
                    },
                ))
            })
            .padding(4)
            .width(Length::Fill),
        text_input("Scope (optional)", &oauth.scope)
            .on_input(|scope| {
                Message::AuthChanged(AuthState::OAuth2ClientCredentials(
                    OAuth2ClientCredentialsAuthState {
                        token_url: oauth.token_url.clone(),
                        client_id: oauth.client_id.clone(),
                        client_secret: oauth.client_secret.clone(),
                        scope,
                        client_secret_method: oauth.client_secret_method,
                    },
                ))
            })
            .padding(4)
            .width(Length::Fill),
        method_pick,
    ]
    .spacing(4)
    .into()
}
