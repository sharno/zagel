use iced::widget::container;
use iced::{border, Theme};
use iced_highlighter::Theme as HighlightTheme;
use serde::{Deserialize, Serialize};

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

pub fn overlay_container_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();

    container::Style {
        background: Some(palette.background.weak.color.into()),
        text_color: Some(palette.background.weak.text),
        border: border::rounded(8.0)
            .width(1.0)
            .color(palette.background.strong.color),
        ..container::Style::default()
    }
}
