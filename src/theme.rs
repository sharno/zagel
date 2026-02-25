use iced::widget::{button, container};
use iced::{border, Color, Theme};
use iced_highlighter::Theme as HighlightTheme;
use serde::{Deserialize, Serialize};

use crate::model::Method;

// ---------------------------------------------------------------------------
// Design tokens — Cosmic DE-inspired spacing & corner-radius system
// ---------------------------------------------------------------------------

/// Spacing tokens (px): 4 / 8 / 12 / 16 / 24 / 32.
#[allow(dead_code)]
pub mod spacing {
    pub const XXXS: f32 = 2.0;
    pub const XXS: f32 = 4.0;
    pub const XS: f32 = 6.0;
    pub const SM: f32 = 8.0;
    pub const MD: f32 = 12.0;
    pub const LG: f32 = 16.0;
    pub const XL: f32 = 24.0;
    pub const XXL: f32 = 32.0;
}

/// Corner-radius tokens following Cosmic DE conventions.
#[allow(dead_code)]
pub mod radius {
    pub const NONE: f32 = 0.0;
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 6.0;
    pub const MD: f32 = 8.0;
    pub const LG: f32 = 12.0;
    pub const PILL: f32 = 160.0;
}

/// Typography scale.
pub mod typo {
    pub const CAPTION: f32 = 11.0;
    pub const BODY: f32 = 13.0;
    pub const HEADING: f32 = 14.0;
    pub const TITLE: f32 = 16.0;
}

// ---------------------------------------------------------------------------
// HTTP method colors (Postman-inspired)
// ---------------------------------------------------------------------------

/// Returns the badge color for an HTTP method.
pub const fn method_color(method: Method) -> Color {
    match method {
        Method::Get | Method::Head => Color::from_rgb(0.38, 0.82, 0.37), // #61D15F green
        Method::Post => Color::from_rgb(0.94, 0.68, 0.31),               // #F0AD4E amber
        Method::Put => Color::from_rgb(0.29, 0.56, 0.85),                // #4A90D9 blue
        Method::Patch => Color::from_rgb(0.69, 0.49, 0.86),              // #B07CDC purple
        Method::Delete => Color::from_rgb(0.90, 0.22, 0.21),             // #E53935 red
    }
}

// ---------------------------------------------------------------------------
// Response status-code colors
// ---------------------------------------------------------------------------

/// Returns a color for an HTTP status code.
pub const fn status_color(code: u16) -> Color {
    match code {
        200..=299 => Color::from_rgb(0.38, 0.82, 0.37), // 2xx = green
        300..=399 => Color::from_rgb(0.94, 0.68, 0.31), // 3xx = amber
        400..=499 => Color::from_rgb(0.90, 0.22, 0.21), // 4xx = red
        500..=599 => Color::from_rgb(0.85, 0.16, 0.16), // 5xx = darker red
        _ => Color::from_rgb(0.63, 0.63, 0.63),         // unknown = grey
    }
}

// ---------------------------------------------------------------------------
// Theme choice
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeChoice {
    #[default]
    CatppuccinMocha,
    TokyoNightStorm,
    Nord,
}

impl ThemeChoice {
    pub const fn iced_theme(self) -> Theme {
        match self {
            Self::CatppuccinMocha => Theme::CatppuccinMocha,
            Self::TokyoNightStorm => Theme::TokyoNightStorm,
            Self::Nord => Theme::Nord,
        }
    }

    pub const fn highlight_theme(self) -> HighlightTheme {
        match self {
            Self::CatppuccinMocha => HighlightTheme::Base16Mocha,
            Self::TokyoNightStorm | Self::Nord => HighlightTheme::Base16Ocean,
        }
    }
}

// ---------------------------------------------------------------------------
// Container styles
// ---------------------------------------------------------------------------

pub fn overlay_container_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weak.color.into()),
        text_color: Some(palette.background.weak.text),
        border: border::rounded(radius::MD)
            .width(1.0)
            .color(palette.background.strong.color),
        ..container::Style::default()
    }
}

/// A card-like section container with rounded corners and a subtle border.
pub fn section_container_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weak.color.into()),
        text_color: Some(palette.background.weak.text),
        border: border::rounded(radius::SM)
            .width(1.0)
            .color(palette.background.strong.color),
        ..container::Style::default()
    }
}

