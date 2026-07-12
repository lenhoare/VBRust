//! Tokeniser for VBR source.
//!
//! VBA keywords are case-insensitive (you can write `Dim`, `dim`, or `DIM`),
//! so keywords are matched on a lowercased copy while identifier spelling is
//! preserved. Newlines are significant — in VB a statement ends at end of line.

use crate::span::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    // Literals
    Int(i64),
    Float(f64),
    Str(String),
    /// A backtick-quoted column name (`` `Unit Price` ``) — sugar for `Col("…")`
    /// in a dataframe column formula.
    Backtick(String),
    Ident(String),

    // Keywords
    Function,
    Sub,
    Return,
    ByVal,
    ByRef,
    End,
    Dim,
    Set,
    Mut,
    As,
    If,
    Then,
    ElseIf,
    Else,
    Select,
    Case,
    Match,
    Await,
    For,
    Each,
    In,
    To,
    Step,
    Next,
    New,
    Do,
    Loop,
    While,
    Until,
    Exit,
    Continue,
    With,
    And,
    Or,
    Not,
    Xor,
    Mod,
    True,
    False,
    TyInteger,
    TyLong,
    TyLongLong,
    TySingle,
    TyDouble,
    TyBoolean,
    TyByte,
    TyString,
    TyCurrency,
    TyVariant,

    // Operators & punctuation
    Plus,
    Minus,
    Star,
    Slash,
    Caret, // ^
    Amp,   // &
    PlusEq,  // +=
    MinusEq, // -=
    StarEq,  // *=
    SlashEq, // /=
    Eq,  // = (assignment or equality, parser decides)
    Ne,  // <>
    Lt,
    Gt,
    Le,
    Ge,
    LParen,
    RParen,
    LBrace,   // {
    RBrace,   // }
    LBracket, // [
    RBracket, // ]
    Comma,
    Dot,
    Colon,    // :
    Question, // ?
    Pipe,     // |
    FatArrow, // => (match arm)
    DotDot,   // ..  (range pattern, exclusive)
    DotDotEq, // ..= (range pattern, inclusive)

    Type,
    Enum,
    Public,
    Private,
    Const,
    ReDim,

    On, // start of `On Error ...`

    /// A `Rust … End Rust` block, captured verbatim (the inner Rust is not tokenised).
    InlineRust(String),

    /// A `Css … End Css` block, captured verbatim — a `Page`'s stylesheet
    /// (real CSS, injected into the generated `index.html`).
    InlineCss(String),

    /// A `Python … End Python` block, captured verbatim (the inner Python is not
    /// tokenised — it is run at runtime via pyo3, not spliced like inline Rust).
    /// `args` is the raw text inside optional leading parens (`Python(df, n)` →
    /// `"df, n"`), the VBR variables passed into the block; `body` is the source.
    InlinePython { args: String, body: String },

    /// A `Text … End Text` block — a multi-line string literal, captured
    /// verbatim (nothing inside is escaped, ever). Triggered only when `Text`
    /// ends its line, so the `Text` view widget (`Text "hi"`) and member
    /// access (`row.Text`) are untouched. `terminated` is false when the
    /// closing `End Text` is missing (the parser turns that into an error).
    TextBlock { body: String, terminated: bool },

    /// A `Use <crate> <version>` line, captured raw (the parser splits it).
    Use(String),

    Comment(String),
    Newline,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub tok: Tok,
    pub line: usize,
    /// The token's byte range in the source (`[start, end)`), for
    /// column-precise diagnostics and cursor→token lookup.
    pub span: Span,
}

