//! `Parallel For` — iterations that must be independent.
//!
//! The source says the index space is free of loop-carried writes. CUDA (and
//! other devices) are later backends for the same claim; this module is the
//! checker that makes the claim hold, plus the scan the Rust emitter uses to
//! know whether to emit the CPU-thread helper.

use std::collections::HashSet;

use crate::ast::*;
use crate::diagnostics::Diagnostics;
use crate::transpiler::is_mutating_method;

/// CPU-thread runtime for `Parallel For`. Emitted once, only when the program
/// contains one. `std::thread::scope` plus chunking; wasm falls back to a
/// sequential walk (no threads). Pointer writes are in the caller.
pub const PARALLEL_HELPER: &str = "fn __vbr_parallel_for(n: usize, f: &(dyn Fn(usize) + Sync)) {
    if n == 0 {
        return;
    }
    #[cfg(target_arch = \"wasm32\")]
    {
        for k in 0..n {
            f(k);
        }
    }
    #[cfg(not(target_arch = \"wasm32\"))]
    {
        let threads = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(1)
            .clamp(1, n);
        let chunk = (n + threads - 1) / threads;
        std::thread::scope(|scope| {
            for t in 0..threads {
                let start = t * chunk;
                if start >= n {
                    break;
                }
                let end = (start + chunk).min(n);
                scope.spawn(move || {
                    for k in start..end {
                        f(k);
                    }
                });
            }
        });
    }
}

#[allow(dead_code)]
#[inline(always)]
unsafe fn __vbr_at<T>(p: usize, i: usize) -> *mut T {
    (p as *mut T).add(i)
}
";

/// True when any `Parallel For` appears in the program (so the helper is
/// emitted, and only then).
pub fn program_uses_parallel_for(program: &Program) -> bool {
    let any = |stmts: &[Stmt]| stmts.iter().any(stmt_uses_parallel);
    program.functions.iter().any(|f| any(&f.body))
        || program.tests.iter().any(|t| any(&t.body))
        || program.windows.iter().any(|w| {
            w.events.iter().any(|e| any(&e.body)) || w.subs.iter().any(|s| any(&s.body))
        })
        || program.sketches.iter().any(|s| {
            any(&s.draw)
                || s.events.iter().any(|e| any(&e.body))
                || s.subs.iter().any(|sub| any(&sub.body))
        })
        || program.screens.iter().any(|s| {
            s.events.iter().any(|e| any(&e.body)) || s.subs.iter().any(|sub| any(&sub.body))
        })
        || program.pages.iter().any(|p| {
            p.events.iter().any(|e| any(&e.body)) || p.subs.iter().any(|s| any(&s.body))
        })
        || program.canvases.iter().any(|c| any(&c.body))
        || program.godot_nodes.iter().any(|n| {
            n.events.iter().any(|e| any(&e.body)) || n.handlers.iter().any(|h| any(&h.body))
        })
}

fn stmt_uses_parallel(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::For { parallel, body, .. } => *parallel || body.iter().any(stmt_uses_parallel),
        Stmt::ForEach { body, .. }
        | Stmt::DoLoop { body, .. }
        | Stmt::GpuInto { body, .. }
        | Stmt::HandleErr { body, .. } => body.iter().any(stmt_uses_parallel),
        Stmt::If { branches, else_body } => {
            branches.iter().any(|(_, b)| b.iter().any(stmt_uses_parallel))
                || else_body.as_ref().is_some_and(|b| b.iter().any(stmt_uses_parallel))
        }
        Stmt::Match { arms, .. } => arms.iter().any(|a| a.body.iter().any(stmt_uses_parallel)),
        _ => false,
    }
}

/// Independence check for one `Parallel For`. Called after the body is
/// resolved, so indexes and types are in their final shape.
pub fn check(
    var: &str,
    step: Option<&Expr>,
    body: &[Stmt],
    ty: Type,
    line: usize,
    diags: &mut Diagnostics,
) {
    if ty.is_float() || !literal_step_ok(step) {
        diags.error(
            line,
            "`Parallel For` needs an integer range and a fixed integer Step \
             (or none, which is 1). A variable or floating Step is a counted \
             sequential `For`, not a parallel index space.",
        );
        return;
    }
    if has_nested_parallel(body) {
        diags.error(
            line,
            "Nested `Parallel For` isn't supported yet — a 2-D index space \
             (`Parallel For y` / `Parallel For x`) is a later slice. Use one \
             `Parallel For` over a flat range for now.",
        );
        return;
    }

    let mut locals = HashSet::new();
    collect_locals(body, &mut locals);

    let mut ok = true;
    if !check_control(body, line, diags) {
        ok = false;
    }
    if !check_writes(var, body, &locals, line, diags) {
        ok = false;
    }
    if !ok {
        return;
    }

    let written = written_arrays(var, body, &locals);
    check_reads(var, body, &written, line, diags);
    check_mutating_calls(body, &locals, line, diags);
}