/// Status bar container style with a subtle top-tinted background.
pub fn status_bar_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    let bg = palette.background.strong.color;
    container::Style {
        background: Some(Color::from_rgba(bg.r, bg.g, bg.b, 0.5).into()),
        text_color: Some(palette.background.weak.text),
        border: border::rounded(radius::NONE),
        ..container::Style::default()
    }
}

/// Sidebar panel background — slightly darker for visual separation.
pub fn sidebar_container_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    let bg = palette.background.strong.color;

    container::Style {
        background: Some(Color::from_rgba(bg.r, bg.g, bg.b, 0.25).into()),
        text_color: None,
        border: border::rounded(radius::NONE),
        ..container::Style::default()
    }
}

/// Section title color derived from the current theme palette.
#[allow(dead_code)]
pub fn section_title_color(theme: &Theme) -> Color {
    let palette = theme.extended_palette();
    let txt = palette.background.weak.text;
    // Slightly dimmed version of the weak text color
    Color::from_rgba(txt.r, txt.g, txt.b, 0.7)
}

// ---------------------------------------------------------------------------
// Button styles — closures compatible with `button.style(fn)`
// ---------------------------------------------------------------------------

/// Accent / CTA button (e.g. Send). Prominent filled style.
pub fn accent_button_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let accent = palette.primary.strong;

    match status {
        button::Status::Active => button::Style {
            background: Some(accent.color.into()),
            text_color: accent.text,
            border: border::rounded(radius::SM),
            ..button::Style::default()
        },
        button::Status::Hovered => {
            let c = accent.color;
            let lighter = Color::from_rgba(
                (c.r + 0.08).min(1.0),
                (c.g + 0.08).min(1.0),
                (c.b + 0.08).min(1.0),
                1.0,
            );
            button::Style {
                background: Some(lighter.into()),
                text_color: accent.text,
                border: border::rounded(radius::SM),
                ..button::Style::default()
            }
        }
        button::Status::Pressed => {
            let c = accent.color;
            let darker = Color::from_rgba(
                (c.r - 0.06).max(0.0),
                (c.g - 0.06).max(0.0),
                (c.b - 0.06).max(0.0),
                1.0,
            );
            button::Style {
                background: Some(darker.into()),
                text_color: accent.text,
                border: border::rounded(radius::SM),
                ..button::Style::default()
            }
        }
        button::Status::Disabled => button::Style {
            background: Some(
                Color::from_rgba(accent.color.r, accent.color.g, accent.color.b, 0.4).into(),
            ),
            text_color: Color::from_rgba(accent.text.r, accent.text.g, accent.text.b, 0.5),
            border: border::rounded(radius::SM),
            ..button::Style::default()
        },
    }
}

/// Destructive button (e.g. Delete). Red-tinted style.
pub fn destructive_button_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let danger = palette.danger.strong;

    match status {
        button::Status::Active => button::Style {
            background: Some(danger.color.into()),
            text_color: danger.text,
            border: border::rounded(radius::SM),
            ..button::Style::default()
        },
        button::Status::Hovered => {
            let c = danger.color;
            let lighter = Color::from_rgba(
                (c.r + 0.08).min(1.0),
                (c.g + 0.08).min(1.0),
                (c.b + 0.08).min(1.0),
                1.0,
            );
            button::Style {
                background: Some(lighter.into()),
                text_color: danger.text,
                border: border::rounded(radius::SM),
                ..button::Style::default()
            }
        }
        button::Status::Pressed => {
            let c = danger.color;
            let darker = Color::from_rgba(
                (c.r - 0.06).max(0.0),
                (c.g - 0.06).max(0.0),
                (c.b - 0.06).max(0.0),
                1.0,
            );
            button::Style {
                background: Some(darker.into()),
                text_color: danger.text,
                border: border::rounded(radius::SM),
                ..button::Style::default()
            }
        }
        button::Status::Disabled => button::Style {
            background: Some(
                Color::from_rgba(danger.color.r, danger.color.g, danger.color.b, 0.4).into(),
            ),
            text_color: Color::from_rgba(danger.text.r, danger.text.g, danger.text.b, 0.5),
            border: border::rounded(radius::SM),
            ..button::Style::default()
        },
    }
}

