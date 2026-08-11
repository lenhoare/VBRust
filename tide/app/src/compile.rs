//! In-process VBR compile for Watch + Rust pane (errors → jump-to-line).

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
    /// Generated Rust (may be empty on hard front-end failure).
    pub rust: String,
    /// `(rust_line, vbr_line)` 1-based checkpoints, ascending by rust line.
    pub line_map: Vec<(usize, usize)>,
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
        rust: compiled.rust,
        line_map: compiled.line_map,
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

/// 0-based inclusive Rust line range generated from a 1-based VBR line.
pub fn rust_span_for_vbr(
    map: &[(usize, usize)],
    vbr_1: usize,
    rust_line_count: usize,
) -> Option<(usize, usize)> {
    if map.is_empty() || rust_line_count == 0 || vbr_1 == 0 {
        return None;
    }

    let mut start_r: Option<usize> = None;
    let mut end_r: Option<usize> = None;

    for i in 0..map.len() {
        let (r1, v) = map[i];
        if v != vbr_1 {
            continue;
        }
        let seg_end = match map.get(i + 1) {
            Some(&(next_r, _)) => next_r.saturating_sub(1).max(r1),
            None => rust_line_count.max(r1),
        };
        start_r = Some(start_r.map_or(r1, |s| s.min(r1)));
        end_r = Some(end_r.map_or(seg_end, |e| e.max(seg_end)));
    }

    if let (Some(s), Some(e)) = (start_r, end_r) {
        let s0 = s.saturating_sub(1);
        let e0 = e.saturating_sub(1).min(rust_line_count.saturating_sub(1));
        return Some((s0, e0.max(s0)));
    }

    // Nearest earlier checkpoint (same rule as rustc→VBR attribution).
    let (r, _) = map
        .iter()
        .rev()
        .find(|(_, v)| *v <= vbr_1)
        .or_else(|| map.first())?;
    let r0 = r.saturating_sub(1).min(rust_line_count.saturating_sub(1));
    Some((r0, r0))
}

/// 1-based VBR line for a 1-based Rust line (last checkpoint at or before it).
pub fn vbr_line_for_rust(map: &[(usize, usize)], rust_1: usize) -> Option<usize> {
    map.iter()
        .take_while(|(r, _)| *r <= rust_1)
        .last()
        .map(|(_, v)| *v)
}

/// Whole-line decorations for a Rust span (0-based inclusive).
pub fn rust_map_decorations(start0: usize, end0: usize, style: Style) -> Vec<Decoration> {
    (start0..=end0)
        .map(|line| Decoration::new(line, 0, usize::MAX / 4, style))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_span_covers_multi_line_region() {
        // Checkpoints: rust lines 10,14,20 from VBR 3,3,5
        let map = vec![(10, 3), (14, 3), (20, 5)];
        let (s, e) = rust_span_for_vbr(&map, 3, 40).unwrap();
        assert_eq!(s, 9); // 10-1
        assert_eq!(e, 18); // next checkpoint 20 → end 19, 0-based 18
    }

    #[test]
    fn vbr_from_rust_uses_last_checkpoint() {
        let map = vec![(10, 3), (14, 3), (20, 5)];
        assert_eq!(vbr_line_for_rust(&map, 12), Some(3));
        assert_eq!(vbr_line_for_rust(&map, 20), Some(5));
    }
}
