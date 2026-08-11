//! Syntax highlighting hooks — language-specific logic stays in the host app.

use crate::style::SpanStyle;

/// Highlight one line of source into styled spans.
///
/// Spans should be non-overlapping and ordered by `start`. Gaps are painted
/// with the editor's default style.
pub trait Highlighter {
    fn highlight(&self, line: &str) -> Vec<SpanStyle>;
}

/// No highlighting — plain text.
#[derive(Debug, Default, Clone, Copy)]
pub struct PlainHighlighter;

impl Highlighter for PlainHighlighter {
    fn highlight(&self, _line: &str) -> Vec<SpanStyle> {
        Vec::new()
    }
}

/// Simple case-insensitive keyword highlighter.
///
/// Good enough for MVP IDEs; hosts supply their own keyword list.
#[derive(Debug, Clone)]
pub struct KeywordHighlighter {
    keywords: Vec<String>,
    style: ratatui::style::Style,
    comment_prefix: Option<String>,
    string_style: Option<ratatui::style::Style>,
    comment_style: Option<ratatui::style::Style>,
}

impl KeywordHighlighter {
    pub fn new(keywords: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            keywords: keywords.into_iter().map(Into::into).collect(),
            style: crate::style::palette::keyword(),
            comment_prefix: None,
            string_style: Some(crate::style::palette::string()),
            comment_style: Some(crate::style::palette::comment()),
        }
    }

    pub fn with_style(mut self, style: ratatui::style::Style) -> Self {
        self.style = style;
        self
    }

    pub fn with_line_comment(mut self, prefix: impl Into<String>) -> Self {
        self.comment_prefix = Some(prefix.into());
        self
    }
}

impl Highlighter for KeywordHighlighter {
    fn highlight(&self, line: &str) -> Vec<SpanStyle> {
        let mut spans = Vec::new();
        let chars: Vec<char> = line.chars().collect();
        let len = chars.len();
        let mut i = 0;

        while i < len {
            // Line comment to EOL
            if let Some(prefix) = &self.comment_prefix {
                let pref: Vec<char> = prefix.chars().collect();
                if i + pref.len() <= len && chars[i..i + pref.len()] == pref[..] {
                    if let Some(style) = self.comment_style {
                        spans.push(SpanStyle::new(i, len, style));
                    }
                    break;
                }
            }

            // String literal "..."
            if chars[i] == '"' {
                let start = i;
                i += 1;
                while i < len {
                    if chars[i] == '"' {
                        // VBA-style doubled quote
                        if i + 1 < len && chars[i + 1] == '"' {
                            i += 2;
                            continue;
                        }
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                if let Some(style) = self.string_style {
                    spans.push(SpanStyle::new(start, i, style));
                }
                continue;
            }

            // Identifier / keyword
            if chars[i].is_ascii_alphabetic() || chars[i] == '_' {
                let start = i;
                i += 1;
                while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                if self
                    .keywords
                    .iter()
                    .any(|k| k.eq_ignore_ascii_case(&word))
                {
                    spans.push(SpanStyle::new(start, i, self.style));
                }
                continue;
            }

            // Number
            if chars[i].is_ascii_digit() {
                let start = i;
                i += 1;
                while i < len && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                spans.push(SpanStyle::new(start, i, crate::style::palette::number()));
                continue;
            }

            i += 1;
        }

        spans
    }
}
