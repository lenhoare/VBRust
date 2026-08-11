//! Cursor, selection, scroll, and key handling over a [`Document`](crate::Document).

use crate::buffer::Document;
use crate::input::{Key, KeyEvent};

/// Inclusive-exclusive selection in char offsets: `[anchor, head)` is not used —
/// both ends are stored; the range is `min..=max` for rendering, edits use
/// `min..max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Selection {
    pub anchor: usize,
    pub head: usize,
}

impl Selection {
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    pub fn range(&self) -> (usize, usize) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }
}

/// Editor view state bound to a document.
#[derive(Debug, Clone)]
pub struct EditorView {
    pub cursor: usize,
    pub selection: Selection,
    pub scroll_row: usize,
    pub scroll_col: usize,
    pub show_line_numbers: bool,
    /// Preferred column for vertical movement (sticky column).
    preferred_col: usize,
    clipboard: String,
    /// True while the left mouse button is dragging a selection.
    mouse_selecting: bool,
}

impl Default for EditorView {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorView {
    pub fn new() -> Self {
        Self {
            cursor: 0,
            selection: Selection::default(),
            scroll_row: 0,
            scroll_col: 0,
            show_line_numbers: true,
            preferred_col: 0,
            clipboard: String::new(),
            mouse_selecting: false,
        }
    }

    pub fn cursor_position(&self, doc: &Document) -> (usize, usize) {
        doc.char_to_position(self.cursor)
    }

    pub fn has_selection(&self) -> bool {
        !self.selection.is_empty()
    }

    pub fn selected_text(&self, doc: &Document) -> String {
        let (a, b) = self.selection.range();
        if a == b || doc.is_empty() {
            return String::new();
        }
        let b = b.min(doc.len_chars());
        let a = a.min(b);
        // Extract via positions — rebuild from char range using line iteration.
        let text = doc.text();
        text.chars().skip(a).take(b - a).collect()
    }

    /// Ensure cursor is visible within a viewport of `rows` × `cols` text cells
    /// (excluding gutter).
    pub fn ensure_visible(&mut self, doc: &Document, rows: usize, cols: usize) {
        if rows == 0 {
            return;
        }
        let (line, col) = self.cursor_position(doc);
        if line < self.scroll_row {
            self.scroll_row = line;
        } else if line >= self.scroll_row + rows {
            self.scroll_row = line + 1 - rows;
        }
        if cols > 0 {
            if col < self.scroll_col {
                self.scroll_col = col;
            } else if col >= self.scroll_col + cols {
                self.scroll_col = col + 1 - cols;
            }
        }
    }

    pub fn gutter_width(&self, doc: &Document) -> u16 {
        if !self.show_line_numbers {
            return 0;
        }
        let n = doc.len_lines().max(1);
        let digits = ((n as f64).log10().floor() as usize) + 1;
        (digits + 1) as u16 // space after number
    }