/// Names of non-local arrays this loop writes at `[i]` — the Rust emitter
/// takes raw pointers to these.
pub fn written_arrays(var: &str, body: &[Stmt], locals: &HashSet<String>) -> HashSet<String> {
    let mut out = HashSet::new();
    collect_written(var, body, locals, &mut out);
    out
}

/// Locals `Dim`'d (and nested loop vars) inside this body.
pub fn collect_locals(stmts: &[Stmt], out: &mut HashSet<String>) {
    for s in stmts {
        match s {
            Stmt::Dim { name, .. } | Stmt::HandleDim { name, .. } => {
                out.insert(name.to_ascii_lowercase());
            }
            Stmt::DestructureDim { names, .. } => {
                for n in names {
                    out.insert(n.to_ascii_lowercase());
                }
            }
            Stmt::HandleErr { err_name, body, .. } => {
                out.insert(err_name.to_ascii_lowercase());
                collect_locals(body, out);
            }
            Stmt::For { var, body, .. } => {
                out.insert(var.to_ascii_lowercase());
                collect_locals(body, out);
            }
            Stmt::ForEach { var1, var2, body, .. } => {
                out.insert(var1.to_ascii_lowercase());
                if let Some(v2) = var2 {
                    out.insert(v2.to_ascii_lowercase());
                }
                collect_locals(body, out);
            }
            Stmt::DoLoop { body, .. } | Stmt::GpuInto { body, .. } => collect_locals(body, out),
            Stmt::If { branches, else_body } => {
                for (_, b) in branches {
                    collect_locals(b, out);
                }
                if let Some(b) = else_body {
                    collect_locals(b, out);
                }
            }
            Stmt::Match { arms, .. } => {
                for a in arms {
                    collect_locals(&a.body, out);
                }
            }
            _ => {}
        }
    }
}

fn literal_step_ok(step: Option<&Expr>) -> bool {
    match step {
        None => true,
        Some(Expr { kind: ExprKind::Int(n), .. }) if *n != 0 => true,
        // A Cast around a literal Step (resolver widening) still counts.
        Some(Expr { kind: ExprKind::Cast(inner, _), .. }) => literal_step_ok(Some(inner)),
        _ => false,
    }
}

fn has_nested_parallel(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::For { parallel: true, .. } => true,
        Stmt::For { body, .. }
        | Stmt::ForEach { body, .. }
        | Stmt::DoLoop { body, .. }
        | Stmt::GpuInto { body, .. }
        | Stmt::HandleErr { body, .. } => has_nested_parallel(body),
        Stmt::If { branches, else_body } => {
            branches.iter().any(|(_, b)| has_nested_parallel(b))
                || else_body.as_ref().is_some_and(|b| has_nested_parallel(b))
        }
        Stmt::Match { arms, .. } => arms.iter().any(|a| has_nested_parallel(&a.body)),
        _ => false,
    })
}

fn check_control(stmts: &[Stmt], line: usize, diags: &mut Diagnostics) -> bool {
    for s in stmts {
        match s {
            Stmt::Break => {
                diags.error(
                    line,
                    "`Exit For` isn't allowed inside `Parallel For` — iterations \
                     must all run. Drop the early exit, or use an ordinary `For`.",
                );
                return false;
            }
            Stmt::Continue => {
                diags.error(
                    line,
                    "`Continue` isn't allowed inside `Parallel For` — iterations \
                     must all run. Drop it, or use an ordinary `For`.",
                );
                return false;
            }
            Stmt::Return(_) => {
                diags.error(
                    line,
                    "`Return` isn't allowed inside `Parallel For` — a function \
                     return is a loop-carried exit. Compute a flag into an array \
                     slot, or use an ordinary `For`.",
                );
                return false;
            }
            Stmt::For { body, .. }
            | Stmt::ForEach { body, .. }
            | Stmt::DoLoop { body, .. }
            | Stmt::GpuInto { body, .. }
            | Stmt::HandleErr { body, .. } => {
                if !check_control(body, line, diags) {
                    return false;
                }
            }
            Stmt::If { branches, else_body } => {
                for (_, b) in branches {
                    if !check_control(b, line, diags) {
                        return false;
                    }
                }
                if let Some(b) = else_body {
                    if !check_control(b, line, diags) {
                        return false;
                    }
                }
            }
            Stmt::Match { arms, .. } => {
                for a in arms {
                    if !check_control(&a.body, line, diags) {
                        return false;
                    }
                }
            }
            _ => {}
        }
    }
    true
}

