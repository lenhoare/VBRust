//! Educational diagnostics.
//!
//! Three levels, straight from spec_01:
//!   ✘ Error   — will not compile
//!   ⚠ Warning — compiles, but you should know
//!   ℹ Note    — a one-time teaching moment ("warn once, not repeatedly")

use crate::ast::DeclType;
use crate::span::Span;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Error,
    Warning,
    Note,
}

impl Level {
    fn symbol(self) -> char {
        match self {
            Level::Error => '✘',
            Level::Warning => '⚠',
            Level::Note => 'ℹ',
        }
    }
}

/// A teaching *caution* — a "this works, but prefer…" note shown both as a
/// compile-time diagnostic AND as a callout in the help docs. Holding the text
/// in one place is the whole point: the note you see while coding and the note
/// in the reference can never drift apart. The docs render `summary` as the
/// callout heading and `body` as its text; the compiler emits `body`.
pub struct Caution {
    pub key: &'static str,
    pub summary: &'static str,
    pub body: &'static str,
}

pub const CAUTIONS: &[Caution] = &[
    Caution {
        key: "val-lenient",
        summary: "Lenient — never fails",
        body: "`Val` returns 0 for text it can't read as a number, so it never fails and never \
               tells you the input was bad. Handy for tidy input, but not a validator: when a bad \
               value should be caught, use `CDbl` / `CLng` / `CInt`. Those can fail; the error \
               propagates unless you intercept the call with `Handle err`.",
    },
    Caution {
        key: "unwrap-training-wheels",
        summary: "`.Unwrap()` is gone",
        body: "`.Unwrap()` is gone. Errors propagate automatically. To intercept a call, use \
               `Handle err` … `End Handle`. For an Option, use `If x Is Some(v) Then`, `Match`, \
               or `.Unwrap_Or(default)`.",
    },
];

/// The caution registered under `key`, if any.
pub fn caution(key: &str) -> Option<&'static Caution> {
    CAUTIONS.iter().find(|c| c.key == key)
}

/// One identifier occurrence the resolver understood: where it is, what it's
/// called, its declared type (`None` for constants and opaque Rust handles),
/// and the display line hover shows. Hover reads `display`; completion reads
/// `ty` to answer "what are the members of the thing before this dot?".
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub span: Span,
    pub name: String,
    pub ty: Option<DeclType>,
    pub display: String,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub level: Level,
    pub message: String,
    pub line: Option<usize>,
    /// The exact byte range in the source, when the reporter knows it — lets
    /// an editor underline the offending word instead of the whole line.
    pub span: Option<Span>,
}

impl Diagnostic {
    pub fn render(&self) -> String {
        match self.line {
            Some(l) => format!("{} [line {}] {}", self.level.symbol(), l, self.message),
            None => format!("{} {}", self.level.symbol(), self.message),
        }
    }
}

/// Collects diagnostics across the whole run and enforces "show a note once".
#[derive(Debug, Default)]
pub struct Diagnostics {
    items: Vec<Diagnostic>,
    seen_notes: HashSet<String>,
    marks: HashSet<String>,
    /// (generated-Rust line, Bust source line) checkpoints, in emission order —
    /// the map `vbr run`/`runproject` use to point rustc errors back at the
    /// `.vbr` source. Lives here because the whole emission pipeline already
    /// threads `Diagnostics` through.
    line_map: Vec<(usize, usize)>,
    /// What the resolver learned about each identifier occurrence — hover
    /// display, and the declared type completion needs to answer `x.`.
    /// Rides here for the same reason as `line_map`.
    symbols: Vec<SymbolInfo>,
    /// (use span, declaration span) pairs — go-to-definition jumps along these.
    defs: Vec<(Span, Span)>,
}