    /// Handle a key. Returns `true` if the document or view changed.
    pub fn handle_key(&mut self, doc: &mut Document, ev: &KeyEvent) -> bool {
        // Some terminals send Ctrl+C as ETX ('\x03') without a Ctrl modifier.
        let ctrlish = (ev.mods.ctrl && !ev.mods.alt)
            || matches!(ev.key, Key::Char('\u{3}'));

        // Clipboard chords
        if ctrlish {
            match ev.key {
                Key::Char('z') | Key::Char('Z') if ev.mods.ctrl => {
                    if let Some(c) = doc.undo() {
                        self.set_cursor(doc, c, false);
                        return true;
                    }
                    return false;
                }
                Key::Char('y') | Key::Char('Y') if ev.mods.ctrl => {
                    if let Some(c) = doc.redo() {
                        self.set_cursor(doc, c, false);
                        return true;
                    }
                    return false;
                }
                Key::Char('a') | Key::Char('A') if ev.mods.ctrl => {
                    self.selection.anchor = 0;
                    self.selection.head = doc.len_chars();
                    self.cursor = doc.len_chars();
                    return true;
                }
                Key::Char('c') | Key::Char('C') | Key::Char('\u{3}') => {
                    let text = if self.has_selection() {
                        self.selected_text(doc)
                    } else {
                        let (line, _) = self.cursor_position(doc);
                        let mut line_text = doc.line(line);
                        if line + 1 < doc.len_lines() {
                            line_text.push('\n');
                        }
                        line_text
                    };
                    crate::clipboard::copy_to_clipboard(&mut self.clipboard, &text);
                    return false;
                }
                Key::Char('x') | Key::Char('X') if ev.mods.ctrl => {
                    if self.has_selection() {
                        let text = self.selected_text(doc);
                        crate::clipboard::copy_to_clipboard(&mut self.clipboard, &text);
                        self.delete_selection(doc);
                        return true;
                    }
                    return false;
                }
                Key::Char('v') | Key::Char('V') if ev.mods.ctrl => {
                    let text = crate::clipboard::paste_from_clipboard(&self.clipboard);
                    if !text.is_empty() {
                        self.clipboard = text.clone();
                        self.delete_selection(doc);
                        let pos = self.cursor;
                        doc.insert(pos, &text);
                        self.set_cursor(doc, pos + text.chars().count(), false);
                        return true;
                    }
                    return false;
                }
                _ => {}
            }
        }

        match (&ev.key, ev.mods) {
            (Key::Left, m) => {
                self.move_left(doc, m.shift);
                true
            }
            (Key::Right, m) => {
                self.move_right(doc, m.shift);
                true
            }
            (Key::Up, m) => {
                self.move_up(doc, m.shift);
                true
            }
            (Key::Down, m) => {
                self.move_down(doc, m.shift);
                true
            }
            (Key::Home, m) => {
                let (line, _) = self.cursor_position(doc);
                let pos = doc.line_to_char(line);
                self.set_cursor(doc, pos, m.shift);
                true
            }
            (Key::End, m) => {
                let (line, _) = self.cursor_position(doc);
                let pos = doc.position_to_char(line, doc.line_len(line));
                self.set_cursor(doc, pos, m.shift);
                true
            }
            (Key::PageUp, m) => {
                for _ in 0..20 {
                    self.move_up(doc, m.shift);
                }
                true
            }
            (Key::PageDown, m) => {
                for _ in 0..20 {
                    self.move_down(doc, m.shift);
                }
                true
            }
            (Key::Backspace, _) => {
                if self.has_selection() {
                    self.delete_selection(doc);
                } else if self.cursor > 0 {
                    doc.delete_range(self.cursor - 1, self.cursor);
                    self.set_cursor(doc, self.cursor - 1, false);
                }
                true
            }
            (Key::Delete, _) => {
                if self.has_selection() {
                    self.delete_selection(doc);
                } else if self.cursor < doc.len_chars() {
                    doc.delete_range(self.cursor, self.cursor + 1);
                }
                true
            }
            (Key::Enter, _) => {
                self.delete_selection(doc);
                doc.insert(self.cursor, "\n");
                self.set_cursor(doc, self.cursor + 1, false);
                true
            }
            (Key::Tab, _) => {
                self.delete_selection(doc);
                doc.insert(self.cursor, "    ");
                self.set_cursor(doc, self.cursor + 4, false);
                true
            }
            (Key::Char(c), m) if !m.ctrl && !m.alt => {
                self.delete_selection(doc);
                let s = c.to_string();
                doc.insert(self.cursor, &s);
                self.set_cursor(doc, self.cursor + 1, false);
                true
            }
            _ => false,
        }
    }

    fn delete_selection(&mut self, doc: &mut Document) {
        if !self.has_selection() {
            return;
        }
        let (a, b) = self.selection.range();
        doc.delete_range(a, b);
        self.set_cursor(doc, a, false);
    }

    fn set_cursor(&mut self, doc: &Document, pos: usize, extend_selection: bool) {
        let pos = pos.min(doc.len_chars());
        self.cursor = pos;
        if extend_selection {
            self.selection.head = pos;
        } else {
            self.selection.anchor = pos;
            self.selection.head = pos;
        }
        let (_, col) = doc.char_to_position(pos);
        self.preferred_col = col;
    }

    fn move_left(&mut self, doc: &Document, extend: bool) {
        if self.cursor > 0 {
            self.set_cursor(doc, self.cursor - 1, extend);
        } else if !extend {
            self.set_cursor(doc, 0, false);
        }
    }

    fn move_right(&mut self, doc: &Document, extend: bool) {
        if self.cursor < doc.len_chars() {
            self.set_cursor(doc, self.cursor + 1, extend);
        }
    }

