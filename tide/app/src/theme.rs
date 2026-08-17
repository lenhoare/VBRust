//! Turbo Pascal colour scheme for the TIDE shell.

use ratatui::style::{Color, Modifier, Style};

/// Classic Turbo Pascal: blue editor, grey menus, cyan status.
pub struct TpTheme;

impl TpTheme {
    pub const DESKTOP: Color = Color::Blue;
    /// Classic TP menu bar: light grey, black text, red accelerator.
    pub const MENU_FG: Color = Color::Black;
    pub const MENU_BG: Color = Color::Rgb(192, 192, 192);
    pub const MENU_HOT: Color = Color::Red;
    pub const MENU_SEL_BG: Color = Color::Green;
    pub const MENU_SEL_FG: Color = Color::Black;
    pub const EDITOR_FG: Color = Color::Yellow;
    pub const EDITOR_BG: Color = Color::Blue;
    /// Turbo Debugger–ish generated-code strip.
    pub const RUST_FG: Color = Color::Black;
    pub const RUST_BG: Color = Color::Cyan;
    pub const RUST_HL_FG: Color = Color::Black;
    pub const RUST_HL_BG: Color = Color::Yellow;
    pub const STATUS_FG: Color = Color::Black;
    pub const STATUS_BG: Color = Color::Cyan;
    pub const FRAME: Color = Color::White;
    pub const DIALOG_BG: Color = Color::Cyan;
    pub const DIALOG_FG: Color = Color::Black;

    pub fn desktop() -> Style {
        Style::default().bg(Self::DESKTOP).fg(Self::EDITOR_FG)
    }

    pub fn menu() -> Style {
        Style::default()
            .bg(Self::MENU_BG)
            .fg(Self::MENU_FG)
            .add_modifier(Modifier::BOLD)
    }

    pub fn menu_hot(selected: bool) -> Style {
        Style::default()
            .bg(if selected {
                Self::MENU_SEL_BG
            } else {
                Self::MENU_BG
            })
            .fg(Self::MENU_HOT)
            .add_modifier(Modifier::BOLD)
    }

    pub fn menu_selected() -> Style {
        Style::default()
            .bg(Self::MENU_SEL_BG)
            .fg(Self::MENU_SEL_FG)
            .add_modifier(Modifier::BOLD)
    }

    pub fn editor() -> Style {
        Style::default().bg(Self::EDITOR_BG).fg(Self::EDITOR_FG)
    }

    pub fn rust_pane() -> Style {
        Style::default().bg(Self::RUST_BG).fg(Self::RUST_FG)
    }

    pub fn rust_mapped() -> Style {
        Style::default()
            .bg(Self::RUST_HL_BG)
            .fg(Self::RUST_HL_FG)
            .add_modifier(Modifier::BOLD)
    }

    pub fn status() -> Style {
        Style::default()
            .bg(Self::STATUS_BG)
            .fg(Self::STATUS_FG)
            .add_modifier(Modifier::BOLD)
    }

    pub fn dialog() -> Style {
        Style::default().bg(Self::DIALOG_BG).fg(Self::DIALOG_FG)
    }

    pub fn frame() -> Style {
        Style::default().fg(Self::FRAME).bg(Self::DESKTOP)
    }
}
