use iced::widget::text::Wrapping;
use iced::widget::{button, column, container, pick_list, row, rule, text, text_editor};
use iced::{Element, Length};
use iced_highlighter::Theme as HighlightTheme;
use regex::Regex;

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

            let is_html = response_syntax(resp) == "html";
            let body_is_pretty_json = pretty_json(&body_text).is_some();
            let body_is_pretty_html = is_html && pretty_html(&body_text).is_some();
            let body_is_pretty = body_is_pretty_json || body_is_pretty_html;
            let syntax = response_syntax(resp);
            let body_editor = text_editor(content)
                .height(Length::Fill)
                .highlight(syntax, highlight_theme)
                .wrapping(Wrapping::None);

            let body_section: Element<'_, Message> = column![
                text(format!(
                    "Body ({})",
                    match display {
                        ResponseDisplay::Pretty if body_is_pretty => {
                            if body_is_pretty_html {
                                "pretty (HTML)"
                            } else if body_is_pretty_json {
                                "pretty (JSON)"
                            } else {
                                "pretty"
                            }
                        }
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

/// Formats HTML with proper indentation using regex-based formatting.
/// Handles malformed HTML gracefully by auto-closing implicit tags and maintaining proper indentation.
/// Returns None if the HTML appears invalid, otherwise returns formatted HTML.
pub fn pretty_html(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Some(String::new());
    }

    // List of void/self-closing tags that don't need closing tags
    let void_tags: std::collections::HashSet<&str> = [
        "area", "base", "br", "col", "embed", "hr", "img", "input",
        "link", "meta", "param", "source", "track", "wbr",
    ]
    .into_iter()
    .collect();

    // Tags that implicitly close certain other tags when they appear
    // For example, <body> implicitly closes <head>
    let implicit_closers: std::collections::HashMap<&str, Vec<&str>> = [
        ("body", vec!["head"]),
        ("head", vec!["head"]), // New <head> closes previous <head>
        ("html", vec!["head", "body"]),
    ]
    .into_iter()
    .collect();

    // Regex pattern to match HTML tags
    let tag_re = Regex::new(r"<(/?)([a-zA-Z][a-zA-Z0-9]*)\b[^>]*(/?)>").ok()?;
    let doctype_re = Regex::new(r"<!DOCTYPE[^>]*>").ok()?;

    let mut result = String::new();
    let mut indent_level = 0;
    const INDENT_SIZE: usize = 2;

    // Extract and preserve DOCTYPE
    let mut processed = trimmed.to_string();
    if let Some(dt_match) = doctype_re.find(trimmed) {
        let dt = dt_match.as_str();
        result.push_str(dt);
        result.push('\n');
        processed = processed.replace(dt, "");
    }

    // Process tags
    let mut last_end = 0;
    let mut tag_stack: Vec<String> = Vec::new();

    for cap in tag_re.captures_iter(&processed) {
        let full_match = cap.get(0)?;
        let is_closing = !cap.get(1)?.as_str().is_empty();
        let tag_name = cap.get(2)?.as_str().to_lowercase();
        let has_self_close = !cap.get(3)?.as_str().is_empty();
        let is_void = void_tags.contains(tag_name.as_str());
        let is_self_closing = has_self_close || is_void;

        // Add text content before this tag
        let text_before = &processed[last_end..full_match.start()];
        let trimmed_text = text_before.trim();
        if !trimmed_text.is_empty() {
            if !result.ends_with('\n') && !result.is_empty() {
                result.push('\n');
            }
            result.push_str(&" ".repeat(indent_level * INDENT_SIZE));
            result.push_str(trimmed_text);
        }

        if is_closing {
            // Closing tag - find and close matching tag in stack
            // Close all tags until we find the matching one
            while let Some(last_tag) = tag_stack.last() {
                if last_tag == &tag_name {
                    tag_stack.pop();
                    indent_level = indent_level.saturating_sub(1);
                    break;
                } else {
                    // Implicitly close mismatched tags
                    tag_stack.pop();
                    indent_level = indent_level.saturating_sub(1);
                }
            }
            
            if !result.ends_with('\n') && !result.is_empty() {
                result.push('\n');
            }
            result.push_str(&" ".repeat(indent_level * INDENT_SIZE));
            result.push_str(full_match.as_str());
        } else if is_self_closing {
            // Self-closing tag
            if !result.ends_with('\n') && !result.is_empty() {
                result.push('\n');
            }
            result.push_str(&" ".repeat(indent_level * INDENT_SIZE));
            result.push_str(full_match.as_str());
        } else {
            // Opening tag - check for implicit closers
            if let Some(tags_to_close) = implicit_closers.get(tag_name.as_str()) {
                // Close any tags that should be implicitly closed
                while let Some(last_tag) = tag_stack.last() {
                    if tags_to_close.iter().any(|&tag| tag == last_tag.as_str()) {
                        tag_stack.pop();
                        indent_level = indent_level.saturating_sub(1);
                    } else {
                        break;
                    }
                }
            }
            
            if !result.ends_with('\n') && !result.is_empty() {
                result.push('\n');
            }
            result.push_str(&" ".repeat(indent_level * INDENT_SIZE));
            result.push_str(full_match.as_str());
            tag_stack.push(tag_name);
            indent_level += 1;
        }

        last_end = full_match.end();
    }

    // Add any remaining text
    let remaining = &processed[last_end..];
    let trimmed_remaining = remaining.trim();
    if !trimmed_remaining.is_empty() {
        if !result.ends_with('\n') && !result.is_empty() {
            result.push('\n');
        }
        result.push_str(&" ".repeat(indent_level * INDENT_SIZE));
        result.push_str(trimmed_remaining);
    }

    // Clean up: remove excessive blank lines
    let lines: Vec<&str> = result.lines().collect();
    let mut formatted = Vec::new();
    let mut prev_empty = false;

    for line in lines {
        let trimmed_line = line.trim();
        if trimmed_line.is_empty() {
            if !prev_empty && !formatted.is_empty() {
                formatted.push(String::new());
            }
            prev_empty = true;
        } else {
            formatted.push(line.to_string());
            prev_empty = false;
        }
    }

    Some(formatted.join("\n"))
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