fn check_writes(
    var: &str,
    stmts: &[Stmt],
    locals: &HashSet<String>,
    line: usize,
    diags: &mut Diagnostics,
) -> bool {
    for s in stmts {
        match s {
            Stmt::Assign { target, .. } => {
                if !check_write_target(var, target, locals, line, diags) {
                    return false;
                }
            }
            Stmt::Set { name, .. } => {
                if !is_local(name, locals) {
                    diags.error(
                        line,
                        format!(
                            "`Parallel For` can't `Set {name}` — that's a shared \
                             write. Each iteration may write `arr[{var}]` of a \
                             different slot, not a scalar."
                        ),
                    );
                    return false;
                }
            }
            Stmt::Destroy { name, .. } => {
                if !is_local(name, locals) {
                    diags.error(
                        line,
                        format!(
                            "`Parallel For` can't release `{name}` — that's a \
                             shared write. Drop is for after the loop."
                        ),
                    );
                    return false;
                }
            }
            Stmt::HandleErr { target: Some(t), body, .. } => {
                if !check_write_target(var, t, locals, line, diags) {
                    return false;
                }
                if !check_writes(var, body, locals, line, diags) {
                    return false;
                }
            }
            Stmt::HandleErr { body, .. }
            | Stmt::For { body, .. }
            | Stmt::ForEach { body, .. }
            | Stmt::DoLoop { body, .. }
            | Stmt::GpuInto { body, .. } => {
                if !check_writes(var, body, locals, line, diags) {
                    return false;
                }
            }
            Stmt::If { branches, else_body } => {
                for (_, b) in branches {
                    if !check_writes(var, b, locals, line, diags) {
                        return false;
                    }
                }
                if let Some(b) = else_body {
                    if !check_writes(var, b, locals, line, diags) {
                        return false;
                    }
                }
            }
            Stmt::Match { arms, .. } => {
                for a in arms {
                    if !check_writes(var, &a.body, locals, line, diags) {
                        return false;
                    }
                }
            }
            _ => {}
        }
    }
    true
}

fn check_write_target(
    var: &str,
    target: &Expr,
    locals: &HashSet<String>,
    line: usize,
    diags: &mut Diagnostics,
) -> bool {
    let t = peel(target);
    match &t.kind {
        ExprKind::Ident(name) => {
            if is_local(name, locals) {
                true
            } else {
                diags.error(
                    line,
                    format!(
                        "`Parallel For` iterations can't all write `{name}` — that's \
                         a race. Each iteration may write a different slot \
                         (`out[{var}] = …`). A reduction (`total = total + …`) \
                         wants `Parallel Sum` later, not a shared scalar."
                    ),
                );
                false
            }
        }
        ExprKind::Index(inner, idx) => {
            let Some(arr) = ident_of(inner) else {
                diags.error(
                    line,
                    "`Parallel For` can only write `arr[i]` where `arr` is a \
                     named Vec or array — not a computed place.",
                );
                return false;
            };
            if is_local(&arr, locals) {
                return true;
            }
            if !is_loop_var(idx, var) {
                diags.error(
                    line,
                    format!(
                        "`Parallel For {var}` can only write `{arr}[{var}]` — two \
                         iterations writing the same slot would race. An offset \
                         (`{arr}[{var} + 1] = …`) isn't independent."
                    ),
                );
                return false;
            }
            true
        }
        ExprKind::Field(_, field) => {
            diags.error(
                line,
                format!(
                    "`Parallel For` can't write a shared field (`.{field}`). \
                     Write `arr[{var}]` of a Vec instead."
                ),
            );
            false
        }
        _ => {
            diags.error(
                line,
                format!(
                    "`Parallel For` writes must be `arr[{var}]` (a different \
                     slot per iteration) or a `Dim` local to the body."
                ),
            );
            false
        }
    }
}

