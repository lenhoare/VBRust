//! Ratatui widget that paints an [`EditorView`] over a [`Document`].

use crate::buffer::Document;
use crate::language::{Highlighter, PlainHighlighter};
use crate::style::{palette, Decoration};
use crate::view::EditorView;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthChar;

/// Configuration for painting the editor.
pub struct EditorWidget<'a> {
    pub doc: &'a Document,
    pub view: &'a EditorView,
    pub highlighter: &'a dyn Highlighter,
    pub decorations: &'a [Decoration],
    pub style: Style,
    pub cursor_style: Style,
    pub show_cursor: bool,
}

impl<'a> EditorWidget<'a> {
    pub fn new(doc: &'a Document, view: &'a EditorView) -> Self {
        static PLAIN: PlainHighlighter = PlainHighlighter;
        Self {
            doc,
            view,
            highlighter: &PLAIN,
            decorations: &[],
            style: Style::default().fg(Color::White).bg(Color::Blue),
            cursor_style: Style::default().bg(Color::Yellow).fg(Color::Black),
            show_cursor: true,
        }
    }

    pub fn highlighter(mut self, h: &'a dyn Highlighter) -> Self {
        self.highlighter = h;
        self
    }

    pub fn decorations(mut self, d: &'a [Decoration]) -> Self {
        self.decorations = d;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn show_cursor(mut self, show: bool) -> Self {
        self.show_cursor = show;
        self
    }
}

// Fix: EditorWidget::new can't use a static ref easily with trait object in
// older patterns — PlainHighlighter as static works.

impl Widget for EditorWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        // Clear
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(' ');
                    cell.set_style(self.style);
                }
            }
        }

        let gutter = self.view.gutter_width(self.doc);
        let text_cols = area.width.saturating_sub(gutter) as usize;
        let text_rows = area.height as usize;

        // Caller should have scrolled; clamp again so a stale overscroll never paints
        // past EOF (corrupt gutters / blank pages).
        let line_count = self.doc.len_lines().max(1);
        let max_scroll = line_count.saturating_sub(text_rows);
        let scroll_row = self.view.scroll_row.min(max_scroll);
        let scroll_col = self.view.scroll_col;
        let (sel_a, sel_b) = self.view.selection.range();
        let (cur_line, cur_col) = self.view.cursor_position(self.doc);

        for row in 0..text_rows {
            let line_idx = scroll_row + row;
            let y = area.y + row as u16;
            if line_idx >= line_count {
                // Past EOF — blank gutter only
                if gutter > 0 {
                    paint_gutter(buf, area.x, y, gutter, None, self.style);
                }
                continue;
            }

            if gutter > 0 {
                paint_gutter(buf, area.x, y, gutter, Some(line_idx + 1), self.style);
            }

            let line = self.doc.line(line_idx);
            let line_start = self.doc.line_to_char(line_idx);
            let highlights = self.highlighter.highlight(&line);

            // Build per-char styles
            let chars: Vec<char> = line.chars().collect();
            let mut styles = vec![self.style; chars.len()];

            // Cursor line background
            if line_idx == cur_line {
                let cl = palette::cursor_line();
                for s in styles.iter_mut() {
                    *s = s.patch(cl);
                }
            }

            for span in &highlights {
                for i in span.start..span.end.min(chars.len()) {
                    styles[i] = styles[i].patch(span.style);
                }
            }

            for deco in self.decorations.iter().filter(|d| d.line == line_idx) {
                for i in deco.start_col..deco.end_col.min(chars.len()) {
                    styles[i] = styles[i].patch(deco.style);
                }
            }

            // Selection
            if sel_a != sel_b {
                for (i, style) in styles.iter_mut().enumerate() {
                    let abs = line_start + i;
                    if abs >= sel_a && abs < sel_b {
                        *style = style.patch(palette::selection());
                    }
                }
            }

            // Paint visible columns
            let mut x = area.x + gutter;
            let mut col = 0usize;
            while col < chars.len() && x < area.right() {
                if col < scroll_col {
                    col += 1;
                    continue;
                }
                if col - scroll_col >= text_cols {
                    break;
                }
                let ch = match chars[col] {
                    '\t' => ' ',
                    c if c.is_control() => '·', // never emit \r etc. into the terminal
                    c => c,
                };
                let width = UnicodeWidthChar::width(ch).unwrap_or(1).max(1) as u16;
                if x + width > area.right() {
                    break;
                }
                let mut style = styles[col];
                if self.show_cursor && line_idx == cur_line && col == cur_col {
                    style = style.patch(self.cursor_style);
                }
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char(ch);
                    cell.set_style(style);
                }
                // Clear the trailing half of wide glyphs so they don't smear.
                if width > 1 {
                    for dx in 1..width {
                        if let Some(cell) = buf.cell_mut((x + dx, y)) {
                            cell.set_char(' ');
                            cell.set_style(style);
                        }
                    }
                }
                x += width;
                col += 1;
            }

            // Cursor past end of line
            if self.show_cursor
                && line_idx == cur_line
                && cur_col >= chars.len()
                && cur_col >= scroll_col
            {
                let visual = cur_col - scroll_col;
                if visual < text_cols {
                    let cx = area.x + gutter + visual as u16;
                    if cx < area.right() {
                        if let Some(cell) = buf.cell_mut((cx, y)) {
                            if cell.symbol() == " " || cell.symbol().is_empty() {
                                cell.set_char(' ');
                            }
                            cell.set_style(cell.style().patch(self.cursor_style));
                        }
                    }
                }
            }
        }
    }
}

fn paint_gutter(buf: &mut Buffer, x: u16, y: u16, width: u16, num: Option<usize>, base: Style) {
    let gutter_style = base.fg(Color::Gray);
    let w = width as usize;
    if w == 0 {
        return;
    }
    // Exact `width` cells — never spill into the text area (format's width is a
    // *minimum*, so a too-narrow gutter must truncate, not overflow).
    let label = match num {
        Some(n) => {
            let digits = w.saturating_sub(1).max(1);
            let raw = n.to_string();
            let body = if raw.len() > digits {
                raw[raw.len() - digits..].to_string()
            } else {
                format!("{raw:>digits$}")
            };
            format!("{body} ")
        }
        None => " ".repeat(w),
    };
    for (i, ch) in label.chars().take(w).enumerate() {
        if let Some(cell) = buf.cell_mut((x + i as u16, y)) {
            cell.set_char(ch);
            cell.set_style(gutter_style);
        }
    }
}
