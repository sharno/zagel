use ego_tree::NodeRef;
use iced::widget::text::Wrapping;
use iced::widget::{
    button, column, container, pick_list, row, rule, scrollable, text, text_editor,
};
use iced::{Alignment, Element, Length};
use iced_highlighter::Theme as HighlightTheme;
use scraper::{Html, Node};

use super::super::Message;
use crate::model::ResponsePreview;
use crate::theme::{self, spacing, typo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxKind {
    Json,
    Html,
    Xml,
    JavaScript,
    Css,
    Text,
}

impl SyntaxKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Html => "html",
            Self::Xml => "xml",
            Self::JavaScript => "javascript",
            Self::Css => "css",
            Self::Text => "text",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtmlParseMode {
    Document,
    Fragment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrettyKind {
    Json,
    Html,
}

#[derive(Debug, Clone)]
pub enum PrettyBody {
    Json { pretty: String },
    Html { pretty: String },
}

impl PrettyBody {
    const fn kind(&self) -> PrettyKind {
        match self {
            Self::Json { .. } => PrettyKind::Json,
            Self::Html { .. } => PrettyKind::Html,
        }
    }

    fn text(&self) -> &str {
        match self {
            Self::Json { pretty } | Self::Html { pretty } => pretty,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResponseBodyData {
    raw: String,
    syntax: SyntaxKind,
    pretty: Option<PrettyBody>,
}

impl ResponseBodyData {
    pub fn from_response(resp: &ResponsePreview) -> Self {
        let raw = resp
            .error
            .clone()
            .or_else(|| resp.body.clone())
            .unwrap_or_else(|| "No body".to_string());
        let syntax = response_syntax_kind(resp);
        let pretty_json = pretty_json(&raw).map(|pretty| PrettyBody::Json { pretty });
        let pretty = pretty_json.or_else(|| {
            if syntax == SyntaxKind::Html {
                let mode = html_parse_mode(&raw);
                let pretty = pretty_html(&raw, mode);
                if pretty.is_empty() {
                    None
                } else {
                    Some(PrettyBody::Html { pretty })
                }
            } else {
                None
            }
        });

        Self {
            raw,
            syntax,
            pretty,
        }
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub const fn syntax(&self) -> SyntaxKind {
        self.syntax
    }

    pub fn pretty_text(&self) -> Option<&str> {
        self.pretty.as_ref().map(PrettyBody::text)
    }

    pub fn pretty_kind(&self) -> Option<PrettyKind> {
        self.pretty.as_ref().map(PrettyBody::kind)
    }
}

#[derive(Debug, Clone)]
pub struct ResponseData {
    pub preview: ResponsePreview,
    pub body: ResponseBodyData,
}

impl ResponseData {
    pub fn from_preview(preview: ResponsePreview) -> Self {
        let body = ResponseBodyData::from_response(&preview);
        Self { preview, body }
    }
}

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
    let body = button(text("Body").size(typo::BODY))
        .style(if current == ResponseTab::Body {
            button::primary
        } else {
            button::secondary
        })
        .padding([spacing::XXS, spacing::SM])
        .on_press(Message::ResponseTabChanged(ResponseTab::Body));
    let headers = button(text("Headers").size(typo::BODY))
        .style(if current == ResponseTab::Headers {
            button::primary
        } else {
            button::secondary
        })
        .padding([spacing::XXS, spacing::SM])
        .on_press(Message::ResponseTabChanged(ResponseTab::Headers));

    row![body, headers].spacing(spacing::XXS).into()
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
#[allow(clippy::too_many_lines)]
pub fn response_panel<'a>(
    response: Option<&'a ResponseData>,
    content: &'a text_editor::Content,
    display: ResponseDisplay,
    tab: ResponseTab,
    highlight_theme: HighlightTheme,
) -> Element<'a, Message> {
    response.map_or_else(
        || text("No response yet").size(typo::BODY).into(),
        |response| {
            let resp = &response.preview;
            let body = &response.body;

            // Status line with colored status code
            let header: Element<'_, Message> = match (resp.status, resp.duration) {
                (Some(status), Some(duration)) => {
                    let color = theme::status_color(status);
                    row![
                        text("HTTP").size(typo::HEADING),
                        text(status.to_string()).size(typo::HEADING).color(color),
                        text(format!("in {} ms", duration.as_millis())).size(typo::BODY),
                    ]
                    .spacing(spacing::XXS)
                    .align_y(Alignment::Center)
                    .into()
                }
                (Some(status), None) => {
                    let color = theme::status_color(status);
                    row![
                        text("HTTP").size(typo::HEADING),
                        text(status.to_string()).size(typo::HEADING).color(color),
                    ]
                    .spacing(spacing::XXS)
                    .align_y(Alignment::Center)
                    .into()
                }
                _ => text("No response").size(typo::HEADING).into(),
            };

            // Headers as a key-value table
            let headers_view: Element<'_, Message> = if resp.headers.is_empty() {
                text("No headers").size(typo::CAPTION).into()
            } else {
                let mut header_rows = column![].spacing(spacing::XXXS);
                for (name, value) in &resp.headers {
                    header_rows = header_rows.push(
                        row![
                            container(text(name).size(typo::CAPTION)).width(Length::FillPortion(2)),
                            container(text(value).size(typo::CAPTION))
                                .width(Length::FillPortion(5)),
                        ]
                        .spacing(spacing::SM)
                        .align_y(Alignment::Start),
                    );
                }
                scrollable(header_rows).height(Length::Fill).into()
            };

            let pretty_kind = body.pretty_kind();
            let syntax = body.syntax();
            let body_editor = text_editor(content)
                .height(Length::Fill)
                .highlight(syntax.as_str(), highlight_theme)
                .wrapping(Wrapping::None);

            let body_section: Element<'_, Message> = column![
                text(format!(
                    "Body ({})",
                    match (display, pretty_kind) {
                        (ResponseDisplay::Pretty, Some(PrettyKind::Html)) => {
                            "pretty (HTML; formatted view)"
                        }
                        (ResponseDisplay::Pretty, Some(PrettyKind::Json)) => "pretty (JSON)",
                        (ResponseDisplay::Pretty, None) => "pretty (raw shown)",
                        (ResponseDisplay::Raw, _) => "raw",
                    }
                ))
                .size(typo::CAPTION),
                body_editor,
            ]
            .spacing(spacing::XS)
            .into();

            let headers_section: Element<'_, Message> =
                column![text("Headers").size(typo::CAPTION), headers_view,]
                    .spacing(spacing::XS)
                    .into();

            let tab_view: Element<'_, Message> = match tab {
                ResponseTab::Body => body_section,
                ResponseTab::Headers => headers_section,
            };

            column![
                header,
                rule::horizontal(1),
                container(tab_view).height(Length::Fill),
            ]
            .spacing(spacing::SM)
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
pub fn pretty_html(raw: &str, mode: HtmlParseMode) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let document = match mode {
        HtmlParseMode::Document => Html::parse_document(trimmed),
        HtmlParseMode::Fragment => Html::parse_fragment(trimmed),
    };

    // List of void/self-closing tags that don't need closing tags
    let void_tags: std::collections::HashSet<&str> = [
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
        "source", "track", "wbr",
    ]
    .into_iter()
    .collect();

    let mut result = String::new();
    match mode {
        HtmlParseMode::Document => {
            format_node(
                document.tree.root(),
                &mut result,
                0,
                &void_tags,
                false,
                false,
            );
        }
        HtmlParseMode::Fragment => {
            let raw_has_body_tag = trimmed.to_ascii_lowercase().contains("<body");
            format_fragment_root(
                document.tree.root(),
                &mut result,
                &void_tags,
                raw_has_body_tag,
            );
        }
    }

    result
}

fn format_fragment_root(
    root: NodeRef<'_, Node>,
    output: &mut String,
    void_tags: &std::collections::HashSet<&str>,
    raw_has_body_tag: bool,
) {
    let element_children: Vec<_> = root
        .children()
        .filter(|child| matches!(child.value(), Node::Element(_)))
        .collect();

    if element_children.len() == 1
        && matches!(element_children[0].value(), Node::Element(element) if element.name.local.as_ref() == "html")
    {
        let html_node = element_children[0];
        let body_node = html_node
            .children()
            .find(|child| matches!(child.value(), Node::Element(element) if element.name.local.as_ref() == "body"));
        match (raw_has_body_tag, body_node) {
            (false, Some(body)) => {
                for child in body.children() {
                    format_node(child, output, 0, void_tags, false, false);
                }
            }
            (true, Some(body)) => {
                format_node(body, output, 0, void_tags, false, false);
            }
            (_, None) => {
                for child in html_node.children() {
                    format_node(child, output, 0, void_tags, false, false);
                }
            }
        }
        return;
    }

    for child in root.children() {
        format_node(child, output, 0, void_tags, false, false);
    }
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
                format_node(
                    child,
                    output,
                    indent,
                    void_tags,
                    in_preformatted,
                    should_indent,
                );
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
            let has_text_children = children
                .iter()
                .any(|child| matches!(child.value(), Node::Text(_)));
            let has_element_children = children
                .iter()
                .any(|child| matches!(child.value(), Node::Element(_)));
            let child_should_indent =
                has_element_children || (has_text_children && children.len() > 1);

            // Determine if children are in preformatted context
            let child_in_preformatted = in_preformatted || tag_is_preformatted;

            for child in children {
                if child_should_indent {
                    format_node(
                        child,
                        output,
                        indent + 1,
                        void_tags,
                        child_in_preformatted,
                        child_should_indent,
                    );
                } else {
                    // For inline formatting, pass 0 indent but preserve preformatted context
                    format_node(
                        child,
                        output,
                        0,
                        void_tags,
                        child_in_preformatted,
                        child_should_indent,
                    );
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

fn response_syntax_kind(resp: &ResponsePreview) -> SyntaxKind {
    let content_type = resp
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
        .map(|(_, value)| value.to_ascii_lowercase())
        .unwrap_or_default();

    if content_type.contains("json") {
        SyntaxKind::Json
    } else if content_type.contains("html") {
        SyntaxKind::Html
    } else if content_type.contains("xml") {
        SyntaxKind::Xml
    } else if content_type.contains("javascript") {
        SyntaxKind::JavaScript
    } else if content_type.contains("css") {
        SyntaxKind::Css
    } else {
        SyntaxKind::Text
    }
}

fn html_parse_mode(raw: &str) -> HtmlParseMode {
    let sniff = raw.trim_start().to_ascii_lowercase();
    if sniff.contains("<!doctype") || sniff.contains("<html") {
        HtmlParseMode::Document
    } else {
        HtmlParseMode::Fragment
    }
}

#[cfg(test)]
mod tests {
    use super::{html_parse_mode, pretty_html, HtmlParseMode};

    #[test]
    fn html_parse_mode_detects_document_markers() {
        assert_eq!(
            html_parse_mode("<!DOCTYPE html><html><body></body></html>"),
            HtmlParseMode::Document
        );
        assert_eq!(html_parse_mode("<HTML></HTML>"), HtmlParseMode::Document);
        assert_eq!(html_parse_mode("<div>ok</div>"), HtmlParseMode::Fragment);
    }

    #[test]
    fn pretty_html_fragment_keeps_fragment_shape() {
        let input = "Hello <b>world</b>";
        let formatted = pretty_html(input, HtmlParseMode::Fragment);
        assert_eq!(formatted, input);
    }
}