pub fn lex(src: &str) -> Vec<Token> {
    let chars: Vec<char> = src.chars().collect();
    // The lexer scans by char index, but spans are byte offsets (what slices
    // a `&str`) — this table converts. One extra entry so `chars.len()` maps
    // to the end of the source.
    let byte_of: Vec<usize> = {
        let mut v = Vec::with_capacity(chars.len() + 1);
        let mut b = 0usize;
        for &c in &chars {
            v.push(b);
            b += c.len_utf8();
        }
        v.push(b);
        v
    };
    let sp = |s: usize, e: usize| Span::new(byte_of[s], byte_of[e]);
    let mut tokens = Vec::new();
    let mut i = 0usize;
    let mut line = 1usize;

    while i < chars.len() {
        let c = chars[i];
        match c {
            '\n' => {
                tokens.push(Token { tok: Tok::Newline, line, span: sp(i, i + 1) });
                line += 1;
                i += 1;
            }
            ' ' | '\t' | '\r' => i += 1,
            '\'' => {
                // Comment runs to end of line.
                let start = i + 1;
                let mut j = start;
                while j < chars.len() && chars[j] != '\n' {
                    j += 1;
                }
                let text: String = chars[start..j].iter().collect();
                tokens.push(Token {
                    tok: Tok::Comment(text.trim().to_string()),
                    line,
                    span: sp(i, j),
                });
                i = j;
            }
            '"' => {
                let mut j = i + 1;
                let mut s = String::new();
                loop {
                    match chars.get(j) {
                        None => break, // unterminated — best effort
                        Some('"') => {
                            // VB doubles a quote to embed one: "" → "
                            if chars.get(j + 1) == Some(&'"') {
                                s.push('"');
                                j += 2;
                            } else {
                                j += 1; // closing quote
                                break;
                            }
                        }
                        Some(c) => {
                            s.push(*c);
                            j += 1;
                        }
                    }
                }
                tokens.push(Token { tok: Tok::Str(s), line, span: sp(i, j) });
                i = j;
            }
            '`' => {
                // A backtick-quoted column name (`Unit Price`) for dataframe
                // formulas — captured verbatim until the closing backtick.
                let mut j = i + 1;
                let mut s = String::new();
                while let Some(c) = chars.get(j) {
                    if *c == '`' {
                        j += 1;
                        break;
                    }
                    s.push(*c);
                    j += 1;
                }
                tokens.push(Token { tok: Tok::Backtick(s), line, span: sp(i, j) });
                i = j;
            }
            '+' if chars.get(i + 1) == Some(&'=') => two(&mut tokens, Tok::PlusEq, line, sp(i, i + 2), &mut i),
            '-' if chars.get(i + 1) == Some(&'=') => two(&mut tokens, Tok::MinusEq, line, sp(i, i + 2), &mut i),
            '*' if chars.get(i + 1) == Some(&'=') => two(&mut tokens, Tok::StarEq, line, sp(i, i + 2), &mut i),
            '/' if chars.get(i + 1) == Some(&'=') => two(&mut tokens, Tok::SlashEq, line, sp(i, i + 2), &mut i),
            '+' => push(&mut tokens, Tok::Plus, line, sp(i, i + 1), &mut i),
            '-' => push(&mut tokens, Tok::Minus, line, sp(i, i + 1), &mut i),
            '*' => push(&mut tokens, Tok::Star, line, sp(i, i + 1), &mut i),
            '/' => push(&mut tokens, Tok::Slash, line, sp(i, i + 1), &mut i),
            '^' => push(&mut tokens, Tok::Caret, line, sp(i, i + 1), &mut i),
            '&' => push(&mut tokens, Tok::Amp, line, sp(i, i + 1), &mut i),
            '=' if chars.get(i + 1) == Some(&'>') => two(&mut tokens, Tok::FatArrow, line, sp(i, i + 2), &mut i),
            '=' => push(&mut tokens, Tok::Eq, line, sp(i, i + 1), &mut i),
            '(' => push(&mut tokens, Tok::LParen, line, sp(i, i + 1), &mut i),
            ')' => push(&mut tokens, Tok::RParen, line, sp(i, i + 1), &mut i),
            '{' => push(&mut tokens, Tok::LBrace, line, sp(i, i + 1), &mut i),
            '}' => push(&mut tokens, Tok::RBrace, line, sp(i, i + 1), &mut i),
            '[' => push(&mut tokens, Tok::LBracket, line, sp(i, i + 1), &mut i),
            ']' => push(&mut tokens, Tok::RBracket, line, sp(i, i + 1), &mut i),
            ',' => push(&mut tokens, Tok::Comma, line, sp(i, i + 1), &mut i),
            // `..=` and `..` for Rust range patterns (`1..=10`). Plain `.` stays
            // member access / float point.
            '.' if chars.get(i + 1) == Some(&'.') => {
                if chars.get(i + 2) == Some(&'=') {
                    tokens.push(Token { tok: Tok::DotDotEq, line, span: sp(i, i + 3) });
                    i += 3;
                } else {
                    two(&mut tokens, Tok::DotDot, line, sp(i, i + 2), &mut i);
                }
            }
            '.' => push(&mut tokens, Tok::Dot, line, sp(i, i + 1), &mut i),
            ':' => push(&mut tokens, Tok::Colon, line, sp(i, i + 1), &mut i),
            '?' => push(&mut tokens, Tok::Question, line, sp(i, i + 1), &mut i),
            '|' => push(&mut tokens, Tok::Pipe, line, sp(i, i + 1), &mut i),
            '<' => {
                if chars.get(i + 1) == Some(&'>') {
                    tokens.push(Token { tok: Tok::Ne, line, span: sp(i, i + 2) });
                    i += 2;
                } else if chars.get(i + 1) == Some(&'=') {
                    tokens.push(Token { tok: Tok::Le, line, span: sp(i, i + 2) });
                    i += 2;
                } else {
                    push(&mut tokens, Tok::Lt, line, sp(i, i + 1), &mut i);
                }
            }
            '>' => {
                if chars.get(i + 1) == Some(&'=') {
                    tokens.push(Token { tok: Tok::Ge, line, span: sp(i, i + 2) });
                    i += 2;
                } else {
                    push(&mut tokens, Tok::Gt, line, sp(i, i + 1), &mut i);
                }
            }
            c if c.is_ascii_digit() => {
                let start = i;
                let mut j = i;
                while j < chars.len() && chars[j].is_ascii_digit() {
                    j += 1;
                }
                let mut is_float = false;
                // A '.' followed by a digit makes it a float (so `x.0` style
                // member access on a literal isn't swallowed).
                if j < chars.len() && chars[j] == '.' && chars.get(j + 1).map_or(false, |d| d.is_ascii_digit()) {
                    is_float = true;
                    j += 1;
                    while j < chars.len() && chars[j].is_ascii_digit() {
                        j += 1;
                    }
                }
                let text: String = chars[start..j].iter().collect();
                let tok = if is_float {
                    Tok::Float(text.parse().unwrap_or(0.0))
                } else {
                    Tok::Int(text.parse().unwrap_or(0))
                };
                tokens.push(Token { tok, line, span: sp(start, j) });
                i = j;
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                let mut j = i;
                while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                    j += 1;
                }
                let word: String = chars[start..j].iter().collect();
                if word.eq_ignore_ascii_case("Rust") {
                    // Inline Rust: capture the body verbatim (do NOT tokenise it)
                    // until a line that reads `End Rust`.
                    let (raw, resume, newlines, _) = capture_inline_block(&chars, j, "rust");
                    tokens.push(Token {
                        tok: Tok::InlineRust(raw),
                        line,
                        span: sp(start, resume),
                    });
                    line += newlines;
                    i = resume;
                } else if word.eq_ignore_ascii_case("Css") {
                    // A Page's stylesheet: capture verbatim until `End Css` —
                    // it's real CSS, not VBR.
                    let (raw, resume, newlines, _) = capture_inline_block(&chars, j, "css");
                    tokens.push(Token {
                        tok: Tok::InlineCss(raw),
                        line,
                        span: sp(start, resume),
                    });
                    line += newlines;
                    i = resume;
                } else if word.eq_ignore_ascii_case("Python") {
                    // Inline Python: optional `(args)` (VBR vars passed in), then the
                    // verbatim body terminated by `End Python`.
                    let (args, after_args) = capture_call_args(&chars, j);
                    let (body, resume, newlines, _) =
                        capture_inline_block(&chars, after_args, "python");
                    tokens.push(Token {
                        tok: Tok::InlinePython { args, body },
                        line,
                        span: sp(start, resume),
                    });
                    line += newlines;
                    i = resume;
                } else if word.eq_ignore_ascii_case("Text")
                    && !matches!(tokens.last().map(|t| &t.tok), Some(Tok::Dot))
                    && rest_of_line_is_blank(&chars, j)
                    && text_block_content_follows(&chars, start, j)
                {
                    // A multi-line string literal: `Text` alone at the end of a
                    // line opens a verbatim block, closed by `End Text`. (With
                    // an argument on the same line it's the view widget, and
                    // after a dot it's a member — both fall through below.)
                    // The content-follows guard keeps a *variable* named `text`
                    // at end of line (`"say: " & text`) from opening a block —
                    // VB is case-insensitive, so `text` and `Text` are one word,
                    // and only a block indents its next line under the opener.
                    let (raw, resume, newlines, terminated) =
                        capture_inline_block(&chars, j, "text");
                    tokens.push(Token {
                        tok: Tok::TextBlock { body: raw, terminated },
                        line,
                        span: sp(start, resume),
                    });
                    line += newlines;
                    i = resume;
                } else if word.eq_ignore_ascii_case("Use") {
                    // `Use rand 0.8` — capture the rest of the line raw, so a
                    // version like `0.8.5` doesn't fight the tokeniser.
                    let (rest, resume) = capture_to_eol(&chars, j);
                    tokens.push(Token {
                        tok: Tok::Use(rest),
                        line,
                        span: sp(start, resume),
                    });
                    i = resume;
                } else {
                    // A word right after `.` is a member name (`df.Select`,
                    // `x.Next`), never a keyword — so it can share a spelling with
                    // one. Otherwise, classify it normally.
                    let after_dot = matches!(tokens.last().map(|t| &t.tok), Some(Tok::Dot));
                    let tok = if after_dot {
                        Tok::Ident(word.clone())
                    } else {
                        keyword_or_ident(&word)
                    };
                    tokens.push(Token { tok, line, span: sp(start, j) });
                    i = j;
                }
            }
            _ => {
                // Unknown character — skip it. The parser will notice the gap.
                i += 1;
            }
        }
    }

    tokens.push(Token { tok: Tok::Eof, line, span: sp(chars.len(), chars.len()) });
    tokens
}

