//! AST in, idiomatic Rust source out.
//!
//! Two small but important touches even at this slice:
//!  * a mutability pre-scan, so a variable that is reassigned becomes `let mut`
//!    (Rust requires it; VB never made you think about it);
//!  * identifier renaming to snake_case, consistently at declaration and use.

use std::collections::HashSet;

use crate::ast::*;
use crate::diagnostics::Diagnostics;
use crate::resolver::{self, FnTable};

pub fn transpile(program: &Program, diags: &mut Diagnostics) -> String {
    // Fire the one-time teaching notes for builtins before generating code,
    // keeping the rendering functions pure.
    for func in &program.functions {
        note_builtins(&func.body, diags);
    }

    let fns = resolver::build_fn_table(program);

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
        emit_function(func, &fns, diags, &mut out);
    }
    out
}

fn emit_function(func: &Function, fns: &FnTable, diags: &mut Diagnostics, out: &mut String) {
    let name = rust_fn_name(&func.name, func.line, diags);
    let params: Vec<String> = func.params.iter().map(render_param).collect();
    let ret = match func.ret {
        Some(RetType::Plain(t)) => format!(" -> {}", t.rust()),
        Some(RetType::Result(t)) => format!(" -> Result<{}, String>", t.rust()),
        Some(RetType::Option(t)) => format!(" -> Option<{}>", t.rust()),
        None => String::new(),
    };
    // Only a plain return type drives literal coercion of the tail expression;
    // an Ok/Some wrapper carries its own type.
    let tail_expected = match func.ret {
        Some(RetType::Plain(t)) => Some(t),
        _ => None,
    };
    out.push_str(&format!("fn {}({}){} {{\n", name, params.join(", "), ret));

    // `FunctionName = value` is really a return — rewrite it before emitting.
    let mut body = func.body.clone();
    convert_returns(&mut body, &name);

    // The ByRef parameters of *this* function — their uses get dereferenced.
    let byref: HashSet<String> = func
        .params
        .iter()
        .filter(|p| p.mode == ParamMode::ByRef)
        .map(|p| to_snake(&p.name))
        .collect();

    // Resolver rewrites the body (&mut at call sites, *deref of ByRef params,
    // `as` casts for numeric coercions) and tells us which locals were lent.
    let passed_by_ref = resolver::resolve_body(&mut body, &func.params, fns, diags);

    // Which locals need `let mut`: those reassigned, plus those lent mutably.
    let mut mutated = HashSet::new();
    collect_mutated(&body, &mut mutated);
    mutated.extend(passed_by_ref);

    emit_fn_body(&body, &mutated, &byref, tail_expected, diags, out);
    out.push_str("}\n");
}

fn render_param(p: &Param) -> String {
    let ty = match (p.mode, p.ty) {
        // ByVal String borrows as a read-only &str; other ByVal types pass by value.
        (ParamMode::ByVal, Type::Text) => "&str".to_string(),
        (ParamMode::ByVal, t) => t.rust().to_string(),
        // ByRef always becomes a mutable borrow.
        (ParamMode::ByRef, t) => format!("&mut {}", t.rust()),
    };
    format!("{}: {}", to_snake(&p.name), ty)
}

/// Emit a function body, rendering a trailing `Return value` as an idiomatic
/// tail expression (no `return`, no semicolon) the way the spec shows.
fn emit_fn_body(
    stmts: &[Stmt],
    mutated: &HashSet<String>,
    byref: &HashSet<String>,
    ret: Option<Type>,
    diags: &mut Diagnostics,
    out: &mut String,
) {
    // The tail expression is the last *non-comment* statement, so a trailing
    // inline comment doesn't rob a `Return` of its idiomatic tail form.
    let last_real = stmts.iter().rposition(|s| !matches!(s, Stmt::Comment(_)));
    if let Some(l) = last_real {
        if let Stmt::Return(Some(e)) = &stmts[l] {
            for stmt in &stmts[..l] {
                emit_stmt(stmt, mutated, byref, 1, diags, out);
            }
            // Any trailing comments are emitted just above the returned value.
            for stmt in &stmts[l + 1..] {
                emit_stmt(stmt, mutated, byref, 1, diags, out);
            }
            out.push_str(&format!("    {}\n", render_expr(e, ret)));
            return;
        }
    }
    for stmt in stmts {
        emit_stmt(stmt, mutated, byref, 1, diags, out);
    }
}

