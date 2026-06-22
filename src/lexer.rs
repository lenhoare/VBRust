//! Tokeniser for VBR source.
//!
//! VBA keywords are case-insensitive (you can write `Dim`, `dim`, or `DIM`),
//! so keywords are matched on a lowercased copy while identifier spelling is
//! preserved. Newlines are significant — in VB a statement ends at end of line.

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    // Literals
    Int(i64),
    Float(f64),
    Str(String),
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
    True,
    False,
    TyInteger,
    TyLong,
    TyLongLong,
    TySingle,
    TyDouble,
    TyBoolean,
    TyByte,
    TyDate,
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

    Type,
    Public,
    Private,
    Const,
    ReDim,

    On, // start of `On Error ...`

    /// A `Rust … End Rust` block, captured verbatim (the inner Rust is not tokenised).
    InlineRust(String),

    Comment(String),
    Newline,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub tok: Tok,
    pub line: usize,
}

pub fn lex(src: &str) -> Vec<Token> {
    let chars: Vec<char> = src.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0usize;
    let mut line = 1usize;

    while i < chars.len() {
        let c = chars[i];
        match c {
            '\n' => {
                tokens.push(Token { tok: Tok::Newline, line });
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
                tokens.push(Token { tok: Tok::Str(s), line });
                i = j;
            }
            '+' => push(&mut tokens, Tok::Plus, line, &mut i),
            '-' => push(&mut tokens, Tok::Minus, line, &mut i),
            '*' => push(&mut tokens, Tok::Star, line, &mut i),
            '/' => push(&mut tokens, Tok::Slash, line, &mut i),
            '^' => push(&mut tokens, Tok::Caret, line, &mut i),
            '&' => push(&mut tokens, Tok::Amp, line, &mut i),
            '=' => push(&mut tokens, Tok::Eq, line, &mut i),
            '(' => push(&mut tokens, Tok::LParen, line, &mut i),
            ')' => push(&mut tokens, Tok::RParen, line, &mut i),
            '{' => push(&mut tokens, Tok::LBrace, line, &mut i),
            '}' => push(&mut tokens, Tok::RBrace, line, &mut i),
            '[' => push(&mut tokens, Tok::LBracket, line, &mut i),
            ']' => push(&mut tokens, Tok::RBracket, line, &mut i),
            ',' => push(&mut tokens, Tok::Comma, line, &mut i),
            '.' => push(&mut tokens, Tok::Dot, line, &mut i),
            ':' => push(&mut tokens, Tok::Colon, line, &mut i),
            '?' => push(&mut tokens, Tok::Question, line, &mut i),
            '|' => push(&mut tokens, Tok::Pipe, line, &mut i),
            '<' => {
                if chars.get(i + 1) == Some(&'>') {
                    tokens.push(Token { tok: Tok::Ne, line });
                    i += 2;
                } else if chars.get(i + 1) == Some(&'=') {
                    tokens.push(Token { tok: Tok::Le, line });
                    i += 2;
                } else {
                    push(&mut tokens, Tok::Lt, line, &mut i);
                }
            }
            '>' => {
                if chars.get(i + 1) == Some(&'=') {
                    tokens.push(Token { tok: Tok::Ge, line });
                    i += 2;
                } else {
                    push(&mut tokens, Tok::Gt, line, &mut i);
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
                tokens.push(Token { tok, line });
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
                    let (raw, resume, newlines) = capture_inline_rust(&chars, j);
                    tokens.push(Token {
                        tok: Tok::InlineRust(raw),
                        line,
                    });
                    line += newlines;
                    i = resume;
                } else {
                    tokens.push(Token {
                        tok: keyword_or_ident(&word),
                        line,
                    });
                    i = j;
                }
            }
            _ => {
                // Unknown character — skip it. The parser will notice the gap.
                i += 1;
            }
        }
    }

    tokens.push(Token { tok: Tok::Eof, line });
    tokens
}

/// Capture an inline Rust block verbatim. `start` is just after the `Rust`
/// keyword; the block runs until a line whose content is `End Rust`.
/// Returns (raw body, index to resume at, number of newlines consumed).
fn capture_inline_rust(chars: &[char], start: usize) -> (String, usize, usize) {
    let mut line_start = start;
    let mut newlines = 0;
    loop {
        // The current line is [line_start, le).
        let mut le = line_start;
        while le < chars.len() && chars[le] != '\n' {
            le += 1;
        }
        let line_str: String = chars[line_start..le].iter().collect();
        let words: Vec<&str> = line_str.split_whitespace().collect();
        let is_end = words.len() == 2
            && words[0].eq_ignore_ascii_case("end")
            && words[1].eq_ignore_ascii_case("rust");
        if is_end {
            // Body is everything up to the newline before this line; resume at
            // the newline after `End Rust` so the normal lexer emits a Newline.
            let body_end = line_start.saturating_sub(1).max(start);
            let raw: String = chars[start..body_end].iter().collect();
            return (raw, le, newlines);
        }
        if le >= chars.len() {
            // Unterminated — capture to end of file.
            let raw: String = chars[start..].iter().collect();
            return (raw, chars.len(), newlines);
        }
        newlines += 1;
        line_start = le + 1;
    }
}

fn push(tokens: &mut Vec<Token>, tok: Tok, line: usize, i: &mut usize) {
    tokens.push(Token { tok, line });
    *i += 1;
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
        "true" => Tok::True,
        "false" => Tok::False,
        "integer" => Tok::TyInteger,
        "long" => Tok::TyLong,
        "longlong" => Tok::TyLongLong,
        "single" => Tok::TySingle,
        "double" => Tok::TyDouble,
        "boolean" => Tok::TyBoolean,
        "byte" => Tok::TyByte,
        "date" => Tok::TyDate,
        "string" => Tok::TyString,
        "currency" => Tok::TyCurrency,
        "variant" => Tok::TyVariant,
        _ => Tok::Ident(word.to_string()),
    }
}