/// Capture the rest of the current line (trimmed), returning it and the index of
/// the line's newline (or EOF) so the main lexer resumes there.
fn capture_to_eol(chars: &[char], start: usize) -> (String, usize) {
    let mut e = start;
    while e < chars.len() && chars[e] != '\n' {
        e += 1;
    }
    let rest: String = chars[start..e].iter().collect();
    (rest.trim().to_string(), e)
}

/// Capture an optional `(args)` group immediately after `Python` (on the same
/// line — a following newline means the body starts, not an argument list).
/// Returns (raw inner text, index to resume the body scan at). No parens → empty
/// args and the original index.
fn capture_call_args(chars: &[char], start: usize) -> (String, usize) {
    let mut p = start;
    // Skip spaces/tabs but not newlines (a newline ends the "same line" window).
    while p < chars.len() && (chars[p] == ' ' || chars[p] == '\t') {
        p += 1;
    }
    if chars.get(p) != Some(&'(') {
        return (String::new(), start);
    }
    let inner_start = p + 1;
    let mut depth = 1;
    let mut e = inner_start;
    while e < chars.len() && depth > 0 {
        match chars[e] {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            break;
        }
        e += 1;
    }
    let inner: String = chars[inner_start..e].iter().collect();
    // Resume just past the closing `)`.
    (inner.trim().to_string(), e + 1)
}

