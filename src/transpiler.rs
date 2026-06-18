//! AST in, idiomatic Rust source out.
//!
//! Two small but important touches even at this slice:
//!  * a mutability pre-scan, so a variable that is reassigned becomes `let mut`
//!    (Rust requires it; VB never made you think about it);
//!  * identifier renaming to snake_case, consistently at declaration and use.

use std::collections::HashSet;

use crate::ast::*;
use crate::diagnostics::Diagnostics;

pub fn transpile(program: &Program, diags: &mut Diagnostics) -> String {
    let mut out = String::new();
    for comment in &program.leading_comments {
        out.push_str(&format!("// {}\n", comment));
    }
    if !program.leading_comments.is_empty() {
        out.push('\n');
    }
    for (idx, func) in program.functions.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        emit_function(func, diags, &mut out);
    }
    out
}

fn emit_function(func: &Function, diags: &mut Diagnostics, out: &mut String) {
    let name = rust_fn_name(&func.name, func.line, diags);
    let ret = match func.ret {
        Some(ty) => format!(" -> {}", ty.rust()),
        None => String::new(),
    };
    out.push_str(&format!("fn {}(){} {{\n", name, ret));

    // Which locals get reassigned? Those need `let mut`.
    let mut mutated = HashSet::new();
    collect_mutated(&func.body, &mut mutated);

    emit_block(&func.body, &mutated, 1, diags, out);
    out.push_str("}\n");
}

fn emit_block(
    stmts: &[Stmt],
    mutated: &HashSet<String>,
    indent: usize,
    diags: &mut Diagnostics,
    out: &mut String,
) {
    for stmt in stmts {
        emit_stmt(stmt, mutated, indent, diags, out);
    }
}

fn emit_stmt(
    stmt: &Stmt,
    mutated: &HashSet<String>,
    indent: usize,
    diags: &mut Diagnostics,
    out: &mut String,
) {
    let pad = "    ".repeat(indent);
    match stmt {
        Stmt::Comment(text) => {
            out.push_str(&format!("{}// {}\n", pad, text));
        }
        Stmt::Dim {
            name,
            ty,
            init,
            line,
        } => {
            let var = to_snake(name);
            let is_mut = mutated.contains(&var);
            if !ty.is_fixed_size() {
                emit_dim_string(&var, name, is_mut, init.as_ref(), *line, diags, &pad, out);
            } else {
                let kw = let_kw(is_mut);
                match init {
                    Some(e) => {
                        let value = render_expr(e, Some(*ty));
                        out.push_str(&format!("{}{} {}: {} = {};\n", pad, kw, var, ty.rust(), value));
                    }
                    None => {
                        out.push_str(&format!("{}{} {}: {};\n", pad, kw, var, ty.rust()));
                    }
                }
            }
        }
        Stmt::Set {
            name,
            mutable,
            value,
        } => {
            diags.note(
                "set-borrow",
                "`Set` borrows instead of copying — the new name points at the same value, \
                 so no copy is made. `Set Mut` borrows mutably, letting you change the original.",
            );
            let var = to_snake(name);
            let borrow = if *mutable { "&mut " } else { "&" };
            out.push_str(&format!(
                "{}let {} = {}{};\n",
                pad,
                var,
                borrow,
                render_expr(value, None)
            ));
        }
        Stmt::Assign { name, value } => {
            let var = to_snake(name);
            out.push_str(&format!("{}{} = {};\n", pad, var, render_expr(value, None)));
        }
        Stmt::Print(e) => {
            out.push_str(&format!("{}println!(\"{{}}\", {});\n", pad, render_expr(e, None)));
        }
        Stmt::If {
            branches,
            else_body,
        } => {
            for (i, (cond, body)) in branches.iter().enumerate() {
                let head = if i == 0 { "if" } else { "} else if" };
                out.push_str(&format!("{}{} {} {{\n", pad, head, render_expr(cond, None)));
                emit_block(body, mutated, indent + 1, diags, out);
            }
            if let Some(body) = else_body {
                out.push_str(&format!("{}}} else {{\n", pad));
                emit_block(body, mutated, indent + 1, diags, out);
            }
            out.push_str(&format!("{}}}\n", pad));
        }
        Stmt::For {
            var,
            from,
            to,
            step,
            body,
        } => {
            let loop_var = to_snake(var);
            let range = render_range(from, to, step.as_ref(), diags);
            out.push_str(&format!("{}for {} in {} {{\n", pad, loop_var, range));
            emit_block(body, mutated, indent + 1, diags, out);
            out.push_str(&format!("{}}}\n", pad));
        }
    }
}

