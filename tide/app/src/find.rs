//! Find / replace — Turbo Pascal edit-loop helpers.

use ratatui::style::{Color, Modifier, Style};
use tide_editor::{Decoration, Document, EditorView};

#[derive(Debug, Clone, Default)]
pub struct FindState {
    pub query: String,
    pub replace: String,
    pub case_sensitive: bool,
    /// Char ranges `[start, end)` of every match in document order.
    pub matches: Vec<(usize, usize)>,
    /// Index into `matches` of the current hit (`usize::MAX` = none).
    pub current: usize,
}

impl FindState {
    pub fn clear_matches(&mut self) {
        self.matches.clear();
        self.current = usize::MAX;
    }

    pub fn has_query(&self) -> bool {
        !self.query.is_empty()
    }

    pub fn current_range(&self) -> Option<(usize, usize)> {
        self.matches.get(self.current).copied()
    }
}

/// Refresh `matches` from the document + query. Keeps `current` if still valid.
pub fn refresh_matches(doc: &Document, find: &mut FindState) {
    if find.query.is_empty() {
        find.clear_matches();
        return;
    }
    find.matches = find_all(&doc.text(), &find.query, find.case_sensitive);
    if find.matches.is_empty() {
        find.current = usize::MAX;
    } else if find.current >= find.matches.len() {
        find.current = 0;
    }
}

fn find_all(text: &str, query: &str, case_sensitive: bool) -> Vec<(usize, usize)> {
    if query.is_empty() {
        return Vec::new();
    }
    let chars: Vec<char> = text.chars().collect();
    let q: Vec<char> = if case_sensitive {
        query.chars().collect()
    } else {
        query.chars().flat_map(|c| c.to_lowercase()).collect()
    };
    let q_len = query.chars().count();
    if q_len == 0 || q_len > chars.len() {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut i = 0;
    while i + q_len <= chars.len() {
        let matched = if case_sensitive {
            chars[i..i + q_len] == q[..]
        } else {
            let slice: Vec<char> = chars[i..i + q_len]
                .iter()
                .flat_map(|c| c.to_lowercase())
                .collect();
            slice == q
        };
        if matched {
            out.push((i, i + q_len));
            i += q_len.max(1); // non-overlapping
        } else {
            i += 1;
        }
    }
    out
}

/// Select the first match at or after `from_char` (wraps). Returns false if none.
pub fn find_next(doc: &Document, view: &mut EditorView, find: &mut FindState) -> bool {
    refresh_matches(doc, find);
    if find.matches.is_empty() {
        return false;
    }
    let from = view.cursor;
    let idx = find
        .matches
        .iter()
        .position(|&(s, _)| s >= from)
        .unwrap_or(0);
    select_match(doc, view, find, idx);
    true
}

/// Select the previous match before `from_char` (wraps).
pub fn find_prev(doc: &Document, view: &mut EditorView, find: &mut FindState) -> bool {
    refresh_matches(doc, find);
    if find.matches.is_empty() {
        return false;
    }
    let from = view.cursor;
    let idx = find
        .matches
        .iter()
        .rposition(|&(s, e)| e <= from && s < from)
        .unwrap_or(find.matches.len() - 1);
    select_match(doc, view, find, idx);
    true
}

fn select_match(doc: &Document, view: &mut EditorView, find: &mut FindState, idx: usize) {
    let (start, end) = find.matches[idx];
    find.current = idx;
    view.select_range(doc, start, end);
}

/// If the current selection is the current find hit, replace it; then find next.
/// If nothing selected as a hit, just find next (TP: first Enter finds).
pub fn replace_one(doc: &mut Document, view: &mut EditorView, find: &mut FindState) -> ReplaceResult {
    refresh_matches(doc, find);
    if find.query.is_empty() {
        return ReplaceResult::EmptyQuery;
    }
    if find.matches.is_empty() {
        return ReplaceResult::NotFound;
    }

    let at_hit = find.current_range().is_some_and(|(s, e)| {
        let (a, b) = view.selection.range();
        a == s && b == e
    });

    if at_hit {
        let (start, end) = find.current_range().unwrap();
        let repl = find.replace.clone();
        doc.delete_range(start, end);
        doc.insert(start, &repl);
        let new_cursor = start + repl.chars().count();
        view.select_range(doc, new_cursor, new_cursor);
        // Search again from after the replacement
        refresh_matches(doc, find);
        if find.matches.is_empty() {
            return ReplaceResult::ReplacedLast;
        }
        let idx = find
            .matches
            .iter()
            .position(|&(s, _)| s >= new_cursor)
            .unwrap_or(0);
        select_match(doc, view, find, idx);
        ReplaceResult::ReplacedAndFound
    } else {
        if find_next(doc, view, find) {
            ReplaceResult::Found
        } else {
            ReplaceResult::NotFound
        }
    }
}

pub fn replace_all(doc: &mut Document, view: &mut EditorView, find: &mut FindState) -> usize {
    refresh_matches(doc, find);
    if find.query.is_empty() || find.matches.is_empty() {
        return 0;
    }
    let repl = find.replace.clone();
    let query_len = find.query.chars().count();
    let repl_len = repl.chars().count();
    // Replace from end to start so offsets stay valid
    let mut count = 0;
    for &(start, end) in find.matches.iter().rev() {
        if end - start != query_len {
            continue;
        }
        doc.delete_range(start, end);
        doc.insert(start, &repl);
        count += 1;
        let _ = repl_len;
    }
    refresh_matches(doc, find);
    if let Some(&(s, e)) = find.matches.first() {
        view.select_range(doc, s, e);
        find.current = 0;
    } else {
        view.select_range(doc, view.cursor.min(doc.len_chars()), view.cursor.min(doc.len_chars()));
        find.current = usize::MAX;
    }
    count
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplaceResult {
    EmptyQuery,
    NotFound,
    Found,
    ReplacedAndFound,
    ReplacedLast,
}

pub fn match_decorations(doc: &Document, find: &FindState) -> Vec<Decoration> {
    let mut out = Vec::new();
    let highlight = Style::default()
        .bg(Color::DarkGray)
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let current = Style::default()
        .bg(Color::Yellow)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD);

    for (i, &(start, end)) in find.matches.iter().enumerate() {
        let style = if i == find.current {
            current
        } else {
            highlight
        };
        push_range_deco(doc, &mut out, start, end, style);
    }
    out
}

fn push_range_deco(
    doc: &Document,
    out: &mut Vec<Decoration>,
    start: usize,
    end: usize,
    style: Style,
) {
    if start >= end || doc.is_empty() {
        return;
    }
    let end = end.min(doc.len_chars());
    let start = start.min(end);
    let (line0, col0) = doc.char_to_position(start);
    let (line1, col1) = if end == 0 {
        (0, 0)
    } else {
        doc.char_to_position(end.saturating_sub(1))
    };
    // end is exclusive — column after last char of match on its line
    let (_, end_col_excl) = doc.char_to_position(end);

    if line0 == line1 {
        out.push(Decoration::new(line0, col0, end_col_excl.max(col0 + 1), style));
        return;
    }
    // Multi-line match: decorate each line piece
    out.push(Decoration::new(
        line0,
        col0,
        doc.line_len(line0),
        style,
    ));
    for line in (line0 + 1)..line1 {
        out.push(Decoration::new(line, 0, doc.line_len(line), style));
    }
    out.push(Decoration::new(line1, 0, end_col_excl.max(1), style));
    let _ = col1;
}