/// Capture an inline block (`Rust`/`Python`/`Text`) verbatim. `start` is just
/// after the opening keyword; the block runs until an `End <term>` terminator,
/// which may sit on its own line (multi-line block) or trail the body on one
/// line (`Rust … End Rust`). Returns (raw body, index to resume at, newlines
/// consumed, whether the terminator was found).
fn capture_inline_block(chars: &[char], start: usize, term: &str) -> (String, usize, usize, bool) {
    let mut i = start;
    while i < chars.len() {
        if let Some(resume) = match_end_block(chars, i, term) {
            // Trim whitespace/newlines between the body and `End`.
            let mut body_end = i;
            while body_end > start && chars[body_end - 1].is_whitespace() {
                body_end -= 1;
            }
            let raw: String = chars[start..body_end].iter().collect();
            let newlines = chars[start..resume].iter().filter(|&&c| c == '\n').count();
            return (raw, resume, newlines, true);
        }
        i += 1;
    }
    // Unterminated — capture to end of file.
    let raw: String = chars[start..].iter().collect();
    let newlines = chars[start..].iter().filter(|&&c| c == '\n').count();
    (raw, chars.len(), newlines, false)
}

/// Is the rest of the current line (from `start`) only spaces/tabs? Decides
/// whether a bare `Text` opens a multi-line string block.
fn rest_of_line_is_blank(chars: &[char], start: usize) -> bool {
    let mut p = start;
    while p < chars.len() && matches!(chars[p], ' ' | '\t' | '\r') {
        p += 1;
    }
    p >= chars.len() || chars[p] == '\n'
}

/// Does a `Text` at end of line open a block, or is it a variable named `text`?
/// A block always indents its content under the opener; a bare identifier is
/// followed by the next statement at the same or lesser indent. `opener` points
/// at the `T`; `after` is just past the word. We compare the opener's column to
/// the first following non-blank line's leading indentation.
fn text_block_content_follows(chars: &[char], opener: usize, after: usize) -> bool {
    // The opener line's own leading indentation (content is compared against
    // this, not the column of the `Text` word — the word may sit far to the
    // right after `Dim body As String = `, while the content indents under the
    // statement).
    let mut ls = opener;
    while ls > 0 && chars[ls - 1] != '\n' {
        ls -= 1;
    }
    let mut opener_col = 0;
    while ls + opener_col < chars.len() && matches!(chars[ls + opener_col], ' ' | '\t') {
        opener_col += 1;
    }
    // Advance to the start of the next line.
    let mut p = after;
    while p < chars.len() && chars[p] != '\n' {
        p += 1;
    }
    // Skip blank lines, then measure the first non-blank line's indent.
    while p < chars.len() {
        p += 1; // step over the '\n'
        let mut q = p;
        while q < chars.len() && matches!(chars[q], ' ' | '\t' | '\r') {
            q += 1;
        }
        if q >= chars.len() || chars[q] == '\n' {
            continue; // blank line — keep looking
        }
        return q - p > opener_col;
    }
    false
}

