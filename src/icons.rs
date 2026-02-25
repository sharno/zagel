//! Centralized icon helpers using Bootstrap Icons via `iced_fonts`.
//!
//! All icon usage across the app should go through this module so that
//! swapping icon sets later is a single-file change.

use iced::widget::Text;

// Re-export frequently used icons under app-domain names.
// The underlying `iced_fonts::bootstrap::*` functions each return a
// `Text` widget pre-configured with the Bootstrap icon font.

/// Chevron-right: collapsed collection indicator.
pub fn chevron_right<'a>() -> Text<'a> {
    iced_fonts::bootstrap::chevron_right()
}

/// Chevron-down: expanded collection indicator.
pub fn chevron_down<'a>() -> Text<'a> {
    iced_fonts::bootstrap::chevron_down()
}

/// Check-square-fill: checked checkbox in edit mode.
pub fn check_square<'a>() -> Text<'a> {
    iced_fonts::bootstrap::check_square_fill()
}

/// Square: unchecked checkbox in edit mode.
pub fn square<'a>() -> Text<'a> {
    iced_fonts::bootstrap::square()
}

/// Arrow-up: move item up in edit mode.
pub fn arrow_up<'a>() -> Text<'a> {
    iced_fonts::bootstrap::arrow_up()
}

/// Arrow-down: move item down in edit mode.
pub fn arrow_down<'a>() -> Text<'a> {
    iced_fonts::bootstrap::arrow_down()
}

/// Arrow-right: selected item indicator.
pub fn arrow_right<'a>() -> Text<'a> {
    iced_fonts::bootstrap::arrow_right()
}

/// Plus-circle: add item.
pub fn plus_circle<'a>() -> Text<'a> {
    iced_fonts::bootstrap::plus_circle()
}

/// Trash: delete item.
pub fn trash<'a>() -> Text<'a> {
    iced_fonts::bootstrap::trash()
}

/// Pencil-square: edit mode toggle.
pub fn pencil<'a>() -> Text<'a> {
    iced_fonts::bootstrap::pencil_square()
}

/// Check-lg: done / confirm.
pub fn check_lg<'a>() -> Text<'a> {
    iced_fonts::bootstrap::check_lg()
}

/// Send-fill: send request.
pub fn send<'a>() -> Text<'a> {
    iced_fonts::bootstrap::send_fill()
}

/// Floppy: save.
pub fn save<'a>() -> Text<'a> {
    iced_fonts::bootstrap::floppy()
}

/// X-lg: close / remove.
pub fn x_lg<'a>() -> Text<'a> {
    iced_fonts::bootstrap::x_lg()
}

/// Plus-lg: add header.
pub fn plus_lg<'a>() -> Text<'a> {
    iced_fonts::bootstrap::plus_lg()
}

/// Clipboard: copy.
pub fn clipboard<'a>() -> Text<'a> {
    iced_fonts::bootstrap::clipboard()
}

/// Folder2-open: project folder.
pub fn folder_open<'a>() -> Text<'a> {
    iced_fonts::bootstrap::foldertwo_open()
}

/// Globe2: global environment.
pub fn globe<'a>() -> Text<'a> {
    iced_fonts::bootstrap::globe()
}

/// Collection: collections section.
pub fn collection<'a>() -> Text<'a> {
    iced_fonts::bootstrap::collection()
}

/// Question-circle: help / shortcuts.
pub fn question_circle<'a>() -> Text<'a> {
    iced_fonts::bootstrap::question_circle()
}

/// X-circle: close overlay.
pub fn x_circle<'a>() -> Text<'a> {
    iced_fonts::bootstrap::x_circle()
}

/// Dash-circle: remove project/env root.
pub fn dash_circle<'a>() -> Text<'a> {
    iced_fonts::bootstrap::dash_circle()
}

/// Hourglass-split: loading / in-flight request.
#[allow(dead_code)]
pub fn hourglass<'a>() -> Text<'a> {
    iced_fonts::bootstrap::hourglass_split()
}

/// Lock-fill: secure/password field indicator.
#[allow(dead_code)]
pub fn lock<'a>() -> Text<'a> {
    iced_fonts::bootstrap::lock_fill()
}

/// Key-fill: auth section icon.
pub fn key<'a>() -> Text<'a> {
    iced_fonts::bootstrap::key_fill()
}

/// Info-circle: meta section icon.
pub fn info_circle<'a>() -> Text<'a> {
    iced_fonts::bootstrap::info_circle()
}

/// Send (outline): request section icon.
pub fn send_icon<'a>() -> Text<'a> {
    iced_fonts::bootstrap::send()
}

/// List-ul: headers section icon.
pub fn list_ul<'a>() -> Text<'a> {
    iced_fonts::bootstrap::list_ul()
}

/// Code-slash: body section icon.
pub fn code_slash<'a>() -> Text<'a> {
    iced_fonts::bootstrap::code_slash()
}

/// Arrow-left-right: response section icon.
pub fn arrow_left_right<'a>() -> Text<'a> {
    iced_fonts::bootstrap::arrow_left_right()
}