/// Emit a `Dim` of an unknown-size `String`, where ownership rules bite.
fn emit_dim_string(
    var: &str,
    orig_name: &str,
    is_mut: bool,
    init: Option<&Expr>,
    line: usize,
    diags: &mut Diagnostics,
    pad: &str,
    out: &mut String,
) {
    match init {
        None => {
            out.push_str(&format!("{}{} {}: String;\n", pad, let_kw(is_mut), var));
        }
        // A string literal is fixed size, so it can be a borrowed &str —
        // unless we later mutate it, in which case we need an owned String.
        Some(Expr::Str(s)) => {
            if is_mut {
                out.push_str(&format!(
                    "{}let mut {}: String = \"{}\".to_string();\n",
                    pad,
                    var,
                    escape(s)
                ));
            } else {
                out.push_str(&format!("{}let {}: &str = \"{}\";\n", pad, var, escape(s)));
            }
        }
        // Assigning one String variable to another would move/copy something of
        // unknown size. Rust won't do that silently — explain the explicit forms.
        Some(Expr::Ident(rhs)) => {
            diags.error(line, unknown_size_message(orig_name, rhs));
        }
        // Anything else (concat → format!, `.clone()`, …) is a freshly owned String.
        Some(other) => {
            if is_clone(other) {
                diags.note(
                    "clone-cost",
                    "`.clone()` makes a full copy. That's fine when you genuinely need a \
                     second copy, but it has a real cost — reach for `Set` to borrow when \
                     you only need to look at the value.",
                );
            }
            out.push_str(&format!(
                "{}{} {}: String = {};\n",
                pad,
                let_kw(is_mut),
                var,
                render_expr(other, None)
            ));
        }
    }
}

fn let_kw(is_mut: bool) -> &'static str {
    if is_mut {
        "let mut"
    } else {
        "let"
    }
}

fn is_clone(e: &Expr) -> bool {
    matches!(e, Expr::MethodCall { method, .. } if method == "clone")
}

/// The teaching block from spec_01, with the user's own variable names filled in.
fn unknown_size_message(target: &str, source: &str) -> String {
    format!(
        "Cannot assign '{src}' to '{dst}' directly.\n\n  \
         Unlike integers or doubles, String is not a fixed size —\n  \
         it can grow to any length. Rust won't silently copy something\n  \
         of unknown size. You need to be explicit:\n\n  \
         Set {dst} = {src}                    ' borrow — {dst} looks at {src}, no copy made\n  \
         Set Mut {dst} = {src}                ' mutable borrow — {dst} can modify {src}\n  \
         Dim {dst} As String = {src}.clone()  ' explicit copy — you are asking\n                                ' \
         for a copy knowing it has a cost\n\n  \
         The same rule applies to any type that can grow to an unknown\n  \
         size. Fixed size types like Long, Double and Boolean copy\n  \
         freely because Rust knows exactly how big they are.",
        src = source,
        dst = target
    )
}

/// Build the Rust range for a `For` loop, including `Step`.
fn render_range(from: &Expr, to: &Expr, step: Option<&Expr>, diags: &mut Diagnostics) -> String {
    let lo = render_expr(from, None);
    let hi = render_expr(to, None);
    match step {
        None => format!("{}..={}", lo, hi),
        Some(Expr::Int(n)) if *n < 0 => {
            // Counting down: Rust ranges only go up, so reverse.
            diags.note(
                "for-step-negative",
                "ℹ A counting-down `For ... Step -n` becomes `(lo..=hi).rev()` in Rust — \
                 Rust ranges always run low-to-high, so we reverse a normal one.",
            );
            let abs = (-n) as i64;
            if abs == 1 {
                format!("({}..={}).rev()", hi, lo)
            } else {
                format!("({}..={}).rev().step_by({})", hi, lo, abs)
            }
        }
        Some(Expr::Int(n)) => format!("({}..={}).step_by({})", lo, hi, n),
        Some(other) => {
            // Non-literal step: fall back to a literal-rendered step_by.
            format!("({}..={}).step_by({})", lo, hi, render_expr(other, None))
        }
    }
}

/// Render an expression. `expected` lets a `Double` context coerce integer
/// literals to floats (`5` -> `5.0`), which Rust requires.
fn render_expr(e: &Expr, expected: Option<Type>) -> String {
    render_prec(e, expected, 0, false)
}

