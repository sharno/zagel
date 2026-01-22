use iced::widget::text::Wrapping;
use iced::widget::{button, column, container, pick_list, row, rule, text, text_editor};
use iced::{Element, Length};
use iced_highlighter::Theme as HighlightTheme;
use scraper::{Html, Node};
use ego_tree::NodeRef;

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

/// Creates a toggle widget for switching between Body and Headers tabs in the response view.
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

/// Creates a pick list widget for switching between Raw and Pretty response display modes.
pub fn response_view_toggle(current: ResponseDisplay) -> Element<'static, Message> {
    pick_list(
        ResponseDisplay::ALL.to_vec(),
        Some(current),
        Message::ResponseViewChanged,
    )
    .into()
}

/// Creates the main response panel UI element.
/// 
/// Displays the HTTP response status, duration, and either the body or headers
/// based on the selected tab. Supports both raw and pretty-printed views for
/// JSON and HTML content.
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
            // Check if HTML formatting would actually change the content
            // We check if formatting produces different output (meaning it worked)
            let body_is_pretty_html = if is_html {
                let formatted = pretty_html(&body_text);
                formatted != body_text.trim() && !formatted.is_empty()
            } else {
                false
            };
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

/// Attempts to format a JSON string with proper indentation.
/// 
/// Returns `Some(formatted_json)` if the input is valid JSON, otherwise returns `None`.
pub fn pretty_json(raw: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .map(|v| serde_json::to_string_pretty(&v).unwrap_or_else(|_| raw.to_string()))
}

/// Formats HTML with proper indentation using scraper (html5ever).
/// Handles malformed HTML gracefully by using html5ever's robust parsing.
pub fn pretty_html(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Parse HTML using scraper (which uses html5ever internally - very lenient)
    let document = Html::parse_document(trimmed);

    // List of void/self-closing tags that don't need closing tags
    let void_tags: std::collections::HashSet<&str> = [
        "area", "base", "br", "col", "embed", "hr", "img", "input",
        "link", "meta", "param", "source", "track", "wbr",
    ]
    .into_iter()
    .collect();

    // Format the DOM tree with proper indentation
    let mut result = String::new();
    format_node(document.tree.root(), &mut result, 0, &void_tags, false, false);

    result
}

/// Recursively formats an HTML node tree with proper indentation.
/// 
/// Preserves text content verbatim, especially within preformatted tags (pre, code, etc.).
/// Only adds newlines and indentation when `should_indent` is true to avoid
/// forcing formatting on inline-only content.
#[allow(clippy::too_many_lines)]
fn format_node(
    node: NodeRef<'_, Node>,
    output: &mut String,
    indent: usize,
    void_tags: &std::collections::HashSet<&str>,
    in_preformatted: bool,
    should_indent: bool,
) {
    // Tags that preserve whitespace and should not have their content modified
    const PREFORMATTED_TAGS: &[&str] = &["pre", "code", "textarea", "script", "style"];
    const INDENT_SIZE: usize = 2;
    let indent_str = " ".repeat(indent * INDENT_SIZE);

    match node.value() {
        Node::Document | Node::Fragment => {
            for child in node.children() {
                format_node(child, output, indent, void_tags, in_preformatted, should_indent);
            }
        }
        Node::Doctype(doctype) => {
            output.push_str("<!DOCTYPE ");
            output.push_str(&doctype.name);
            let pid_empty = doctype.public_id.is_empty();
            let sid_empty = doctype.system_id.is_empty();
            if !pid_empty {
                output.push_str(" PUBLIC \"");
                output.push_str(&doctype.public_id);
                output.push('"');
            }
            if !sid_empty {
                if pid_empty {
                    output.push_str(" SYSTEM \"");
                } else {
                    output.push_str(" \"");
                }
                output.push_str(&doctype.system_id);
                output.push('"');
            }
            output.push_str(">\n");
        }
        Node::Text(text) => {
            // In preformatted context, preserve text verbatim
            if in_preformatted {
                output.push_str(text);
            } else {
                // Outside preformatted context, skip purely whitespace nodes
                if !text.trim().is_empty() {
                    // Only add newline/indentation when should_indent is true
                    // and we're starting a new indented line
                    if should_indent && !output.ends_with('\n') && !output.is_empty() {
                        output.push('\n');
                        output.push_str(&indent_str);
                    }
                    // Preserve the text content verbatim (don't trim)
                    output.push_str(text);
                }
            }
        }
        Node::Element(element) => {
            let tag_name = element.name.local.as_ref();
            let is_void = void_tags.contains(tag_name);
            let tag_is_preformatted = PREFORMATTED_TAGS.contains(&tag_name);

            // Only add newline before opening tag if we're in an indented context
            if should_indent && !output.ends_with('\n') && !output.is_empty() {
                output.push('\n');
            }
            if should_indent {
                output.push_str(&indent_str);
            }
            output.push('<');
            output.push_str(tag_name);

            // Escape attribute values properly
            for (attr_name, attr_value) in &element.attrs {
                output.push(' ');
                output.push_str(attr_name.local.as_ref());
                output.push_str("=\"");
                // Escape & first, then <, >, and " to avoid double-escaping
                let value = attr_value
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;")
                    .replace('"', "&quot;");
                output.push_str(&value);
                output.push('"');
            }

            if is_void {
                output.push_str(" />");
            } else {
                output.push('>');
            }

            let children: Vec<_> = node.children().collect();
            let has_text_children = children.iter().any(|child| matches!(child.value(), Node::Text(_)));
            let has_element_children = children.iter().any(|child| matches!(child.value(), Node::Element(_)));
            let child_should_indent = has_element_children || (has_text_children && children.len() > 1);

            // Determine if children are in preformatted context
            let child_in_preformatted = in_preformatted || tag_is_preformatted;

            for child in children {
                if child_should_indent {
                    format_node(child, output, indent + 1, void_tags, child_in_preformatted, child_should_indent);
                } else {
                    // For inline formatting, pass 0 indent but preserve preformatted context
                    format_node(child, output, 0, void_tags, child_in_preformatted, child_should_indent);
                }
            }

            if !is_void {
                // Only add newline before closing tag if we're in an indented context
                if child_should_indent && !output.ends_with('\n') {
                    output.push('\n');
                }
                if child_should_indent {
                    output.push_str(&indent_str);
                }
                output.push_str("</");
                output.push_str(tag_name);
                output.push('>');
            }
        }
        Node::Comment(comment) => {
            if should_indent && !output.ends_with('\n') && !output.is_empty() {
                output.push('\n');
            }
            if should_indent {
                output.push_str(&indent_str);
            }
            output.push_str("<!--");
            output.push_str(comment);
            output.push_str("-->");
        }
        Node::ProcessingInstruction(_) => {}
    }
}

/// Determines the syntax highlighting language based on the response's Content-Type header.
/// 
/// Returns one of: "json", "html", "xml", "javascript", "css", or "text" (default).
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
