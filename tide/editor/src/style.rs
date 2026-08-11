//! Text styling for highlighted spans and decorations.

use ratatui::style::{Color, Modifier, Style};

/// A styled slice of a line: `[start, end)` in chars.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpanStyle {
    pub start: usize,
    pub end: usize,
    pub style: Style,
}

impl SpanStyle {
    pub fn new(start: usize, end: usize, style: Style) -> Self {
        Self { start, end, style }
    }
}

/// Span decoration overlaid on the document (diagnostics, search, etc.).
#[derive(Debug, Clone)]
pub struct Decoration {
    pub line: usize,
    pub start_col: usize,
    pub end_col: usize,
    pub style: Style,
}

impl Decoration {
    pub fn new(line: usize, start_col: usize, end_col: usize, style: Style) -> Self {
        Self {
            line,
            start_col,
            end_col,
            style,
        }
    }
}

/// Common token colours hosts can reuse.
pub mod palette {
    use super::*;

    pub fn keyword() -> Style {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    }

    pub fn string() -> Style {
        Style::default().fg(Color::LightGreen)
    }

    pub fn comment() -> Style {
        Style::default().fg(Color::DarkGray)
    }

    pub fn number() -> Style {
        Style::default().fg(Color::Cyan)
    }

    pub fn selection() -> Style {
        Style::default().bg(Color::DarkGray).fg(Color::White)
    }

    pub fn cursor_line() -> Style {
        Style::default().bg(Color::Indexed(17))
    }
}
