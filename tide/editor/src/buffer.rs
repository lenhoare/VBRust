//! Rope-backed document with undo/redo and dirty tracking.

use ropey::Rope;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
struct Edit {
    /// Byte offset where the edit started.
    start: usize,
    /// Text that was removed (for undo).
    removed: String,
    /// Text that was inserted (for redo).
    inserted: String,
}

/// Owned text buffer. Coordinates are `(line, column)` in chars (0-based).
#[derive(Debug, Clone)]
pub struct Document {
    rope: Rope,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    dirty: bool,
    path: Option<PathBuf>,
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Document {
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            dirty: false,
            path: None,
        }
    }

    pub fn from_str(text: &str) -> Self {
        let mut doc = Self::new();
        doc.rope = Rope::from_str(&normalize_line_endings(text));
        doc
    }

    pub fn open(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)?;
        let mut doc = Self::from_str(&text);
        doc.path = Some(path.to_path_buf());
        doc.dirty = false;
        Ok(doc)
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        let path = self
            .path
            .clone()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "no path"))?;
        self.save_as(path)
    }

    pub fn save_as(&mut self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let path = path.as_ref();
        std::fs::write(path, self.text())?;
        self.path = Some(path.to_path_buf());
        self.dirty = false;
        Ok(())
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn set_path(&mut self, path: Option<PathBuf>) {
        self.path = path;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn is_empty(&self) -> bool {
        self.rope.len_chars() == 0
    }

    /// Line text without the trailing `\n` (last line may have none).
    pub fn line(&self, line: usize) -> String {
        if line >= self.len_lines() {
            return String::new();
        }
        let slice = self.rope.line(line);
        let mut s = slice.to_string();
        if s.ends_with('\n') {
            s.pop();
            if s.ends_with('\r') {
                s.pop();
            }
        }
        s
    }

    pub fn line_len(&self, line: usize) -> usize {
        self.line(line).chars().count()
    }

    pub fn char_to_line(&self, char_idx: usize) -> usize {
        if self.len_chars() == 0 {
            return 0;
        }
        let idx = char_idx.min(self.len_chars().saturating_sub(1));
        self.rope.char_to_line(idx)
    }

    pub fn line_to_char(&self, line: usize) -> usize {
        if self.len_lines() == 0 {
            return 0;
        }
        let line = line.min(self.len_lines() - 1);
        self.rope.line_to_char(line)
    }

    /// Convert `(line, col)` to a char index, clamping to the line.
    pub fn position_to_char(&self, line: usize, col: usize) -> usize {
        if self.is_empty() {
            return 0;
        }
        let line = line.min(self.len_lines().saturating_sub(1));
        let start = self.rope.line_to_char(line);
        let max_col = self.line_len(line);
        start + col.min(max_col)
    }

    pub fn char_to_position(&self, char_idx: usize) -> (usize, usize) {
        if self.is_empty() {
            return (0, 0);
        }
        let char_idx = char_idx.min(self.len_chars());
        // At end-of-document: sit after last char on last line (may be the
        // empty line ropey adds after a trailing `\n`).
        if char_idx == self.len_chars() {
            let line = self.len_lines().saturating_sub(1);
            return (line, self.line_len(line));
        }
        let line = self.rope.char_to_line(char_idx);
        let start = self.rope.line_to_char(line);
        let col = char_idx - start;
        // If the index points at the line's trailing `\n`, report the column
        // as end-of-content (same visual cell as "past last character").
        let content_len = self.line_len(line);
        (line, col.min(content_len))
    }

    pub fn insert(&mut self, char_idx: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        // Normalize line endings on the way in — a paste of `\r\n` (Windows) or
        // bare `\r` otherwise becomes one ropey "line" and painting `\r` into the
        // terminal resets the cursor mid-row (gutters/text look corrupted).
        let text = normalize_line_endings(text);
        if text.is_empty() {
            return;
        }
        let char_idx = char_idx.min(self.len_chars());
        let start_byte = self.rope.char_to_byte(char_idx);
        self.rope.insert(char_idx, &text);
        self.push_edit(Edit {
            start: start_byte,
            removed: String::new(),
            inserted: text,
        });
    }

    pub fn delete_range(&mut self, start_char: usize, end_char: usize) {
        let start = start_char.min(end_char).min(self.len_chars());
        let end = start_char.max(end_char).min(self.len_chars());
        if start == end {
            return;
        }
        let removed = self.rope.slice(start..end).to_string();
        let start_byte = self.rope.char_to_byte(start);
        self.rope.remove(start..end);
        self.push_edit(Edit {
            start: start_byte,
            removed,
            inserted: String::new(),
        });
    }

    /// Returns the new cursor char index after undo, if anything changed.
    pub fn undo(&mut self) -> Option<usize> {
        let edit = self.undo.pop()?;
        let start_char = self.byte_to_char_clamped(edit.start);
        if !edit.inserted.is_empty() {
            let end = start_char + edit.inserted.chars().count();
            let end = end.min(self.len_chars());
            self.rope.remove(start_char..end);
        }
        if !edit.removed.is_empty() {
            self.rope.insert(start_char, &edit.removed);
        }
        self.redo.push(edit.clone());
        self.dirty = true;
        let cursor = start_char + edit.removed.chars().count();
        Some(cursor.min(self.len_chars()))
    }

    /// Returns the new cursor char index after redo, if anything changed.
    pub fn redo(&mut self) -> Option<usize> {
        let edit = self.redo.pop()?;
        let start_char = self.byte_to_char_clamped(edit.start);
        if !edit.removed.is_empty() {
            let end = start_char + edit.removed.chars().count();
            let end = end.min(self.len_chars());
            self.rope.remove(start_char..end);
        }
        if !edit.inserted.is_empty() {
            self.rope.insert(start_char, &edit.inserted);
        }
        self.undo.push(edit.clone());
        self.dirty = true;
        let cursor = start_char + edit.inserted.chars().count();
        Some(cursor.min(self.len_chars()))
    }

    fn push_edit(&mut self, edit: Edit) {
        self.undo.push(edit);
        self.redo.clear();
        self.dirty = true;
    }

    fn byte_to_char_clamped(&self, byte: usize) -> usize {
        let byte = byte.min(self.rope.len_bytes());
        self.rope.byte_to_char(byte)
    }
}

fn normalize_line_endings(text: &str) -> String {
    if !text.as_bytes().contains(&b'\r') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_undo() {
        let mut doc = Document::new();
        doc.insert(0, "hi");
        assert_eq!(doc.text(), "hi");
        assert!(doc.is_dirty());
        doc.undo();
        assert_eq!(doc.text(), "");
    }

    #[test]
    fn multiline_positions() {
        let doc = Document::from_str("ab\ncd");
        assert_eq!(doc.line(0), "ab");
        assert_eq!(doc.line(1), "cd");
        assert_eq!(doc.position_to_char(1, 1), 4);
        assert_eq!(doc.char_to_position(4), (1, 1));
    }

    #[test]
    fn eof_after_trailing_newline() {
        let doc = Document::from_str("a\nb\n");
        assert_eq!(doc.len_lines(), 3);
        assert_eq!(doc.char_to_position(doc.len_chars()), (2, 0));
        assert_eq!(doc.char_to_position(1), (0, 1));
    }

    #[test]
    fn crlf_paste_becomes_real_lines() {
        use crate::input::{Key, KeyEvent, KeyMods};
        use crate::view::EditorView;

        let mut doc = Document::from_str("keep\n");
        let mut view = EditorView::new();
        view.ensure_visible(&doc, 10, 80);
        view.goto(&doc, 1, 0);
        let block = (0..30)
            .map(|i| format!("line{i}\r\n"))
            .collect::<String>();
        view.insert_text(&mut doc, &block);
        assert!(
            doc.len_lines() > 20,
            "CRLF paste must split into many lines, got {}",
            doc.len_lines()
        );
        assert!(!doc.text().contains('\r'));

        let (line_before, _) = view.cursor_position(&doc);
        view.handle_key(&mut doc, &KeyEvent::new(Key::Up, KeyMods::none()));
        let (line_after, _) = view.cursor_position(&doc);
        assert_eq!(
            line_after,
            line_before - 1,
            "↑ must leave the last line after a CRLF paste"
        );
    }

    #[test]
    fn scroll_clamps_at_last_page() {
        use crate::input::{Key, KeyEvent, KeyMods};
        use crate::view::EditorView;

        let mut text = String::new();
        for i in 0..40 {
            text.push_str(&format!("line{i}\n"));
        }
        let mut doc = Document::from_str(&text);
        let mut view = EditorView::new();
        view.ensure_visible(&doc, 10, 80);
        assert_eq!(EditorView::max_scroll_row(&doc, 10), doc.len_lines() - 10);

        view.goto(&doc, doc.len_lines() - 1, 0);
        view.ensure_visible(&doc, 10, 80);
        assert!(view.scroll_row <= EditorView::max_scroll_row(&doc, 10));
        assert_eq!(
            view.scroll_row,
            EditorView::max_scroll_row(&doc, 10),
            "cursor on last line should pin scroll to last page"
        );

        view.scroll_by(&doc, 100);
        assert_eq!(view.scroll_row, EditorView::max_scroll_row(&doc, 10));

        let before = view.cursor_position(&doc).0;
        view.handle_key(
            &mut doc,
            &KeyEvent::new(Key::Up, KeyMods::none()),
        );
        let after = view.cursor_position(&doc).0;
        assert_eq!(after, before - 1);
        assert!(view.scroll_row <= after || after >= view.scroll_row);
    }

    #[test]
    fn paste_scrolls_caret_into_view() {
        use crate::view::EditorView;

        let mut doc = Document::from_str("start\n");
        let mut view = EditorView::new();
        view.ensure_visible(&doc, 5, 80);
        let block = (0..20).map(|i| format!("p{i}\n")).collect::<String>();
        assert!(view.insert_text(&mut doc, &block));
        let (line, _) = view.cursor_position(&doc);
        assert!(
            line >= view.scroll_row && line < view.scroll_row + 5,
            "caret line {line} should be visible in scroll {}..{}",
            view.scroll_row,
            view.scroll_row + 5
        );
        assert!(view.scroll_row <= EditorView::max_scroll_row(&doc, 5));
    }
}
