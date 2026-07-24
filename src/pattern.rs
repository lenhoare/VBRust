//! Parse a match-arm pattern — the parser captures it as a space-joined,
//! Rust-shaped token string (`Shape :: Circle ( r )`, `90 ..= 99`, `0 | 1 | 2`)
//! — into a structured [`Pat`] the non-Rust backends can lower without
//! re-tokenising. Shared by the typing pass and the C backend (a candidate to
//! move into the IR once it's lifted; the Python backend still parses these
//! strings inline).

/// A structured match pattern (slice-3 grammar: literals, ranges, alternation,
/// enum tags and data-variant destructures, plus a binding and the wildcard).
#[derive(Debug, Clone)]
pub enum Pat {
    /// `_`
    Wildcard,
    /// `x` — binds the whole scrutinee.
    Binding(String),
    /// `100` (or `- 5`).
    Int(i64),
    /// `90 ..= 99` (inclusive).
    Range(i64, i64),
    /// `0 | 1 | 2` — matches if any alternative does.
    Alt(Vec<Pat>),
    /// `Suit :: Hearts` — a payload-free variant / C-like enum value.
    EnumTag { enom: String, variant: String },
    /// `Shape :: Circle ( r )` — a data variant, binding its payload fields.
    Variant { enom: String, variant: String, binds: Vec<String> },
    /// Anything this slice doesn't model — kept verbatim.
    Other(String),
}

/// Parse the space-joined pattern string.
pub fn parse(src: &str) -> Pat {
    let toks: Vec<&str> = src.split_whitespace().collect();
    parse_toks(&toks)
}

fn parse_toks(toks: &[&str]) -> Pat {
    if toks.is_empty() {
        return Pat::Other(String::new());
    }
    // Top-level alternation splits first.
    if toks.contains(&"|") {
        let parts = split_top(toks, "|");
        return Pat::Alt(parts.iter().map(|p| parse_toks(p)).collect());
    }
    if toks == ["_"] {
        return Pat::Wildcard;
    }
    // A range `a ..= b`.
    if let Some(pos) = toks.iter().position(|t| *t == "..=") {
        if let (Some(a), Some(b)) = (parse_int(&toks[..pos]), parse_int(&toks[pos + 1..])) {
            return Pat::Range(a, b);
        }
    }
    // An enum path `Enum :: Variant`, optionally with `( a , b )` bindings.
    if let Some(pos) = toks.iter().position(|t| *t == "::") {
        let enom = toks[..pos].join("");
        if let Some(variant) = toks.get(pos + 1) {
            let variant = variant.to_string();
            if toks.get(pos + 2) == Some(&"(") {
                let binds: Vec<String> = toks[pos + 3..]
                    .iter()
                    .take_while(|t| **t != ")")
                    .filter(|t| **t != ",")
                    .map(|t| t.to_string())
                    .collect();
                return Pat::Variant { enom, variant, binds };
            }
            return Pat::EnumTag { enom, variant };
        }
    }
    if let Some(n) = parse_int(toks) {
        return Pat::Int(n);
    }
    // A single lowercase-ish identifier binds the scrutinee.
    if toks.len() == 1 && is_ident(toks[0]) {
        return Pat::Binding(toks[0].to_string());
    }
    Pat::Other(toks.join(" "))
}

fn parse_int(toks: &[&str]) -> Option<i64> {
    match toks {
        [n] => n.parse().ok(),
        ["-", n] => n.parse::<i64>().ok().map(|v| -v),
        _ => None,
    }
}

fn is_ident(t: &str) -> bool {
    let mut chars = t.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Split a token slice on `sep` at paren depth 0.
fn split_top<'a>(toks: &[&'a str], sep: &str) -> Vec<Vec<&'a str>> {
    let mut groups = Vec::new();
    let mut cur = Vec::new();
    let mut depth = 0i32;
    for t in toks {
        match *t {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            _ => {}
        }
        if *t == sep && depth == 0 {
            groups.push(std::mem::take(&mut cur));
        } else {
            cur.push(*t);
        }
    }
    groups.push(cur);
    groups
}
