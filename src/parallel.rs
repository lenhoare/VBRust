//! `Parallel For` / `Parallel Sum` — independent iterations and a first-class
//! reduction.
//!
//! The source says the index space is free of loop-carried writes, or that a
//! Vec is summed as a proven reduction. CUDA (`CudaBuffer` + `CUDA.*`) is a
//! backend for the same claim when every array in the loop lives on the
//! device; this module is the CPU checker plus the scan the Rust emitter
//! uses to know whether to emit the CPU-thread helpers.

use std::collections::HashSet;

use crate::ast::*;
use crate::diagnostics::Diagnostics;
use crate::transpiler::is_mutating_method;

/// Why Python and C refuse `Parallel For` / `Parallel Sum` rather than silently looping.
pub const RUST_ONLY: &str = "`Parallel For` and `Parallel Sum` are Rust-only. Python and C have no parallel \
backend, and we won't emit a sequential loop that pretends otherwise. Run it with `vbr run`.";

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

/// CPU-thread reduction for `Parallel Sum`. Per-thread partial sums, then a
/// sequential combine — no atomics. Empty input is `T::default()` (0 / 0.0).
pub const PARALLEL_SUM_HELPER: &str = "#[allow(dead_code)]
fn __vbr_parallel_sum<T>(xs: &[T]) -> T
where
    T: Copy + Default + std::ops::Add<Output = T> + Send + Sync,
{
    let n = xs.len();
    if n == 0 {
        return T::default();
    }
    #[cfg(target_arch = \"wasm32\")]
    {
        return xs.iter().copied().fold(T::default(), |a, b| a + b);
    }
    #[cfg(not(target_arch = \"wasm32\"))]
    {
        let threads = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(1)
            .clamp(1, n);
        let chunk = (n + threads - 1) / threads;
        let mut parts = vec![T::default(); threads];
        std::thread::scope(|scope| {
            for (t, slot) in parts.iter_mut().enumerate() {
                let start = t * chunk;
                if start >= n {
                    break;
                }
                let end = (start + chunk).min(n);
                let slice = &xs[start..end];
                scope.spawn(move || {
                    *slot = slice.iter().copied().fold(T::default(), |a, b| a + b);
                });
            }
        });
        parts.into_iter().fold(T::default(), |a, b| a + b)
    }
}
";