/// Rewrite `FunctionName = value` (assignment to the function's own name) into
/// a `Return`, recursing through nested blocks.
fn convert_returns(stmts: &mut [Stmt], fn_name: &str) {
    for stmt in stmts.iter_mut() {
        match stmt {
            Stmt::Assign { name, value } if to_snake(name) == fn_name => {
                *stmt = Stmt::Return(Some(value.clone()));
            }
            Stmt::If {
                branches,
                else_body,
            } => {
                for (_, body) in branches.iter_mut() {
                    convert_returns(body, fn_name);
                }
                if let Some(body) = else_body {
                    convert_returns(body, fn_name);
                }
            }
            Stmt::For { body, .. } => convert_returns(body, fn_name),
            Stmt::Select {
                arms, else_body, ..
            } => {
                for arm in arms.iter_mut() {
                    convert_returns(&mut arm.body, fn_name);
                }
                if let Some(body) = else_body {
                    convert_returns(body, fn_name);
                }
            }
            _ => {}
        }
    }
}

fn emit_block(
    stmts: &[Stmt],
    mutated: &HashSet<String>,
    byref: &HashSet<String>,
    indent: usize,
    diags: &mut Diagnostics,
    out: &mut String,
) {
    for stmt in stmts {
        emit_stmt(stmt, mutated, byref, indent, diags, out);
    }
}

fn emit_stmt(
    stmt: &Stmt,
    mutated: &HashSet<String>,
    byref: &HashSet<String>,
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
            // Assigning through a ByRef parameter writes to the pointee: `*p = …`.
            let target = if byref.contains(&var) {
                format!("*{}", var)
            } else {
                var
            };
            out.push_str(&format!("{}{} = {};\n", pad, target, render_expr(value, None)));
        }
        Stmt::Expr(e) => {
            out.push_str(&format!("{}{};\n", pad, render_expr(e, None)));
        }
        Stmt::Return(Some(e)) => {
            out.push_str(&format!("{}return {};\n", pad, render_expr(e, None)));
        }
        Stmt::Return(None) => {
            out.push_str(&format!("{}return;\n", pad));
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
                emit_block(body, mutated, byref, indent + 1, diags, out);
            }
            if let Some(body) = else_body {
                out.push_str(&format!("{}}} else {{\n", pad));
                emit_block(body, mutated, byref, indent + 1, diags, out);
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
            emit_block(body, mutated, byref, indent + 1, diags, out);
            out.push_str(&format!("{}}}\n", pad));
        }
        Stmt::Select {
            scrutinee,
            arms,
            else_body,
            line: _,
        } => {
            let arm_pad = "    ".repeat(indent + 1);
            out.push_str(&format!("{}match {} {{\n", pad, render_expr(scrutinee, None)));
            for arm in arms {
                let pats: Vec<String> = arm.patterns.iter().map(render_pattern).collect();
                out.push_str(&format!("{}{} => {{\n", arm_pad, pats.join(" | ")));
                emit_block(&arm.body, mutated, byref, indent + 2, diags, out);
                out.push_str(&format!("{}}}\n", arm_pad));
            }
            // `Case Else` is the `_` catch-all (its absence is a hard error upstream).
            if let Some(body) = else_body {
                out.push_str(&format!("{}_ => {{\n", arm_pad));
                emit_block(body, mutated, byref, indent + 2, diags, out);
                out.push_str(&format!("{}}}\n", arm_pad));
            }
            out.push_str(&format!("{}}}\n", pad));
        }
    }
}

