//! Turbo Pascal–ish colours for the designer chrome.

use ratatui::style::{Color, Modifier, Style};

pub struct Theme;

impl Theme {
    pub const DESKTOP: Color = Color::Blue;
    pub const FG: Color = Color::Yellow;
    pub const SEL_BG: Color = Color::Green;
    pub const SEL_FG: Color = Color::Black;
    pub const PREVIEW_BG: Color = Color::Black;
    pub const PREVIEW_FG: Color = Color::White;
    pub const HL_BG: Color = Color::Cyan;
    pub const HL_FG: Color = Color::Black;
    pub const STATUS_BG: Color = Color::Cyan;
    pub const STATUS_FG: Color = Color::Black;
    pub const DIALOG_BG: Color = Color::Cyan;
    pub const DIALOG_FG: Color = Color::Black;

    pub fn desktop() -> Style {
        Style::default().bg(Self::DESKTOP).fg(Self::FG)
    }

    pub fn panel() -> Style {
        Style::default().bg(Self::DESKTOP).fg(Self::FG)
    }

    pub fn menu() -> Style {
        Self::panel()
    }

    pub fn menu_selected() -> Style {
        Self::selected()
    }

    pub fn selected() -> Style {
        Style::default()
            .bg(Self::SEL_BG)
            .fg(Self::SEL_FG)
            .add_modifier(Modifier::BOLD)
    }

    pub fn preview() -> Style {
        Style::default().bg(Self::PREVIEW_BG).fg(Self::PREVIEW_FG)
    }

    pub fn highlight() -> Style {
        Style::default().bg(Self::HL_BG).fg(Self::HL_FG)
    }

    pub fn status() -> Style {
        Style::default().bg(Self::STATUS_BG).fg(Self::STATUS_FG)
    }

    pub fn dialog() -> Style {
        Style::default().bg(Self::DIALOG_BG).fg(Self::DIALOG_FG)
    }

    pub fn frame() -> Style {
        Style::default().fg(Color::White).bg(Self::DESKTOP)
    }
}