/// True when any **CPU** `Parallel For` appears (so the thread helper is
/// emitted). A device loop over `CudaBuffer`s uses the CUDA helper instead.
/// Walks `Dim` types because this runs before the resolver sets `For.device`.
pub fn program_uses_parallel_for(program: &Program) -> bool {
    let any = |stmts: &[Stmt]| fn_needs_cpu_parallel(stmts);
    program.functions.iter().any(|f| params_need_cpu(&f.params, &f.body))
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

fn params_need_cpu(params: &[Param], body: &[Stmt]) -> bool {
    let mut cuda = HashSet::new();
    let mut host = HashSet::new();
    for p in params {
        match &p.ty {
            DeclType::CudaBuffer(_) => {
                cuda.insert(p.name.to_ascii_lowercase());
            }
            DeclType::Vec(_) | DeclType::Array(..) | DeclType::Array2D(..) => {
                host.insert(p.name.to_ascii_lowercase());
            }
            _ => {}
        }
    }
    collect_dim_arrays(body, &mut cuda, &mut host);
    stmts_need_cpu_parallel(body, &cuda, &host)
}

fn fn_needs_cpu_parallel(stmts: &[Stmt]) -> bool {
    let mut cuda = HashSet::new();
    let mut host = HashSet::new();
    collect_dim_arrays(stmts, &mut cuda, &mut host);
    stmts_need_cpu_parallel(stmts, &cuda, &host)
}

fn collect_dim_arrays(stmts: &[Stmt], cuda: &mut HashSet<String>, host: &mut HashSet<String>) {
    for s in stmts {
        match s {
            Stmt::Dim { name, ty, .. } => match ty {
                DeclType::CudaBuffer(_) => {
                    cuda.insert(name.to_ascii_lowercase());
                }
                DeclType::Vec(_) | DeclType::Array(..) | DeclType::Array2D(..) => {
                    host.insert(name.to_ascii_lowercase());
                }
                _ => {}
            },
            Stmt::If { branches, else_body } => {
                for (_, b) in branches {
                    collect_dim_arrays(b, cuda, host);
                }
                if let Some(b) = else_body {
                    collect_dim_arrays(b, cuda, host);
                }
            }
            Stmt::For { body, .. }
            | Stmt::ForEach { body, .. }
            | Stmt::DoLoop { body, .. }
            | Stmt::GpuInto { body, .. }
            | Stmt::HandleErr { body, .. } => collect_dim_arrays(body, cuda, host),
            Stmt::Match { arms, .. } => {
                for a in arms {
                    collect_dim_arrays(&a.body, cuda, host);
                }
            }
            _ => {}
        }
    }
}

fn stmts_need_cpu_parallel(stmts: &[Stmt], cuda: &HashSet<String>, host: &HashSet<String>) -> bool {
    stmts.iter().any(|s| stmt_needs_cpu_parallel(s, cuda, host))
}

fn stmt_needs_cpu_parallel(stmt: &Stmt, cuda: &HashSet<String>, host: &HashSet<String>) -> bool {
    match stmt {
        Stmt::For { parallel: true, body, .. } => {
            let idx = crate::cuda::indexed_names(body);
            let uses_cuda = idx.iter().any(|n| cuda.contains(&n.to_ascii_lowercase()));
            let uses_host = idx.iter().any(|n| host.contains(&n.to_ascii_lowercase()));
            (uses_host || !uses_cuda) || stmts_need_cpu_parallel(body, cuda, host)
        }
        Stmt::For { body, .. }
        | Stmt::ForEach { body, .. }
        | Stmt::DoLoop { body, .. }
        | Stmt::GpuInto { body, .. }
        | Stmt::HandleErr { body, .. } => stmts_need_cpu_parallel(body, cuda, host),
        Stmt::If { branches, else_body } => {
            branches.iter().any(|(_, b)| stmts_need_cpu_parallel(b, cuda, host))
                || else_body.as_ref().is_some_and(|b| stmts_need_cpu_parallel(b, cuda, host))
        }
        Stmt::Match { arms, .. } => arms
            .iter()
            .any(|a| stmts_need_cpu_parallel(&a.body, cuda, host)),
        _ => false,
    }
}

/// True when any `Parallel Sum` appears (so Python/C refuse, and only then).
pub fn program_uses_parallel_sum(program: &Program) -> bool {
    program_has_parallel_sum(program, false)
}

/// True when any **host** `Parallel Sum` appears (so the CPU-thread helper is
/// emitted). A device sum over `CudaBuffer` uses the CUDA helper instead.
pub fn program_uses_host_parallel_sum(program: &Program) -> bool {
    program_has_parallel_sum(program, true)
}

fn program_has_parallel_sum(program: &Program, host_only: bool) -> bool {
    let any = |stmts: &[Stmt]| stmts.iter().any(|s| stmt_uses_parallel_sum(s, host_only));
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

pub fn program_uses_parallel(program: &Program) -> bool {
    program_uses_any_parallel_for(program) || program_uses_parallel_sum(program)
}

fn program_uses_any_parallel_for(program: &Program) -> bool {
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

fn stmt_uses_parallel_sum(stmt: &Stmt, host_only: bool) -> bool {
    match stmt {
        Stmt::Dim { init: Some(e), .. }
        | Stmt::Set { value: e, .. }
        | Stmt::DestructureDim { value: e, .. }
        | Stmt::Return(Some(e))
        | Stmt::Print(e)
        | Stmt::Log(_, e)
        | Stmt::Expr(e)
        | Stmt::RaiseError(e)
        | Stmt::Assert(e) => expr_uses_parallel_sum(e, host_only),
        Stmt::Assign { target, value, .. } => {
            expr_uses_parallel_sum(target, host_only) || expr_uses_parallel_sum(value, host_only)
        }
        Stmt::If {
            branches,
            else_body,
        } => {
            branches
                .iter()
                .any(|(c, b)| {
                    expr_uses_parallel_sum(c, host_only)
                        || b.iter().any(|s| stmt_uses_parallel_sum(s, host_only))
                })
                || else_body
                    .as_ref()
                    .is_some_and(|b| b.iter().any(|s| stmt_uses_parallel_sum(s, host_only)))
        }
        Stmt::For {
            from, to, step, body, ..
        } => {
            expr_uses_parallel_sum(from, host_only)
                || expr_uses_parallel_sum(to, host_only)
                || step.as_ref().is_some_and(|s| expr_uses_parallel_sum(s, host_only))
                || body.iter().any(|s| stmt_uses_parallel_sum(s, host_only))
        }
        Stmt::ForEach { iter, body, .. } => {
            expr_uses_parallel_sum(iter, host_only)
                || body.iter().any(|s| stmt_uses_parallel_sum(s, host_only))
        }
        Stmt::DoLoop { cond, body } => {
            let in_cond = match cond {
                Some(
                    DoCond::PreWhile(c)
                    | DoCond::PreUntil(c)
                    | DoCond::PostWhile(c)
                    | DoCond::PostUntil(c),
                ) => expr_uses_parallel_sum(c, host_only),
                None => false,
            };
            in_cond || body.iter().any(|s| stmt_uses_parallel_sum(s, host_only))
        }
        Stmt::Match { scrutinee, arms, .. } => {
            expr_uses_parallel_sum(scrutinee, host_only)
                || arms.iter().any(|a| {
                    a.guard.as_ref().is_some_and(|g| expr_uses_parallel_sum(g, host_only))
                        || a.body.iter().any(|s| stmt_uses_parallel_sum(s, host_only))
                })
        }
        Stmt::HandleErr { call, body, target, .. } => {
            expr_uses_parallel_sum(call, host_only)
                || target.as_ref().is_some_and(|t| expr_uses_parallel_sum(t, host_only))
                || body.iter().any(|s| stmt_uses_parallel_sum(s, host_only))
        }
        Stmt::GpuInto { body, .. } => body.iter().any(|s| stmt_uses_parallel_sum(s, host_only)),
        _ => false,
    }
}

fn expr_uses_parallel_sum(e: &Expr, host_only: bool) -> bool {
    match &e.kind {
        ExprKind::ParallelSum(_, None) => true,
        ExprKind::ParallelSum(_, Some(_)) => !host_only,
        ExprKind::Binary { lhs, rhs, .. }
        | ExprKind::Index(lhs, rhs)
        | ExprKind::ListRepeat { value: lhs, count: rhs } => {
            expr_uses_parallel_sum(lhs, host_only) || expr_uses_parallel_sum(rhs, host_only)
        }
        ExprKind::MethodCall { recv, args, .. } => {
            expr_uses_parallel_sum(recv, host_only)
                || args.iter().any(|a| expr_uses_parallel_sum(a, host_only))
        }
        ExprKind::Call { args, .. } | ExprKind::Tuple(args) | ExprKind::List(args) => {
            args.iter().any(|a| expr_uses_parallel_sum(a, host_only))
        }
        ExprKind::StructLit { fields, .. } => {
            fields.iter().any(|(_, v)| expr_uses_parallel_sum(v, host_only))
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
        | ExprKind::Closure { body: inner, .. } => expr_uses_parallel_sum(inner, host_only),
        _ => false,
    }
}

/// Independence check for one `Parallel For`. Called after the body is
/// resolved, so indexes and types are in their final shape. `vars` is the
/// enclosing parallel variables, outermost first (`[y, x]` inside a nested
/// pair).
pub fn check(
    vars: &[String],
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
    match classify_nest(body) {
        Nest::Oblique => {
            diags.error(
                line,
                "A 2-D index space is `Parallel For y` wrapping `Parallel For x` \
                 — the outer body is just that inner loop. A `Parallel For` \
                 inside `If` or next to other statements isn't a grid.",
            );
            return;
        }
        Nest::TooDeep => {
            diags.error(
                line,
                "A 3-D index space (`Parallel For` three deep) isn't supported \
                 yet. Two loops — `Parallel For y` / `Parallel For x` — is the \
                 nest that flattens to one launch.",
            );
            return;
        }
        Nest::Grid2 { inner } => {
            if let Stmt::For { from, to, step, .. } = inner {
                if range_mentions(from, vars)
                    || range_mentions(to, vars)
                    || step.as_ref().is_some_and(|s| range_mentions(s, vars))
                {
                    diags.error(
                        line,
                        "The inner `Parallel For` range must be the same for \
                         every outer index — a rectangular grid. An inner bound \
                         that mentions the outer variable is a later slice.",
                    );
                    return;
                }
            }
            if body.iter().any(|s| stmt_uses_parallel_sum(s, false)) {
                diags.error(
                    line,
                    "`Parallel Sum` inside `Parallel For` would start more threads from \
                     each iteration. Sum outside the loop, or write `dest[i]` inside it.",
                );
            }
            return;
        }
        Nest::Flat => {}
    }
    if body.iter().any(|s| stmt_uses_parallel_sum(s, false)) {
        diags.error(
            line,
            "`Parallel Sum` inside `Parallel For` would start more threads from \
             each iteration. Sum outside the loop, or write `dest[i]` inside it.",
        );
        return;
    }

    let mut locals = HashSet::new();
    collect_locals(body, &mut locals);

    let mut ok = true;
    if !check_control(body, line, diags) {
        ok = false;
    }
    if !check_writes(vars, body, &locals, line, diags) {
        ok = false;
    }
    if !ok {
        return;
    }

    let written = written_arrays(vars, body, &locals);
    check_reads(vars, body, &written, line, diags);
    check_mutating_calls(body, &locals, line, diags);
}

/// The inner `Parallel For` when `body` is a 2-D nest (`Parallel For y` wrapping
/// exactly one `Parallel For x`).
pub fn nested_grid2(body: &[Stmt]) -> Option<&Stmt> {
    match classify_nest(body) {
        Nest::Grid2 { inner } => Some(inner),
        _ => None,
    }
}

enum Nest<'a> {
    /// No nested `Parallel For` — a 1-D index space (maybe with a sequential inner `For`).
    Flat,
    /// Outer body is exactly one inner `Parallel For` whose body is itself flat.
    Grid2 { inner: &'a Stmt },
    /// Three `Parallel For`s deep.
    TooDeep,
    /// Nested `Parallel For` that isn't a wrapping pair (inside `If`, extra statements…).
    Oblique,
}

fn classify_nest(body: &[Stmt]) -> Nest<'_> {
    let real: Vec<&Stmt> = body
        .iter()
        .filter(|s| !matches!(s, Stmt::LineMark(_) | Stmt::Comment(_)))
        .collect();
    let parallels: Vec<&Stmt> = real.iter().copied().filter(|s| is_parallel_for(s)).collect();
    let buried = real
        .iter()
        .any(|s| !is_parallel_for(s) && has_nested_parallel(std::slice::from_ref(s)));
    if buried {
        return Nest::Oblique;
    }
    match parallels.as_slice() {
        [] => Nest::Flat,
        [inner] if real.len() == 1 => {
            let Stmt::For { body: inner_body, .. } = inner else {
                unreachable!()
            };
            match classify_nest(inner_body) {
                Nest::Flat => Nest::Grid2 { inner },
                Nest::Grid2 { .. } | Nest::TooDeep => Nest::TooDeep,
                Nest::Oblique => Nest::Oblique,
            }
        }
        _ => Nest::Oblique,
    }
}

fn is_parallel_for(s: &Stmt) -> bool {
    matches!(s, Stmt::For { parallel: true, .. })
}

/// Names of non-local arrays this loop writes at a covering index — the Rust
/// emitter takes raw pointers to these.
pub fn written_arrays(vars: &[String], body: &[Stmt], locals: &HashSet<String>) -> HashSet<String> {
    let mut out = HashSet::new();
    collect_written(vars, body, locals, &mut out);
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
    vars: &[String],
    stmts: &[Stmt],
    locals: &HashSet<String>,
    line: usize,
    diags: &mut Diagnostics,
) -> bool {
    for s in stmts {
        match s {
            Stmt::Assign { target, .. } => {
                if !check_write_target(vars, target, locals, line, diags) {
                    return false;
                }
            }
            Stmt::Set { name, .. } => {
                if !is_local(name, locals) {
                    diags.error(
                        line,
                        format!(
                            "`Parallel For` can't `Set {name}` — that's a shared \
                             write. Each iteration may write `{}` of a \
                             different slot, not a scalar.",
                            slot_pattern(vars)
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
                if !check_write_target(vars, t, locals, line, diags) {
                    return false;
                }
                if !check_writes(vars, body, locals, line, diags) {
                    return false;
                }
            }
            Stmt::HandleErr { body, .. }
            | Stmt::For { body, .. }
            | Stmt::ForEach { body, .. }
            | Stmt::DoLoop { body, .. }
            | Stmt::GpuInto { body, .. } => {
                if !check_writes(vars, body, locals, line, diags) {
                    return false;
                }
            }
            Stmt::If { branches, else_body } => {
                for (_, b) in branches {
                    if !check_writes(vars, b, locals, line, diags) {
                        return false;
                    }
                }
                if let Some(b) = else_body {
                    if !check_writes(vars, b, locals, line, diags) {
                        return false;
                    }
                }
            }
            Stmt::Match { arms, .. } => {
                for a in arms {
                    if !check_writes(vars, &a.body, locals, line, diags) {
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
    vars: &[String],
    target: &Expr,
    locals: &HashSet<String>,
    line: usize,
    diags: &mut Diagnostics,
) -> bool {
    let slots = slot_pattern(vars);
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
                         (`{slots} = …`). A reduction (`total = total + …`) \
                         wants `Parallel Sum xs`, not a shared scalar."
                    ),
                );
                false
            }
        }
        ExprKind::Index(..) => {
            let Some((arr, idxs)) = index_chain(t) else {
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
            if !covers_vars(&idxs, vars) {
                diags.error(
                    line,
                    format!(
                        "`Parallel For` can only write `{slots}` — every parallel \
                         index must appear as a bare name (`{arr}[y][x]`, not \
                         `{arr}[y + 1]`). Two iterations writing the same slot \
                         would race."
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
                     Write `{slots}` of a Vec instead."
                ),
            );
            false
        }
        _ => {
            diags.error(
                line,
                format!(
                    "`Parallel For` writes must be `{slots}` (a different \
                     slot per iteration) or a `Dim` local to the body."
                ),
            );
            false
        }
    }
}

fn collect_written(
    vars: &[String],
    stmts: &[Stmt],
    locals: &HashSet<String>,
    out: &mut HashSet<String>,
) {
    for s in stmts {
        match s {
            Stmt::Assign { target, .. } => {
                if let Some((arr, idxs)) = index_chain(peel(target)) {
                    if covers_vars(&idxs, vars) && !is_local(&arr, locals) {
                        out.insert(arr);
                    }
                }
            }
            Stmt::HandleErr { target, body, .. } => {
                if let Some(target) = target {
                    if let Some((arr, idxs)) = index_chain(peel(target)) {
                        if covers_vars(&idxs, vars) && !is_local(&arr, locals) {
                            out.insert(arr);
                        }
                    }
                }
                collect_written(vars, body, locals, out);
            }
            Stmt::For { body, .. }
            | Stmt::ForEach { body, .. }
            | Stmt::DoLoop { body, .. }
            | Stmt::GpuInto { body, .. } => collect_written(vars, body, locals, out),
            Stmt::If { branches, else_body } => {
                for (_, b) in branches {
                    collect_written(vars, b, locals, out);
                }
                if let Some(b) = else_body {
                    collect_written(vars, b, locals, out);
                }
            }
            Stmt::Match { arms, .. } => {
                for a in arms {
                    collect_written(vars, &a.body, locals, out);
                }
            }
            _ => {}
        }
    }
}

fn check_reads(
    vars: &[String],
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
            | Stmt::Assert(e) => check_read_expr(vars, e, written, line, diags),
            Stmt::Assign { target, value, .. } => {
                check_read_expr(vars, target, written, line, diags);
                check_read_expr(vars, value, written, line, diags);
            }
            Stmt::If { branches, else_body } => {
                for (c, b) in branches {
                    check_read_expr(vars, c, written, line, diags);
                    check_reads(vars, b, written, line, diags);
                }
                if let Some(b) = else_body {
                    check_reads(vars, b, written, line, diags);
                }
            }
            Stmt::For { from, to, step, body, .. } => {
                check_read_expr(vars, from, written, line, diags);
                check_read_expr(vars, to, written, line, diags);
                if let Some(st) = step {
                    check_read_expr(vars, st, written, line, diags);
                }
                check_reads(vars, body, written, line, diags);
            }
            Stmt::ForEach { iter, body, .. } => {
                if let Some(arr) = ident_of(iter) {
                    if written.contains(&arr) {
                        let slots = slot_pattern(vars);
                        diags.error(
                            line,
                            format!(
                                "`For Each` over `{arr}` isn't allowed inside `Parallel For` \
                                 while `{arr}` is also being written — that's a conflicting \
                                 whole-array read. Index `{slots}` instead."
                            ),
                        );
                    }
                }
                check_read_expr(vars, iter, written, line, diags);
                check_reads(vars, body, written, line, diags);
            }
            Stmt::DoLoop { cond, body } => {
                if let Some(
                    DoCond::PreWhile(c)
                    | DoCond::PreUntil(c)
                    | DoCond::PostWhile(c)
                    | DoCond::PostUntil(c),
                ) = cond
                {
                    check_read_expr(vars, c, written, line, diags);
                }
                check_reads(vars, body, written, line, diags);
            }
            Stmt::Match { scrutinee, arms, .. } => {
                check_read_expr(vars, scrutinee, written, line, diags);
                for a in arms {
                    if let Some(g) = &a.guard {
                        check_read_expr(vars, g, written, line, diags);
                    }
                    check_reads(vars, &a.body, written, line, diags);
                }
            }
            Stmt::HandleErr { target, call, body, .. } => {
                if let Some(t) = target {
                    check_read_expr(vars, t, written, line, diags);
                }
                check_read_expr(vars, call, written, line, diags);
                check_reads(vars, body, written, line, diags);
            }
            Stmt::GpuInto { body, .. } => check_reads(vars, body, written, line, diags),
            _ => {}
        }
    }
}

fn check_read_expr(
    vars: &[String],
    e: &Expr,
    written: &HashSet<String>,
    line: usize,
    diags: &mut Diagnostics,
) {
    match &e.kind {
        ExprKind::Index(inner, idx) => {
            if let Some((arr, idxs)) = index_chain(e) {
                if written.contains(&arr) && !covers_vars(&idxs, vars) {
                    let slots = slot_pattern(vars);
                    diags.error(
                        line,
                        format!(
                            "`{arr}` is written inside this `Parallel For`, so a read \
                             must be `{slots}` too — a neighbour index would race \
                             with another iteration's write. Stencil a *different* \
                             array (`in[y + 1][x]` while writing `out[y][x]`)."
                        ),
                    );
                }
                for i in idxs {
                    check_read_expr(vars, i, written, line, diags);
                }
            } else {
                check_read_expr(vars, inner, written, line, diags);
                check_read_expr(vars, idx, written, line, diags);
            }
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            check_read_expr(vars, lhs, written, line, diags);
            check_read_expr(vars, rhs, written, line, diags);
        }
        ExprKind::MethodCall { recv, method, args } => {
            check_method_on_written(vars, recv, method, args, written, line, diags);
            check_read_expr(vars, recv, written, line, diags);
            for a in args {
                check_read_expr(vars, a, written, line, diags);
            }
        }
        ExprKind::Call { args, .. } | ExprKind::Tuple(args) | ExprKind::List(args) => {
            for a in args {
                check_read_expr(vars, a, written, line, diags);
            }
        }
        ExprKind::ListRepeat { value, count } => {
            check_read_expr(vars, value, written, line, diags);
            check_read_expr(vars, count, written, line, diags);
        }
        ExprKind::StructLit { fields, .. } => {
            for (_, v) in fields {
                check_read_expr(vars, v, written, line, diags);
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
        | ExprKind::ParallelSum(inner, _)
        | ExprKind::TupleIndex(inner, _)
        | ExprKind::Closure { body: inner, .. } => {
            check_read_expr(vars, inner, written, line, diags);
        }
        _ => {}
    }
}

fn check_method_on_written(
    vars: &[String],
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
    if m == "get" && args.len() == 1 && vars.len() == 1 && is_loop_var(&args[0], &vars[0]) {
        return;
    }
    let slots = slot_pattern(vars);
    diags.error(
        line,
        format!(
            "`{arr}.{method}(…)` reads or mutates a Vec this `Parallel For` is \
             writing. Index `{slots}` instead."
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
        ExprKind::Binary { lhs, rhs, .. }
        | ExprKind::Index(lhs, rhs)
        | ExprKind::ListRepeat { value: lhs, count: rhs } => {
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
        | ExprKind::ParallelSum(inner, _)
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

pub(crate) fn index_chain(e: &Expr) -> Option<(String, Vec<&Expr>)> {
    let mut idxs = Vec::new();
    let mut cur = peel(e);
    loop {
        match &cur.kind {
            ExprKind::Index(inner, idx) => {
                idxs.push(idx.as_ref());
                cur = peel(inner);
            }
            ExprKind::Ident(n) => {
                idxs.reverse();
                return Some((n.to_ascii_lowercase(), idxs));
            }
            _ => return None,
        }
    }
}

fn covers_vars(idxs: &[&Expr], vars: &[String]) -> bool {
    vars.iter().all(|v| idxs.iter().any(|i| is_loop_var(i, v)))
}

fn slot_pattern(vars: &[String]) -> String {
    let mut s = "arr".to_string();
    for v in vars {
        s.push_str(&format!("[{v}]"));
    }
    s
}

fn range_mentions(e: &Expr, names: &[String]) -> bool {
    match &e.kind {
        ExprKind::Ident(n) => names.iter().any(|v| n.eq_ignore_ascii_case(v)),
        ExprKind::Binary { lhs, rhs, .. }
        | ExprKind::Index(lhs, rhs)
        | ExprKind::ListRepeat { value: lhs, count: rhs } => {
            range_mentions(lhs, names) || range_mentions(rhs, names)
        }
        ExprKind::MethodCall { recv, args, .. } => {
            range_mentions(recv, names) || args.iter().any(|a| range_mentions(a, names))
        }
        ExprKind::Call { args, .. } | ExprKind::Tuple(args) | ExprKind::List(args) => {
            args.iter().any(|a| range_mentions(a, names))
        }
        ExprKind::Field(inner, _)
        | ExprKind::Cast(inner, _)
        | ExprKind::Not(inner)
        | ExprKind::Deref(inner)
        | ExprKind::Ref(inner)
        | ExprKind::MutRef(inner)
        | ExprKind::Try(inner)
        | ExprKind::Raw(inner)
        | ExprKind::Await(inner)
        | ExprKind::ParallelSum(inner, _)
        | ExprKind::TupleIndex(inner, _) => range_mentions(inner, names),
        _ => false,
    }
}