/// Precedence-aware rendering: parens are emitted only where Rust needs them,
/// so the output reads like hand-written Rust (and rustc stays quiet).
/// `parent_prec` is the binding power of the enclosing operator; `is_right`
/// marks the right operand of a left-associative parent (so `a - (b - c)`
/// keeps its parens).
fn render_prec(e: &Expr, expected: Option<Type>, parent_prec: u8, is_right: bool) -> String {
    match e {
        Expr::Int(n) => {
            if expected == Some(Type::Double) {
                format!("{}.0", n)
            } else {
                n.to_string()
            }
        }
        Expr::Float(f) => fmt_float(*f),
        Expr::Bool(b) => b.to_string(),
        Expr::Str(s) => format!("\"{}\"", escape(s)),
        Expr::Ident(name) => to_snake(name),
        Expr::Binary { op, lhs, rhs } if *op == BinOp::Concat => {
            // `&` concatenation always becomes format!, sidestepping ownership.
            // The call is atomic, so it never needs surrounding parens.
            format!(
                "format!(\"{{}}{{}}\", {}, {})",
                render_prec(lhs, None, 0, false),
                render_prec(rhs, None, 0, false)
            )
        }
        Expr::Binary { op, lhs, rhs } => {
            let p = prec(*op);
            // Arithmetic propagates the Double context; comparisons don't.
            let child = if is_arithmetic(*op) { expected } else { None };
            let inner = format!(
                "{} {} {}",
                render_prec(lhs, child, p, false),
                op_str(*op),
                render_prec(rhs, child, p, true)
            );
            if p < parent_prec || (p == parent_prec && is_right) {
                format!("({})", inner)
            } else {
                inner
            }
        }
        Expr::MethodCall { recv, method, args } => {
            let rendered: Vec<String> = args.iter().map(|a| render_expr(a, None)).collect();
            // High parent precedence so a binary receiver gets parens: (a + b).abs()
            format!("{}.{}({})", render_prec(recv, None, 5, false), method, rendered.join(", "))
        }
    }
}

fn is_arithmetic(op: BinOp) -> bool {
    matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div)
}

/// Binding power — higher binds tighter.
fn prec(op: BinOp) -> u8 {
    match op {
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => 1,
        BinOp::Concat => 2,
        BinOp::Add | BinOp::Sub => 3,
        BinOp::Mul | BinOp::Div => 4,
    }
}

fn op_str(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::Le => "<=",
        BinOp::Ge => ">=",
        BinOp::Concat => "&", // handled separately
    }
}

fn fmt_float(f: f64) -> String {
    let s = f.to_string();
    if s.contains('.') || s.contains('e') || s.contains('E') {
        s
    } else {
        format!("{}.0", s)
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Names that are reassigned somewhere in the body need `let mut`.
fn collect_mutated(stmts: &[Stmt], set: &mut HashSet<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::Assign { name, .. } => {
                set.insert(to_snake(name));
            }
            // `Set Mut a = b` borrows b mutably, so b's own binding must be `mut`.
            Stmt::Set {
                mutable: true,
                value: Expr::Ident(n),
                ..
            } => {
                set.insert(to_snake(n));
            }
            Stmt::If {
                branches,
                else_body,
            } => {
                for (_, body) in branches {
                    collect_mutated(body, set);
                }
                if let Some(body) = else_body {
                    collect_mutated(body, set);
                }
            }
            Stmt::For { body, .. } => collect_mutated(body, set),
            _ => {}
        }
    }
}

fn rust_fn_name(name: &str, line: usize, diags: &mut Diagnostics) -> String {
    if name == "Main" {
        return "main".to_string();
    }
    let snake = to_snake(name);
    if snake != name {
        diags.warn(
            line,
            format!(
                "Function '{}' renamed to '{}' — Rust uses snake_case for functions.",
                name, snake
            ),
        );
    }
    snake
}

/// Convert PascalCase / camelCase to snake_case. Already-snake names pass through.
fn to_snake(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            let prev_lower = i > 0 && (chars[i - 1].is_lowercase() || chars[i - 1].is_ascii_digit());
            let next_lower = i + 1 < chars.len() && chars[i + 1].is_lowercase();
            if i > 0 && (prev_lower || next_lower) {
                out.push('_');
            }
            for lc in c.to_lowercase() {
                out.push(lc);
            }
        } else {
            out.push(c);
        }
    }
    out
}
