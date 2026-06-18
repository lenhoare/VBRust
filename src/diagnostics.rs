//! Educational diagnostics.
//!
//! Three levels, straight from spec_01:
//!   ✘ Error   — will not compile
//!   ⚠ Warning — compiles, but you should know
//!   ℹ Note    — a one-time teaching moment ("warn once, not repeatedly")

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

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub level: Level,
    pub message: String,
    pub line: Option<usize>,
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
        });
    }

    pub fn warn(&mut self, line: usize, message: impl Into<String>) {
        self.items.push(Diagnostic {
            level: Level::Warning,
            message: message.into(),
            line: Some(line),
        });
    }

    /// A warning shown only once per `key` — for caveats that would otherwise
    /// repeat on every occurrence (e.g. "Date carries no date semantics").
    pub fn warn_once(&mut self, key: &str, line: usize, message: impl Into<String>) {
        if self.seen_notes.insert(key.to_string()) {
            self.warn(line, message);
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
            });
        }
    }

    pub fn has_errors(&self) -> bool {
        self.items.iter().any(|d| d.level == Level::Error)
    }

    pub fn items(&self) -> &[Diagnostic] {
        &self.items
    }
}
