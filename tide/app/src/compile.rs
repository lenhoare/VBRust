//! In-process VBR compile for the watch window (errors → jump-to-line).

use ratatui::style::{Color, Modifier, Style};
use tide_editor::Decoration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagLevel {
    Error,
    Warning,
    Note,
}

#[derive(Debug, Clone)]
pub struct TideDiag {
    pub level: DiagLevel,
    pub message: String,
    /// 1-based VBR line when known.
    pub line: Option<usize>,
    /// 0-based start/end columns on that line (chars), when a span is known.
    pub start_col: Option<usize>,
    pub end_col: Option<usize>,
}

impl TideDiag {
    pub fn symbol(&self) -> char {
        match self.level {
            DiagLevel::Error => '✘',
            DiagLevel::Warning => '⚠',
            DiagLevel::Note => 'ℹ',
        }
    }

    pub fn render_line(&self) -> String {
        match self.line {
            Some(l) => format!("{} [{}] {}", self.symbol(), l, self.message),
            None => format!("{} {}", self.symbol(), self.message),
        }
    }
}

pub struct CompileOutcome {
    pub diagnostics: Vec<TideDiag>,
    pub has_errors: bool,
}

/// Compile the buffer with the in-process `vbr` library (same front-end as the CLI).
pub fn compile_buffer(source: &str) -> CompileOutcome {
    let compiled = vbr::compile(source);
    let diagnostics = compiled
        .diagnostic_items
        .iter()
        .map(|d| map_diag(source, d))
        .collect::<Vec<_>>();
    CompileOutcome {
        has_errors: compiled.has_errors,
        diagnostics,
    }
}

fn map_diag(source: &str, d: &vbr::diagnostics::Diagnostic) -> TideDiag {
    let level = match d.level {
        vbr::diagnostics::Level::Error => DiagLevel::Error,
        vbr::diagnostics::Level::Warning => DiagLevel::Warning,
        vbr::diagnostics::Level::Note => DiagLevel::Note,
    };
    let (start_col, end_col) = d
        .span
        .map(|s| span_cols(source, s.start, s.end))
        .unwrap_or((None, None));
    TideDiag {
        level,
        message: d.message.clone(),
        line: d.line,
        start_col,
        end_col,
    }
}

fn span_cols(source: &str, start: usize, end: usize) -> (Option<usize>, Option<usize>) {
    let start = start.min(source.len());
    let end = end.min(source.len());
    let line_start = source[..start].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let start_col = source[line_start..start].chars().count();
    let end_col = if end >= start {
        start_col + source[start..end].chars().count()
    } else {
        start_col
    };
    (Some(start_col), Some(end_col.max(start_col + 1)))
}

/// Editor decorations for the current diagnostic list.
pub fn decorations_for(diags: &[TideDiag]) -> Vec<Decoration> {
    let mut out = Vec::new();
    for d in diags {
        let Some(line1) = d.line else { continue };
        let line = line1.saturating_sub(1);
        let style = match d.level {
            DiagLevel::Error => Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::UNDERLINED | Modifier::BOLD),
            DiagLevel::Warning => Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::UNDERLINED),
            DiagLevel::Note => Style::default().fg(Color::Cyan),
        };
        let (start, end) = match (d.start_col, d.end_col) {
            (Some(s), Some(e)) => (s, e.max(s + 1)),
            _ => (0, usize::MAX / 4), // whole visible line painted in widget clamp
        };
        out.push(Decoration::new(line, start, end, style));
    }
    out
}

/// Jump target: 0-based line and preferred column.
pub fn jump_target(diag: &TideDiag) -> Option<(usize, usize)> {
    let line = diag.line?.saturating_sub(1);
    let col = diag.start_col.unwrap_or(0);
    Some((line, col))
}