/// Ghost / icon-only button — transparent background, visible on hover.
pub fn ghost_button_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let txt = palette.background.base.text;

    match status {
        button::Status::Active => button::Style {
            background: None,
            text_color: txt,
            border: border::rounded(radius::XS),
            ..button::Style::default()
        },
        button::Status::Hovered => {
            let bg = palette.background.weak.color;
            button::Style {
                background: Some(Color::from_rgba(bg.r, bg.g, bg.b, 0.3).into()),
                text_color: txt,
                border: border::rounded(radius::XS),
                ..button::Style::default()
            }
        }
        button::Status::Pressed => {
            let bg = palette.background.weak.color;
            button::Style {
                background: Some(Color::from_rgba(bg.r, bg.g, bg.b, 0.5).into()),
                text_color: txt,
                border: border::rounded(radius::XS),
                ..button::Style::default()
            }
        }
        button::Status::Disabled => button::Style {
            background: None,
            text_color: Color::from_rgba(txt.r, txt.g, txt.b, 0.3),
            border: border::rounded(radius::XS),
            ..button::Style::default()
        },
    }
}

/// Sidebar tree-item button — subtle background, highlight on hover/selected.
pub fn sidebar_item_style(theme: &Theme, status: button::Status, selected: bool) -> button::Style {
    let palette = theme.extended_palette();

    if selected {
        let accent = palette.primary.weak;
        return match status {
            button::Status::Active | button::Status::Pressed => button::Style {
                background: Some(accent.color.into()),
                text_color: accent.text,
                border: border::rounded(radius::XS),
                ..button::Style::default()
            },
            button::Status::Hovered => {
                let c = accent.color;
                let brighter = Color::from_rgba(
                    (c.r + 0.05).min(1.0),
                    (c.g + 0.05).min(1.0),
                    (c.b + 0.05).min(1.0),
                    1.0,
                );
                button::Style {
                    background: Some(brighter.into()),
                    text_color: accent.text,
                    border: border::rounded(radius::XS),
                    ..button::Style::default()
                }
            }
            button::Status::Disabled => button::Style {
                background: Some(accent.color.into()),
                text_color: Color::from_rgba(accent.text.r, accent.text.g, accent.text.b, 0.5),
                border: border::rounded(radius::XS),
                ..button::Style::default()
            },
        };
    }

    // Unselected
    let txt = palette.background.base.text;
    match status {
        button::Status::Active => button::Style {
            background: None,
            text_color: txt,
            border: border::rounded(radius::XS),
            ..button::Style::default()
        },
        button::Status::Hovered => {
            let bg = palette.background.weak.color;
            button::Style {
                background: Some(Color::from_rgba(bg.r, bg.g, bg.b, 0.35).into()),
                text_color: txt,
                border: border::rounded(radius::XS),
                ..button::Style::default()
            }
        }
        button::Status::Pressed => {
            let bg = palette.background.weak.color;
            button::Style {
                background: Some(Color::from_rgba(bg.r, bg.g, bg.b, 0.55).into()),
                text_color: txt,
                border: border::rounded(radius::XS),
                ..button::Style::default()
            }
        }
        button::Status::Disabled => button::Style {
            background: None,
            text_color: Color::from_rgba(txt.r, txt.g, txt.b, 0.3),
            border: border::rounded(radius::XS),
            ..button::Style::default()
        },
    }
}

/// Method badge button — colored background based on HTTP method.
#[allow(dead_code)]
pub fn method_badge_style(method: Method) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let color = method_color(method);
        let bg_alpha = match status {
            button::Status::Active => 0.18,
            button::Status::Hovered => 0.28,
            button::Status::Pressed => 0.35,
            button::Status::Disabled => 0.08,
        };
        let text_alpha = if matches!(status, button::Status::Disabled) {
            0.5
        } else {
            1.0
        };
        button::Style {
            background: Some(Color::from_rgba(color.r, color.g, color.b, bg_alpha).into()),
            text_color: Color::from_rgba(color.r, color.g, color.b, text_alpha),
            border: border::rounded(radius::XS),
            ..button::Style::default()
        }
    }
}