    fn move_up(&mut self, doc: &Document, extend: bool) {
        let (line, _) = self.cursor_position(doc);
        if line == 0 {
            self.set_cursor(doc, 0, extend);
            return;
        }
        let col = self.preferred_col;
        let pos = doc.position_to_char(line - 1, col);
        // Preserve preferred_col across vertical moves
        let preferred = self.preferred_col;
        self.set_cursor(doc, pos, extend);
        self.preferred_col = preferred;
    }

    fn move_down(&mut self, doc: &Document, extend: bool) {
        let (line, _) = self.cursor_position(doc);
        let last = doc.len_lines().saturating_sub(1);
        if line >= last {
            let pos = doc.len_chars();
            let preferred = self.preferred_col;
            self.set_cursor(doc, pos, extend);
            self.preferred_col = preferred;
            return;
        }
        let col = self.preferred_col;
        let pos = doc.position_to_char(line + 1, col);
        let preferred = self.preferred_col;
        self.set_cursor(doc, pos, extend);
        self.preferred_col = preferred;
    }

    /// Set clipboard from the outside (e.g. system paste).
    pub fn set_clipboard(&mut self, text: String) {
        self.clipboard = text;
    }

    pub fn clipboard(&self) -> &str {
        &self.clipboard
    }

    /// Insert text at the cursor (replacing any selection). Used for bracketed paste.
    pub fn insert_text(&mut self, doc: &mut Document, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        self.delete_selection(doc);
        let pos = self.cursor;
        doc.insert(pos, text);
        self.set_cursor(doc, pos + text.chars().count(), false);
        true
    }

    /// Map a screen cell inside the editor text pane to a char index.
    ///
    /// `area` is the widget rect (gutter + text), matching [`crate::EditorWidget`].
    pub fn char_at_screen(
        &self,
        doc: &Document,
        area: ratatui::layout::Rect,
        column: u16,
        row: u16,
    ) -> Option<usize> {
        if area.width == 0 || area.height == 0 {
            return None;
        }
        if row < area.y || row >= area.y + area.height {
            return None;
        }
        if column < area.x || column >= area.x + area.width {
            return None;
        }
        let gutter = self.gutter_width(doc);
        let text_x = area.x.saturating_add(gutter);
        let rel_row = (row - area.y) as usize;
        let line = self.scroll_row + rel_row;
        let line_count = doc.len_lines().max(1);
        let line = line.min(line_count.saturating_sub(1));

        let col = if column <= text_x {
            self.scroll_col
        } else {
            self.scroll_col + (column - text_x) as usize
        };
        let col = col.min(doc.line_len(line));
        Some(doc.position_to_char(line, col))
    }

    /// Left-button press in the editor pane: move caret / start a selection.
    pub fn mouse_down(
        &mut self,
        doc: &Document,
        area: ratatui::layout::Rect,
        column: u16,
        row: u16,
    ) -> bool {
        let Some(pos) = self.char_at_screen(doc, area, column, row) else {
            return false;
        };
        self.set_cursor(doc, pos, false);
        self.mouse_selecting = true;
        true
    }

    /// Left-button drag: extend the selection.
    pub fn mouse_drag(
        &mut self,
        doc: &Document,
        area: ratatui::layout::Rect,
        column: u16,
        row: u16,
    ) -> bool {
        if !self.mouse_selecting {
            return false;
        }
        let Some(pos) = self.char_at_screen(doc, area, column, row) else {
            return false;
        };
        self.set_cursor(doc, pos, true);
        true
    }

    /// Left-button release.
    pub fn mouse_up(&mut self) {
        self.mouse_selecting = false;
    }

    pub fn is_mouse_selecting(&self) -> bool {
        self.mouse_selecting
    }

    /// Move the caret to `(line, col)` (0-based) and clear the selection.
    pub fn goto(&mut self, doc: &Document, line: usize, col: usize) {
        let pos = doc.position_to_char(line, col);
        self.set_cursor(doc, pos, false);
    }
}

/// Convenience: map common Ctrl chords that the host may want to intercept
/// before the editor (Save, Open, Quit, Run, Menu).
pub fn is_ctrl(ev: &KeyEvent, c: char) -> bool {
    ev.mods.ctrl
        && !ev.mods.alt
        && matches!(ev.key, Key::Char(ch) if ch.eq_ignore_ascii_case(&c))
}