fn collect_written(
    var: &str,
    stmts: &[Stmt],
    locals: &HashSet<String>,
    out: &mut HashSet<String>,
) {
    for s in stmts {
        match s {
            Stmt::Assign { target, .. } => {
                if let ExprKind::Index(inner, idx) = &peel(target).kind {
                    if is_loop_var(idx, var) {
                        if let Some(arr) = ident_of(inner) {
                            if !is_local(&arr, locals) {
                                out.insert(arr);
                            }
                        }
                    }
                }
            }
            Stmt::HandleErr { target, body, .. } => {
                if let Some(target) = target {
                    if let ExprKind::Index(inner, idx) = &peel(target).kind {
                        if is_loop_var(idx, var) {
                            if let Some(arr) = ident_of(inner) {
                                if !is_local(&arr, locals) {
                                    out.insert(arr);
                                }
                            }
                        }
                    }
                }
                collect_written(var, body, locals, out);
            }
            Stmt::For { body, .. }
            | Stmt::ForEach { body, .. }
            | Stmt::DoLoop { body, .. }
            | Stmt::GpuInto { body, .. } => collect_written(var, body, locals, out),
            Stmt::If { branches, else_body } => {
                for (_, b) in branches {
                    collect_written(var, b, locals, out);
                }
                if let Some(b) = else_body {
                    collect_written(var, b, locals, out);
                }
            }
            Stmt::Match { arms, .. } => {
                for a in arms {
                    collect_written(var, &a.body, locals, out);
                }
            }
            _ => {}
        }
    }
}

fn check_reads(
    var: &str,
    stmts: &[Stmt],
    written: &HashSet<String>,
    line: usize,
    diags: &mut Diagnostics,
) {
    for s in stmts {
        match s {
            Stmt::Dim { init: Some(e), .. }
            | Stmt::Set { value: e, .. }
            | Stmt::DestructureDim { value: e, .. }
            | Stmt::Return(Some(e))
            | Stmt::RaiseError(e)
            | Stmt::Print(e)
            | Stmt::Log(_, e)
            | Stmt::Expr(e)
            | Stmt::Assert(e) => check_read_expr(var, e, written, line, diags),
            Stmt::Assign { target, value, .. } => {
                check_read_expr(var, target, written, line, diags);
                check_read_expr(var, value, written, line, diags);
            }
            Stmt::If { branches, else_body } => {
                for (c, b) in branches {
                    check_read_expr(var, c, written, line, diags);
                    check_reads(var, b, written, line, diags);
                }
                if let Some(b) = else_body {
                    check_reads(var, b, written, line, diags);
                }
            }
            Stmt::For { from, to, step, body, .. } => {
                check_read_expr(var, from, written, line, diags);
                check_read_expr(var, to, written, line, diags);
                if let Some(st) = step {
                    check_read_expr(var, st, written, line, diags);
                }
                check_reads(var, body, written, line, diags);
            }
            Stmt::ForEach { iter, body, .. } => {
                if let Some(arr) = ident_of(iter) {
                    if written.contains(&arr) {
                        diags.error(
                            line,
                            format!(
                                "`For Each` over `{arr}` isn't allowed inside `Parallel For` \
                                 while `{arr}` is also being written — that's a conflicting \
                                 whole-array read. Index `{arr}[{var}]` instead."
                            ),
                        );
                    }
                }
                check_read_expr(var, iter, written, line, diags);
                check_reads(var, body, written, line, diags);
            }
            Stmt::DoLoop { cond, body } => {
                if let Some(
                    DoCond::PreWhile(c)
                    | DoCond::PreUntil(c)
                    | DoCond::PostWhile(c)
                    | DoCond::PostUntil(c),
                ) = cond
                {
                    check_read_expr(var, c, written, line, diags);
                }
                check_reads(var, body, written, line, diags);
            }
            Stmt::Match { scrutinee, arms, .. } => {
                check_read_expr(var, scrutinee, written, line, diags);
                for a in arms {
                    if let Some(g) = &a.guard {
                        check_read_expr(var, g, written, line, diags);
                    }
                    check_reads(var, &a.body, written, line, diags);
                }
            }
            Stmt::HandleErr { target, call, body, .. } => {
                if let Some(t) = target {
                    check_read_expr(var, t, written, line, diags);
                }
                check_read_expr(var, call, written, line, diags);
                check_reads(var, body, written, line, diags);
            }
            Stmt::GpuInto { body, .. } => check_reads(var, body, written, line, diags),
            _ => {}
        }
    }
}