/// If `chars[i..]` begins a whole-word `End` <ws> `<term>` terminator (at a word
/// boundary), return the index just past `<term>`; otherwise `None`.
fn match_end_block(chars: &[char], i: usize, term: &str) -> Option<usize> {
    // `End` must start at a word boundary.
    if i > 0 && !chars[i - 1].is_whitespace() {
        return None;
    }
    let word = |p: usize, w: &str| -> Option<usize> {
        let w: Vec<char> = w.chars().collect();
        if p + w.len() > chars.len() {
            return None;
        }
        for (k, c) in w.iter().enumerate() {
            if !chars[p + k].eq_ignore_ascii_case(c) {
                return None;
            }
        }
        Some(p + w.len())
    };
    let after_end = word(i, "end")?;
    // At least one whitespace between `End` and the terminator keyword.
    if !chars.get(after_end).is_some_and(|c| c.is_whitespace()) {
        return None;
    }
    let mut p = after_end;
    while p < chars.len() && chars[p].is_whitespace() {
        p += 1;
    }
    let after_term = word(p, term)?;
    // The terminator must end at a word boundary (whitespace or EOF).
    match chars.get(after_term) {
        None => Some(after_term),
        Some(c) if c.is_whitespace() => Some(after_term),
        _ => None,
    }
}

fn push(tokens: &mut Vec<Token>, tok: Tok, line: usize, span: Span, i: &mut usize) {
    tokens.push(Token { tok, line, span });
    *i += 1;
}

/// Push a two-character operator token and advance past both characters.
fn two(tokens: &mut Vec<Token>, tok: Tok, line: usize, span: Span, i: &mut usize) {
    tokens.push(Token { tok, line, span });
    *i += 2;
}

fn keyword_or_ident(word: &str) -> Tok {
    match word.to_ascii_lowercase().as_str() {
        "function" => Tok::Function,
        "sub" => Tok::Sub,
        "return" => Tok::Return,
        "byval" => Tok::ByVal,
        "byref" => Tok::ByRef,
        "on" => Tok::On,
        "type" => Tok::Type,
        "enum" => Tok::Enum,
        "public" => Tok::Public,
        "private" => Tok::Private,
        "const" => Tok::Const,
        "redim" => Tok::ReDim,
        "end" => Tok::End,
        "dim" => Tok::Dim,
        "set" => Tok::Set,
        "mut" => Tok::Mut,
        "as" => Tok::As,
        "if" => Tok::If,
        "then" => Tok::Then,
        "elseif" => Tok::ElseIf,
        "else" => Tok::Else,
        "select" => Tok::Select,
        "case" => Tok::Case,
        "match" => Tok::Match,
        "await" => Tok::Await,
        "for" => Tok::For,
        "each" => Tok::Each,
        "in" => Tok::In,
        "to" => Tok::To,
        "step" => Tok::Step,
        "next" => Tok::Next,
        "new" => Tok::New,
        "do" => Tok::Do,
        "loop" => Tok::Loop,
        "while" => Tok::While,
        "until" => Tok::Until,
        "exit" => Tok::Exit,
        "continue" => Tok::Continue,
        "with" => Tok::With,
        "and" => Tok::And,
        "or" => Tok::Or,
        "not" => Tok::Not,
        "xor" => Tok::Xor,
        "mod" => Tok::Mod,
        "true" => Tok::True,
        "false" => Tok::False,
        "integer" => Tok::TyInteger,
        "long" => Tok::TyLong,
        "longlong" => Tok::TyLongLong,
        "single" => Tok::TySingle,
        "double" => Tok::TyDouble,
        "boolean" => Tok::TyBoolean,
        "byte" => Tok::TyByte,
        "string" => Tok::TyString,
        "currency" => Tok::TyCurrency,
        "variant" => Tok::TyVariant,
        _ => Tok::Ident(word.to_string()),
    }
}
