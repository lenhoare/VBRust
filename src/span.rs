//! Source positions.
//!
//! A `Span` is a half-open **byte** range `[start, end)` into the original
//! `.vbr` source text. Tokens carry one, and (via the parser) so do
//! expressions — which is what lets a diagnostic underline the exact word
//! rather than the whole line, and lets the language server answer "what is
//! under the cursor?". Byte offsets (not char or line/column) because that's
//! what slices a Rust `&str` directly; `LineIndex` converts to line/column
//! when a tool needs those.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Span {
        Span { start, end }
    }

    /// The smallest span covering both — how a parent node's span is built
    /// from its children's (`a + b` spans from the start of `a` to the end
    /// of `b`).
    pub fn to(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Does this span cover the byte at `offset`? (Cursor→node lookup.)
    pub fn contains(self, offset: usize) -> bool {
        self.start <= offset && offset < self.end
    }

    /// A span that points nowhere — for nodes synthesized with no single
    /// source home. `Default` gives the same; this reads better at call sites.
    pub fn none() -> Span {
        Span::default()
    }
}

/// Converts a byte offset into a (line, column) pair, both **0-based** with
/// the column in bytes — the shape editors and the LSP work in. Built once
/// per source text; lookups are a binary search.
pub struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(text: &str) -> LineIndex {
        let mut line_starts = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        LineIndex { line_starts }
    }

    /// The (0-based line, 0-based byte column) of a byte offset. Offsets past
    /// the end land on the last line.
    pub fn position(&self, offset: usize) -> (usize, usize) {
        let line = self.line_starts.partition_point(|&s| s <= offset) - 1;
        (line, offset - self.line_starts[line])
    }
}