fn render_pattern(p: &CasePattern) -> String {
    match p {
        // Ok(v) / Err(e) / Some(x) bind their payload; None stands alone.
        CasePattern::Value(Expr::Call { name, args })
            if matches!(name.as_str(), "Ok" | "Err" | "Some") =>
        {
            let bindings: Vec<String> = args
                .iter()
                .map(|a| match a {
                    Expr::Ident(b) => to_snake(b),
                    other => render_expr(other, None),
                })
                .collect();
            format!("{}({})", name, bindings.join(", "))
        }
        CasePattern::Value(e) => render_expr(e, None),
        CasePattern::Range(lo, hi) => {
            format!("{}..={}", render_expr(lo, None), render_expr(hi, None))
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

/// Walk the body firing the one-time `⚠`/`ℹ` notes for builtins that behave
/// differently than VB programmers expect (indexing, Option/Result returns).
fn note_builtins(stmts: &[Stmt], diags: &mut Diagnostics) {
    for stmt in stmts {
        match stmt {
            Stmt::Dim { init: Some(e), .. } => note_builtins_expr(e, diags),
            Stmt::Set { value, .. } | Stmt::Assign { value, .. } => note_builtins_expr(value, diags),
            Stmt::Return(Some(e)) | Stmt::Print(e) | Stmt::Expr(e) => {
                note_builtins_expr(e, diags)
            }
            Stmt::If {
                branches,
                else_body,
            } => {
                for (cond, body) in branches {
                    note_builtins_expr(cond, diags);
                    note_builtins(body, diags);
                }
                if let Some(body) = else_body {
                    note_builtins(body, diags);
                }
            }
            Stmt::For {
                from, to, step, body, ..
            } => {
                note_builtins_expr(from, diags);
                note_builtins_expr(to, diags);
                if let Some(s) = step {
                    note_builtins_expr(s, diags);
                }
                note_builtins(body, diags);
            }
            Stmt::Select {
                scrutinee,
                arms,
                else_body,
                ..
            } => {
                note_builtins_expr(scrutinee, diags);
                for arm in arms {
                    for pat in &arm.patterns {
                        match pat {
                            CasePattern::Value(e) => note_builtins_expr(e, diags),
                            CasePattern::Range(lo, hi) => {
                                note_builtins_expr(lo, diags);
                                note_builtins_expr(hi, diags);
                            }
                        }
                    }
                    note_builtins(&arm.body, diags);
                }
                if let Some(body) = else_body {
                    note_builtins(body, diags);
                }
            }
            _ => {}
        }
    }
}

fn note_builtins_expr(e: &Expr, diags: &mut Diagnostics) {
    match e {
        Expr::Binary { lhs, rhs, .. } => {
            note_builtins_expr(lhs, diags);
            note_builtins_expr(rhs, diags);
        }
        Expr::MethodCall { recv, method, args } => {
            if method.eq_ignore_ascii_case("unwrap") {
                diags.warn_once_global(
                    "unwrap-training-wheels",
                    ".unwrap() works, but it's training wheels — it crashes the program if the \
                     value is an error or None. Prefer the `?` operator to propagate, or \
                     `Select Case` over Ok/Err (Some/None) to handle both outcomes.",
                );
            }
            note_builtins_expr(recv, diags);
            for a in args {
                note_builtins_expr(a, diags);
            }
        }
        Expr::Try(inner) => note_builtins_expr(inner, diags),
        Expr::Call { name, args } => {
            match name.to_ascii_lowercase().as_str() {
                "mid" => diags.warn_once_global(
                    "builtin-mid",
                    "Mid is 1-indexed in VB but Rust slices are 0-indexed — VBR shifts the \
                     positions for you, so Mid(s, 2, 3) becomes &s[1..4].",
                ),
                "instr" => diags.note(
                    "builtin-instr",
                    "InStr becomes Rust's .find(), which returns an Option: Some(pos) when \
                     found, None when not. You handle both instead of checking for 0.",
                ),
                "val" => diags.note(
                    "builtin-val",
                    "Val becomes Rust's .parse(), which returns a Result: parsing can fail, \
                     so you handle the error rather than getting a silent 0.",
                ),
                "rnd" => diags.error_once(
                    "builtin-rnd",
                    "Rnd() is not built in — Rust keeps randomness in the `rand` crate so it \
                     stays explicit. Add it with `Use rand 0.8`, then:\n\n    \
                     use rand::Rng;\n    \
                     let x: f64 = rand::thread_rng().gen_range(0.0..1.0);",
                ),
                _ => {}
            }
            for a in args {
                note_builtins_expr(a, diags);
            }
        }
        _ => {}
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
            // An integer literal assigned into a float context needs a `.0`.
            if expected.map_or(false, |t| t.is_float()) {
                format!("{}.0", n)
            } else {
                n.to_string()
            }
        }
        Expr::Float(f) => fmt_float(*f),
        Expr::Bool(b) => b.to_string(),
        Expr::Str(s) => format!("\"{}\"", escape(s)),
        // `None` is the Option constructor, not a variable — keep it as-is.
        Expr::Ident(name) if name == "None" => "None".to_string(),
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
        Expr::Binary { op, lhs, rhs } if *op == BinOp::Pow => {
            // `^` lowers to powi (integer exponent) or powf (float exponent),
            // assuming a floating-point base as the spec shows.
            let base = render_math_recv(lhs);
            match rhs.as_ref() {
                Expr::Int(n) => format!("{}.powi({})", base, n),
                Expr::Float(f) => format!("{}.powf({})", base, fmt_float(*f)),
                other => format!("{}.powf({})", base, render_expr(other, None)),
            }
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
            // Method names follow Rust convention (`.Unwrap()` → `.unwrap()`).
            format!(
                "{}.{}({})",
                render_prec(recv, None, 5, false),
                to_snake(method),
                rendered.join(", ")
            )
        }
        Expr::Call { name, args } => {
            if let Some(s) = lower_constructor(name, args) {
                // Ok/Err/Some result/option constructors.
                s
            } else if let Some(s) = lower_builtin(name, args) {
                // Known string/maths builtins lower to idiomatic Rust.
                s
            } else {
                // An ordinary call to a user function.
                let rendered: Vec<String> = args.iter().map(|a| render_expr(a, None)).collect();
                format!("{}({})", to_snake(name), rendered.join(", "))
            }
        }
        Expr::Try(inner) => format!("{}?", render_prec(inner, None, 6, false)),
        Expr::Deref(inner) => format!("*{}", render_prec(inner, expected, 6, false)),
        Expr::MutRef(inner) => format!("&mut {}", render_prec(inner, None, 6, false)),
        Expr::Cast(inner, ty) => {
            // `x as f64`. Parenthesise the cast if it sits under a tighter op.
            let inner = render_prec(inner, None, 6, false);
            let cast = format!("{} as {}", inner, ty.rust());
            if parent_prec > 0 {
                format!("({})", cast)
            } else {
                cast
            }
        }
    }
}

/// Lower the Result/Option constructors. `Err` wraps its message in `.to_string()`.
fn lower_constructor(name: &str, args: &[Expr]) -> Option<String> {
    match (name, args.len()) {
        ("Ok", 1) => Some(format!("Ok({})", render_expr(&args[0], None))),
        ("Some", 1) => Some(format!("Some({})", render_expr(&args[0], None))),
        ("Err", 1) => Some(format!("Err({}.to_string())", render_expr(&args[0], None))),
        _ => None,
    }
}

/// Lower a VB string or maths builtin to Rust, or return `None` if it isn't one
/// (or the argument count doesn't match — then it's treated as a normal call).
fn lower_builtin(name: &str, args: &[Expr]) -> Option<String> {
    let r = |i: usize| render_expr(&args[i], None);
    match (name.to_ascii_lowercase().as_str(), args.len()) {
        // --- strings ---
        ("len", 1) => Some(method0(&args[0], "len")),
        ("ucase", 1) => Some(method0(&args[0], "to_uppercase")),
        ("lcase", 1) => Some(method0(&args[0], "to_lowercase")),
        ("trim", 1) => Some(method0(&args[0], "trim")),
        ("left", 2) => Some(format!("&{}[..{}]", r(0), r(1))),
        ("right", 2) => Some(format!("&{0}[{0}.len() - {1}..]", r(0), r(1))),
        ("replace", 3) => Some(format!("{}.replace({}, {})", r(0), r(1), r(2))),
        ("str", 1) => Some(method0(&args[0], "to_string")),
        // InStr → .find() (returns Option); Val → .parse() (returns Result).
        ("instr", 2) => Some(format!("{}.find({})", r(0), r(1))),
        ("val", 1) => Some(format!("{}.parse::<f64>()", r(0))),
        // Mid is 1-indexed in VB; Rust slices are 0-indexed, so shift by one.
        ("mid", 3) => Some(render_mid(&args[0], &args[1], Some(&args[2]))),
        ("mid", 2) => Some(render_mid(&args[0], &args[1], None)),
        // --- maths (assume a floating-point argument) ---
        ("sqr", 1) => Some(math0(&args[0], "sqrt")),
        ("abs", 1) => Some(math0(&args[0], "abs")),
        ("int", 1) => Some(math0(&args[0], "floor")),
        ("round", 1) => Some(math0(&args[0], "round")),
        ("sin", 1) => Some(math0(&args[0], "sin")),
        ("cos", 1) => Some(math0(&args[0], "cos")),
        ("tan", 1) => Some(math0(&args[0], "tan")),
        ("log", 1) => Some(math0(&args[0], "ln")),
        ("exp", 1) => Some(math0(&args[0], "exp")),
        _ => None,
    }
}

/// `recv.method()` for a string builtin: parenthesise the receiver if needed.
fn method0(recv: &Expr, method: &str) -> String {
    format!("{}.{}()", render_recv(recv), method)
}

/// `recv.method()` for a maths builtin: a bare numeric literal receiver is
/// ambiguous between f32/f64, so tag it `f64` (`3.7.floor()` won't compile,
/// `3.7f64.floor()` will). Variables keep their declared type.
fn math0(recv: &Expr, method: &str) -> String {
    format!("{}.{}()", render_math_recv(recv), method)
}

fn render_recv(e: &Expr) -> String {
    let s = render_prec(e, None, 6, false);
    // Parenthesise a leading unary so `(-5.0).abs()` / `(*p).foo()` parse right.
    if s.starts_with('-') || s.starts_with('*') {
        format!("({})", s)
    } else {
        s
    }
}

fn render_math_recv(e: &Expr) -> String {
    match e {
        Expr::Int(n) => suffix_f64(n.to_string()),
        Expr::Float(f) => suffix_f64(fmt_float(*f)),
        _ => render_recv(e),
    }
}

fn suffix_f64(literal: String) -> String {
    let typed = format!("{}f64", literal);
    if typed.starts_with('-') {
        format!("({})", typed)
    } else {
        typed
    }
}

/// `Mid(s, start)` / `Mid(s, start, len)` → a 0-indexed `&s[..]` slice. When the
/// positions are literals we fold the arithmetic so the output stays clean.
fn render_mid(s: &Expr, start: &Expr, len: Option<&Expr>) -> String {
    let s = render_expr(s, None);
    match (start, len) {
        (Expr::Int(start), Some(Expr::Int(len))) => {
            let lo = start - 1;
            format!("&{}[{}..{}]", s, lo, lo + len)
        }
        (Expr::Int(start), None) => format!("&{}[{}..]", s, start - 1),
        (_, Some(len)) => {
            let start = render_expr(start, None);
            let len = render_expr(len, None);
            format!("&{0}[({1} - 1)..({1} - 1 + {2})]", s, start, len)
        }
        (_, None) => format!("&{}[({} - 1)..]", s, render_expr(start, None)),
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
        BinOp::Pow => 5,
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
        BinOp::Pow => "^",    // handled separately (lowers to powi/powf)
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
            Stmt::Select {
                arms, else_body, ..
            } => {
                for arm in arms {
                    collect_mutated(&arm.body, set);
                }
                if let Some(body) = else_body {
                    collect_mutated(body, set);
                }
            }
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
pub(crate) fn to_snake(name: &str) -> String {
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