fn check_read_expr(
    var: &str,
    e: &Expr,
    written: &HashSet<String>,
    line: usize,
    diags: &mut Diagnostics,
) {
    match &e.kind {
        ExprKind::Index(inner, idx) => {
            if let Some(arr) = ident_of(inner) {
                if written.contains(&arr) && !is_loop_var(idx, var) {
                    diags.error(
                        line,
                        format!(
                            "`{arr}` is written inside this `Parallel For`, so a read \
                             must be `{arr}[{var}]` too — `{arr}[{var} + 1]` would race \
                             with another iteration's write. Stencil a *different* \
                             array (`in[{var} + 1]` while writing `out[{var}]`)."
                        ),
                    );
                }
            }
            check_read_expr(var, inner, written, line, diags);
            check_read_expr(var, idx, written, line, diags);
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            check_read_expr(var, lhs, written, line, diags);
            check_read_expr(var, rhs, written, line, diags);
        }
        ExprKind::MethodCall { recv, method, args } => {
            check_method_on_written(var, recv, method, args, written, line, diags);
            check_read_expr(var, recv, written, line, diags);
            for a in args {
                check_read_expr(var, a, written, line, diags);
            }
        }
        ExprKind::Call { args, .. } | ExprKind::Tuple(args) | ExprKind::List(args) => {
            for a in args {
                check_read_expr(var, a, written, line, diags);
            }
        }
        ExprKind::StructLit { fields, .. } => {
            for (_, v) in fields {
                check_read_expr(var, v, written, line, diags);
            }
        }
        ExprKind::Field(inner, _)
        | ExprKind::Deref(inner)
        | ExprKind::MutRef(inner)
        | ExprKind::Ref(inner)
        | ExprKind::Cast(inner, _)
        | ExprKind::Try(inner)
        | ExprKind::Raw(inner)
        | ExprKind::Not(inner)
        | ExprKind::Await(inner)
        | ExprKind::TupleIndex(inner, _)
        | ExprKind::Closure { body: inner, .. } => {
            check_read_expr(var, inner, written, line, diags);
        }
        _ => {}
    }
}

fn check_method_on_written(
    var: &str,
    recv: &Expr,
    method: &str,
    args: &[Expr],
    written: &HashSet<String>,
    line: usize,
    diags: &mut Diagnostics,
) {
    let Some(arr) = ident_of(recv) else {
        return;
    };
    if !written.contains(&arr) {
        return;
    }
    let m = method.to_ascii_lowercase();
    if matches!(
        m.as_str(),
        "len" | "count" | "capacity" | "isempty" | "is_empty" | "clone"
    ) {
        return;
    }
    if m == "get" && args.len() == 1 && is_loop_var(&args[0], var) {
        return;
    }
    diags.error(
        line,
        format!(
            "`{arr}.{method}(…)` reads or mutates a Vec this `Parallel For` is \
             writing. Index `{arr}[{var}]` (or `.Get({var})`) instead."
        ),
    );
}