impl Diagnostics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn error(&mut self, line: usize, message: impl Into<String>) {
        self.items.push(Diagnostic {
            level: Level::Error,
            message: message.into(),
            line: Some(line),
            span: None,
        });
    }

    /// An error whose exact source range is known — editors underline just
    /// that range. `line` still comes along for the CLI rendering.
    pub fn error_at(&mut self, span: Span, line: usize, message: impl Into<String>) {
        self.items.push(Diagnostic {
            level: Level::Error,
            message: message.into(),
            line: Some(line),
            span: Some(span),
        });
    }

    pub fn warn(&mut self, line: usize, message: impl Into<String>) {
        self.items.push(Diagnostic {
            level: Level::Warning,
            message: message.into(),
            line: Some(line),
            span: None,
        });
    }

    /// A warning with an exact source range (see `error_at`).
    pub fn warn_at(&mut self, span: Span, line: usize, message: impl Into<String>) {
        self.items.push(Diagnostic {
            level: Level::Warning,
            message: message.into(),
            line: Some(line),
            span: Some(span),
        });
    }

    /// A warning shown only once per `key` — for caveats that would otherwise
    /// repeat on every occurrence (e.g. "Date carries no date semantics").
    pub fn warn_once(&mut self, key: &str, line: usize, message: impl Into<String>) {
        if self.seen_notes.insert(key.to_string()) {
            self.warn(line, message);
        }
    }

    /// A line-less hard error shown only once per `key` — for unsupported
    /// builtins that may appear many times (e.g. `Rnd()`).
    pub fn error_once(&mut self, key: &str, message: impl Into<String>) {
        if self.seen_notes.insert(key.to_string()) {
            self.items.push(Diagnostic {
                level: Level::Error,
                message: message.into(),
                line: None,
                span: None,
            });
        }
    }

    /// A line-less warning shown only once per `key` — for general caveats not
    /// tied to a specific source position (e.g. "Mid is 1-indexed").
    pub fn warn_once_global(&mut self, key: &str, message: impl Into<String>) {
        if self.seen_notes.insert(key.to_string()) {
            self.items.push(Diagnostic {
                level: Level::Warning,
                message: message.into(),
                line: None,
                span: None,
            });
        }
    }

    /// A teaching note shown only once per `key`, no matter how often it fires.
    pub fn note(&mut self, key: &str, message: impl Into<String>) {
        if self.seen_notes.insert(key.to_string()) {
            self.items.push(Diagnostic {
                level: Level::Note,
                message: message.into(),
                line: None,
                span: None,
            });
        }
    }

    /// Emit a registered [`Caution`]'s text as a one-time note.
    pub fn caution_note(&mut self, key: &str) {
        if let Some(c) = caution(key) {
            self.note(key, c.body);
        }
    }

    /// Emit a registered [`Caution`]'s text as a one-time warning.
    pub fn caution_warn(&mut self, key: &str) {
        if let Some(c) = caution(key) {
            self.warn_once_global(key, c.body);
        }
    }

    pub fn has_errors(&self) -> bool {
        self.items.iter().any(|d| d.level == Level::Error)
    }

    /// A silent flag (not shown) the transpiler can leave for itself, e.g. to
    /// note that a runtime helper needs emitting.
    pub fn mark(&mut self, key: &str) {
        self.marks.insert(key.to_string());
    }

    pub fn has_mark(&self, key: &str) -> bool {
        self.marks.contains(key)
    }

    pub fn items(&self) -> &[Diagnostic] {
        &self.items
    }

    /// Record that generated-Rust line `rust_line` came from Bust line `vbr_line`.
    pub fn map_line(&mut self, rust_line: usize, vbr_line: usize) {
        self.line_map.push((rust_line, vbr_line));
    }

    /// Take the finished line map (leaves an empty one behind).
    pub fn take_line_map(&mut self) -> Vec<(usize, usize)> {
        std::mem::take(&mut self.line_map)
    }

    /// Drop the map — for emitters (GUI/TUI) that assemble output out of
    /// order, where the checkpoints would mislead rather than help.
    pub fn clear_line_map(&mut self) {
        self.line_map.clear();
    }

    /// Record an identifier occurrence the resolver understood.
    pub fn symbol(&mut self, info: SymbolInfo) {
        self.symbols.push(info);
    }

    /// Take the collected symbol entries (leaves an empty list behind).
    pub fn take_symbols(&mut self) -> Vec<SymbolInfo> {
        std::mem::take(&mut self.symbols)
    }

    /// Record that the name at `use_span` was declared at `decl_span`.
    pub fn def(&mut self, use_span: Span, decl_span: Span) {
        self.defs.push((use_span, decl_span));
    }

    /// Take the collected go-to-definition pairs.
    pub fn take_defs(&mut self) -> Vec<(Span, Span)> {
        std::mem::take(&mut self.defs)
    }
}
