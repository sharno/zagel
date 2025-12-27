use iced::widget::text::Wrapping;
use iced::widget::{button, column, container, pick_list, row, rule, text, text_editor};
use iced::{Element, Length};
use iced_highlighter::Theme as HighlightTheme;

use super::super::Message;
use crate::model::ResponsePreview;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseDisplay {
    Raw,
    Pretty,
}

impl ResponseDisplay {
    pub const ALL: [Self; 2] = [Self::Raw, Self::Pretty];
}

impl std::fmt::Display for ResponseDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Raw => f.write_str("Raw"),
            Self::Pretty => f.write_str("Pretty"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseTab {
    Body,
    Headers,
}

impl std::fmt::Display for ResponseTab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Body => f.write_str("Body"),
            Self::Headers => f.write_str("Headers"),
        }
    }
}

pub fn response_tab_toggle(current: ResponseTab) -> Element<'static, Message> {
    let body = button(text("Body"))
        .style(if current == ResponseTab::Body {
            button::primary
        } else {
            button::secondary
        })
        .on_press(Message::ResponseTabChanged(ResponseTab::Body));
    let headers = button(text("Headers"))
        .style(if current == ResponseTab::Headers {
            button::primary
        } else {
            button::secondary
        })
        .on_press(Message::ResponseTabChanged(ResponseTab::Headers));

    row![body, headers].spacing(6).into()
}

pub fn response_view_toggle(current: ResponseDisplay) -> Element<'static, Message> {
    pick_list(
        ResponseDisplay::ALL.to_vec(),
        Some(current),
        Message::ResponseViewChanged,
    )
    .into()
}

pub fn response_panel<'a>(
    response: Option<&ResponsePreview>,
    content: &'a text_editor::Content,
    display: ResponseDisplay,
    tab: ResponseTab,
    highlight_theme: HighlightTheme,
) -> Element<'a, Message> {
    response.map_or_else(
        || text("No response yet").into(),
        |resp| {
            let header = match (resp.status, resp.duration) {
                (Some(status), Some(duration)) => {
                    format!("HTTP {status} in {} ms", duration.as_millis())
                }
                (Some(status), None) => format!("HTTP {status}"),
                _ => "No response".to_string(),
            };

            let body_text = resp
                .error
                .clone()
                .or_else(|| resp.body.clone())
                .unwrap_or_else(|| "No body".to_string());

            let mut headers_view = column![];
            if resp.headers.is_empty() {
                headers_view = headers_view.push(text("No headers").size(12));
            } else {
                for (name, value) in &resp.headers {
                    headers_view = headers_view.push(text(format!("{name}: {value}")).size(12));
                }
            }

            let body_is_pretty = pretty_json(&body_text).is_some();
            let syntax = response_syntax(resp);
            let body_editor = text_editor(content)
                .height(Length::Fill)
                .highlight(syntax, highlight_theme)
                .wrapping(Wrapping::None);

            let body_section: Element<'_, Message> = column![
                text(format!(
                    "Body ({})",
                    match display {
                        ResponseDisplay::Pretty if body_is_pretty => "pretty",
                        ResponseDisplay::Pretty => "pretty (raw shown)",
                        ResponseDisplay::Raw => "raw",
                    }
                ))
                .size(14),
                body_editor,
            ]
            .spacing(6)
            .into();

            let headers_section: Element<'_, Message> =
                column![text("Headers").size(14), headers_view.spacing(4),]
                    .spacing(6)
                    .into();

            let tab_view: Element<'_, Message> = match tab {
                ResponseTab::Body => body_section,
                ResponseTab::Headers => headers_section,
            };

            column![
                text(header).size(16),
                rule::horizontal(1),
                container(tab_view).height(Length::Fill),
            ]
            .spacing(6)
            .height(Length::Fill)
            .into()
        },
    )
}

pub fn pretty_json(raw: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .map(|v| serde_json::to_string_pretty(&v).unwrap_or_else(|_| raw.to_string()))
}

fn response_syntax(resp: &ResponsePreview) -> &'static str {
    let content_type = resp
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
        .map(|(_, value)| value.to_ascii_lowercase())
        .unwrap_or_default();

    if content_type.contains("json") {
        "json"
    } else if content_type.contains("html") {
        "html"
    } else if content_type.contains("xml") {
        "xml"
    } else if content_type.contains("javascript") {
        "javascript"
    } else if content_type.contains("css") {
        "css"
    } else {
        "text"
    }
}
