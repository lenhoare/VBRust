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
    For,
    To,
    Step,
    Next,
    True,
    False,
    TyLong,
    TyInteger,
    TyDouble,
    TyBoolean,
    TyString,

    // Operators & punctuation
    Plus,
    Minus,
    Star,
    Slash,
    Amp, // &
    Eq,  // = (assignment or equality, parser decides)
    Ne,  // <>
    Lt,
    Gt,
    Le,
    Ge,
    LParen,
    RParen,
    Comma,
    Dot,

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
                while j < chars.len() && chars[j] != '"' {
                    s.push(chars[j]);
                    j += 1;
                }
                j += 1; // consume closing quote (if present)
                tokens.push(Token { tok: Tok::Str(s), line });
                i = j;
            }
            '+' => push(&mut tokens, Tok::Plus, line, &mut i),
            '-' => push(&mut tokens, Tok::Minus, line, &mut i),
            '*' => push(&mut tokens, Tok::Star, line, &mut i),
            '/' => push(&mut tokens, Tok::Slash, line, &mut i),
            '&' => push(&mut tokens, Tok::Amp, line, &mut i),
            '=' => push(&mut tokens, Tok::Eq, line, &mut i),
            '(' => push(&mut tokens, Tok::LParen, line, &mut i),
            ')' => push(&mut tokens, Tok::RParen, line, &mut i),
            ',' => push(&mut tokens, Tok::Comma, line, &mut i),
            '.' => push(&mut tokens, Tok::Dot, line, &mut i),
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
                tokens.push(Token {
                    tok: keyword_or_ident(&word),
                    line,
                });
                i = j;
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
        "end" => Tok::End,
        "dim" => Tok::Dim,
        "set" => Tok::Set,
        "mut" => Tok::Mut,
        "as" => Tok::As,
        "if" => Tok::If,
        "then" => Tok::Then,
        "elseif" => Tok::ElseIf,
        "else" => Tok::Else,
        "for" => Tok::For,
        "to" => Tok::To,
        "step" => Tok::Step,
        "next" => Tok::Next,
        "true" => Tok::True,
        "false" => Tok::False,
        "long" => Tok::TyLong,
        "integer" => Tok::TyInteger,
        "double" => Tok::TyDouble,
        "boolean" => Tok::TyBoolean,
        "string" => Tok::TyString,
        _ => Tok::Ident(word.to_string()),
    }
}
