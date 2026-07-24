//! Shared iterator-chain analysis — parse a method-call chain
//! (`xs.filter(|v| c).map(|v| e).collect()`) into a structured pipeline: a base
//! receiver, a sequence of adapter [`Step`]s, and a [`Terminal`] consumer. Both
//! the Python backend (→ comprehensions) and the C backend (→ explicit loops)
//! detect the *shape* here, once, then lower it their own way. A new adapter is
//! added in one place and both backends can grow to support it.

use crate::ast::*;

/// An adapter step, in source order (base → terminal).
pub enum Step<'a> {
    Filter { var: &'a str, cond: &'a Expr },
    Map { var: &'a str, body: &'a Expr },
    Take(&'a Expr),
    Skip(&'a Expr),
    Rev,
}

/// The consuming operation at the end of a pipeline.
pub enum Terminal<'a> {
    Collect,
    Sum,
    Count,
    Any { var: &'a str, cond: &'a Expr },
    All { var: &'a str, cond: &'a Expr },
    Find { var: &'a str, cond: &'a Expr },
    Position { var: &'a str, cond: &'a Expr },
    Max,
    Min,
}

pub struct Chain<'a> {
    pub base: &'a Expr,
    pub steps: Vec<Step<'a>>,
    pub terminal: Terminal<'a>,
}

/// Parse `e` as a whole pipeline (its outermost call must be a terminal).
pub fn parse(e: &Expr) -> Option<Chain<'_>> {
    if let ExprKind::MethodCall { recv, method, args } = &e.kind {
        let terminal = terminal(method, args)?;
        let (base, steps) = split_adapters(recv);
        return Some(Chain { base, steps, terminal });
    }
    None
}

/// Classify a terminal consumer `method(args)`, or `None` if it isn't one.
pub fn terminal<'a>(method: &str, args: &'a [Expr]) -> Option<Terminal<'a>> {
    match method.to_ascii_lowercase().as_str() {
        "collect" => Some(Terminal::Collect),
        "sum" => Some(Terminal::Sum),
        "count" | "len" => Some(Terminal::Count),
        "max" if args.is_empty() => Some(Terminal::Max),
        "min" if args.is_empty() => Some(Terminal::Min),
        "any" => closure(args).map(|(var, cond)| Terminal::Any { var, cond }),
        "all" => closure(args).map(|(var, cond)| Terminal::All { var, cond }),
        "find" => closure(args).map(|(var, cond)| Terminal::Find { var, cond }),
        "position" => closure(args).map(|(var, cond)| Terminal::Position { var, cond }),
        _ => None,
    }
}

/// Peel the adapter chain off `recv`, returning the base receiver and the steps
/// in source order (base-first).
pub fn split_adapters(recv: &Expr) -> (&Expr, Vec<Step<'_>>) {
    let mut steps = Vec::new();
    let mut cur = recv;
    while let ExprKind::MethodCall { recv: r, method: m, args: a } = &cur.kind {
        match step(m, a) {
            Some(s) => {
                steps.push(s);
                cur = r;
            }
            None => break,
        }
    }
    steps.reverse();
    (cur, steps)
}

fn step<'a>(method: &str, args: &'a [Expr]) -> Option<Step<'a>> {
    match method.to_ascii_lowercase().as_str() {
        "filter" => closure(args).map(|(var, cond)| Step::Filter { var, cond }),
        "map" => closure(args).map(|(var, body)| Step::Map { var, body }),
        "take" if args.len() == 1 => Some(Step::Take(&args[0])),
        "skip" if args.len() == 1 => Some(Step::Skip(&args[0])),
        "rev" if args.is_empty() => Some(Step::Rev),
        _ => None,
    }
}

/// A single-parameter closure argument → its `(var, body)`.
fn closure(args: &[Expr]) -> Option<(&str, &Expr)> {
    if let [Expr { kind: ExprKind::Closure { params, body, .. }, .. }] = args {
        if params.len() == 1 {
            return Some((params[0].as_str(), body));
        }
    }
    None
}
