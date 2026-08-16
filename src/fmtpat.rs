//! Parse a Rust `format!` pattern with **one** placeholder.
//!
//! `Format(x, "{:.2}")` is a thin wrapper around `format!("{:.2}", x)`. VB's
//! `"#,###.00"` strings have no `{` and stay a hard error.

/// One `{…}` placeholder, with the literal text around it (`{{` / `}}` already
/// unescaped in prefix/suffix).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatPat {
    pub prefix: String,
    /// The placeholder including braces (`{:.2}`, `{}`, `{:04}`).
    pub inner: String,
    pub suffix: String,
}

impl FormatPat {
    pub fn parse(pat: &str) -> Option<Self> {
        let mut prefix = String::new();
        let mut chars = pat.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '{' => {
                    if chars.peek() == Some(&'{') {
                        chars.next();
                        prefix.push('{');
                        continue;
                    }
                    let mut inner = String::from("{");
                    let mut closed = false;
                    for c2 in chars.by_ref() {
                        inner.push(c2);
                        if c2 == '}' {
                            closed = true;
                            break;
                        }
                    }
                    if !closed {
                        return None;
                    }
                    let mut suffix = String::new();
                    while let Some(c2) = chars.next() {
                        match c2 {
                            '{' => {
                                if chars.peek() == Some(&'{') {
                                    chars.next();
                                    suffix.push('{');
                                } else {
                                    return None; // a second placeholder
                                }
                            }
                            '}' => {
                                if chars.peek() == Some(&'}') {
                                    chars.next();
                                    suffix.push('}');
                                } else {
                                    return None;
                                }
                            }
                            _ => suffix.push(c2),
                        }
                    }
                    return Some(FormatPat { prefix, inner, suffix });
                }
                '}' => {
                    if chars.peek() == Some(&'}') {
                        chars.next();
                        prefix.push('}');
                    } else {
                        return None;
                    }
                }
                _ => prefix.push(c),
            }
        }
        None
    }

    pub fn is_bare(&self) -> bool {
        self.inner == "{}"
    }

    /// Rebuild with `{:.2}` → `{:.2f}` so Python's `.format` accepts a float.
    pub fn python_pattern(&self) -> String {
        format!("{}{}{}", self.prefix, python_placeholder(&self.inner), self.suffix)
    }

    /// A `printf` spec for this placeholder, or `None` to stringify via Display.
    pub fn printf_spec(&self, float: bool) -> Option<String> {
        printf_spec(&self.inner, float)
    }
}

fn python_placeholder(inner: &str) -> String {
    if inner == "{}" {
        return inner.to_string();
    }
    let Some(spec) = inner.strip_prefix("{:").and_then(|s| s.strip_suffix('}')) else {
        return inner.to_string();
    };
    if spec.contains('.') {
        if let Some(last) = spec.chars().last() {
            if last.is_ascii_digit() {
                return format!("{{:{spec}f}}");
            }
        }
    }
    inner.to_string()
}

fn printf_spec(inner: &str, float: bool) -> Option<String> {
    if inner == "{}" {
        return None;
    }
    let spec = inner.strip_prefix("{:").and_then(|s| s.strip_suffix('}'))?;
    if float {
        if let Some(prec) = spec.strip_prefix('.') {
            if !prec.is_empty() && prec.chars().all(|c| c.is_ascii_digit()) {
                return Some(format!("%.{prec}f"));
            }
        }
        if let Some((w, p)) = spec.split_once('.') {
            if w.chars().all(|c| c.is_ascii_digit())
                && !p.is_empty()
                && p.chars().all(|c| c.is_ascii_digit())
            {
                return Some(format!("%{w}.{p}f"));
            }
        }
        return None;
    }
    if let Some(w) = spec.strip_prefix('0') {
        if !w.is_empty() && w.chars().all(|c| c.is_ascii_digit()) {
            return Some(format!("%0{w}lld"));
        }
    }
    if !spec.is_empty() && spec.chars().all(|c| c.is_ascii_digit()) {
        return Some(format!("%{spec}lld"));
    }
    if let Some(prec) = spec.strip_prefix('.') {
        if !prec.is_empty() && prec.chars().all(|c| c.is_ascii_digit()) {
            return Some(format!("%.{prec}lld"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_and_prefixed() {
        let p = FormatPat::parse("{:.2}").unwrap();
        assert_eq!(p.inner, "{:.2}");
        assert!(p.prefix.is_empty() && p.suffix.is_empty());
        let p = FormatPat::parse("approx {:.1}!").unwrap();
        assert_eq!(p.prefix, "approx ");
        assert_eq!(p.inner, "{:.1}");
        assert_eq!(p.suffix, "!");
    }

    #[test]
    fn rejects_vb_and_two_slots() {
        assert!(FormatPat::parse("#,###.00").is_none());
        assert!(FormatPat::parse("{:.2} {}").is_none());
    }
}