fn check_mutating_calls(stmts: &[Stmt], locals: &HashSet<String>, line: usize, diags: &mut Diagnostics) {
    for s in stmts {
        match s {
            Stmt::Expr(e)
            | Stmt::Print(e)
            | Stmt::Log(_, e)
            | Stmt::Assert(e)
            | Stmt::RaiseError(e)
            | Stmt::Return(Some(e)) => walk_mutating(e, locals, line, diags),
            Stmt::Dim { init: Some(e), .. }
            | Stmt::Set { value: e, .. }
            | Stmt::DestructureDim { value: e, .. } => walk_mutating(e, locals, line, diags),
            Stmt::Assign { target, value, .. } => {
                walk_mutating(target, locals, line, diags);
                walk_mutating(value, locals, line, diags);
            }
            Stmt::If { branches, else_body } => {
                for (c, b) in branches {
                    walk_mutating(c, locals, line, diags);
                    check_mutating_calls(b, locals, line, diags);
                }
                if let Some(b) = else_body {
                    check_mutating_calls(b, locals, line, diags);
                }
            }
            Stmt::For { from, to, step, body, .. } => {
                walk_mutating(from, locals, line, diags);
                walk_mutating(to, locals, line, diags);
                if let Some(st) = step {
                    walk_mutating(st, locals, line, diags);
                }
                check_mutating_calls(body, locals, line, diags);
            }
            Stmt::ForEach { iter, body, .. } => {
                walk_mutating(iter, locals, line, diags);
                check_mutating_calls(body, locals, line, diags);
            }
            Stmt::DoLoop { cond, body } => {
                if let Some(
                    DoCond::PreWhile(c)
                    | DoCond::PreUntil(c)
                    | DoCond::PostWhile(c)
                    | DoCond::PostUntil(c),
                ) = cond
                {
                    walk_mutating(c, locals, line, diags);
                }
                check_mutating_calls(body, locals, line, diags);
            }
            Stmt::Match { scrutinee, arms, .. } => {
                walk_mutating(scrutinee, locals, line, diags);
                for a in arms {
                    if let Some(g) = &a.guard {
                        walk_mutating(g, locals, line, diags);
                    }
                    check_mutating_calls(&a.body, locals, line, diags);
                }
            }
            Stmt::HandleErr { call, body, .. } => {
                walk_mutating(call, locals, line, diags);
                check_mutating_calls(body, locals, line, diags);
            }
            Stmt::GpuInto { body, .. } => check_mutating_calls(body, locals, line, diags),
            _ => {}
        }
    }
}

fn walk_mutating(e: &Expr, locals: &HashSet<String>, line: usize, diags: &mut Diagnostics) {
    match &e.kind {
        ExprKind::MethodCall { recv, method, args } => {
            if is_mutating_method(&method.to_ascii_lowercase()) {
                if let Some(name) = ident_of(recv) {
                    if !is_local(&name, locals) {
                        diags.error(
                            line,
                            format!(
                                "`{name}.{method}(…)` mutates a shared collection inside \
                                 `Parallel For`. Iterations must be independent — write \
                                 `arr[i] = …` of a pre-sized Vec instead of Push/Pop."
                            ),
                        );
                    }
                } else {
                    diags.error(
                        line,
                        format!(
                            "`.{method}(…)` mutates a collection inside `Parallel For`. \
                             Iterations must be independent."
                        ),
                    );
                }
            }
            walk_mutating(recv, locals, line, diags);
            for a in args {
                walk_mutating(a, locals, line, diags);
            }
        }
        ExprKind::Call { args, .. } | ExprKind::Tuple(args) | ExprKind::List(args) => {
            for a in args {
                walk_mutating(a, locals, line, diags);
            }
        }
        ExprKind::Binary { lhs, rhs, .. } | ExprKind::Index(lhs, rhs) => {
            walk_mutating(lhs, locals, line, diags);
            walk_mutating(rhs, locals, line, diags);
        }
        ExprKind::StructLit { fields, .. } => {
            for (_, v) in fields {
                walk_mutating(v, locals, line, diags);
            }
        }
        ExprKind::Field(inner, _)
        | ExprKind::Deref(inner)
        | ExprKind::MutRef(inner)
        | ExprKind::Ref(inner)
        | ExprKind::Cast(inner, _)
        | ExprKind::Try(inner)
        | ExprKind::Raw(inner)
        | ExprKind::Not(inner)
        | ExprKind::Await(inner)
        | ExprKind::TupleIndex(inner, _)
        | ExprKind::Closure { body: inner, .. } => walk_mutating(inner, locals, line, diags),
        _ => {}
    }
}

fn peel(e: &Expr) -> &Expr {
    match &e.kind {
        ExprKind::Cast(inner, _)
        | ExprKind::Deref(inner)
        | ExprKind::Ref(inner)
        | ExprKind::MutRef(inner)
        | ExprKind::Try(inner)
        | ExprKind::Raw(inner) => peel(inner),
        _ => e,
    }
}

fn ident_of(e: &Expr) -> Option<String> {
    match &peel(e).kind {
        ExprKind::Ident(n) => Some(n.to_ascii_lowercase()),
        _ => None,
    }
}

fn is_loop_var(e: &Expr, var: &str) -> bool {
    matches!(&peel(e).kind, ExprKind::Ident(n) if n.eq_ignore_ascii_case(var))
}

fn is_local(name: &str, locals: &HashSet<String>) -> bool {
    locals.contains(&name.to_ascii_lowercase())
}
