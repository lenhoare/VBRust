//! Type-resolution pass.
//!
//! Builds a table of user-function signatures and a per-function variable-type
//! environment, then rewrites each function body so calling conventions and
//! number-type conversions become correct Rust:
//!
//!   * a `ByRef` argument at a call site is wrapped in `&mut` (`MutRef`),
//!   * every use of a `ByRef` parameter inside the body is dereferenced (`Deref`),
//!   * a numeric value assigned to a different numeric type is wrapped in an
//!     `as` cast (`Cast`) — VB converts silently, Rust wants it spelled out,
//!   * locals passed to a `ByRef` parameter are reported so they become `let mut`.
//!
//! Doing this as AST rewrites keeps the rendering code a plain tree walk.

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diagnostics::Diagnostics;
use crate::transpiler::to_snake as snake;

/// One user function's signature — enough to fix up its call sites.
pub struct FnSig {
    pub modes: Vec<ParamMode>,
    pub ret: Option<RetType>,
}

pub type FnTable = HashMap<String, FnSig>;

/// Map every user function (by its Rust snake_case name) to its signature.
pub fn build_fn_table(program: &Program) -> FnTable {
    program
        .functions
        .iter()
        .map(|f| {
            (
                snake(&f.name),
                FnSig {
                    modes: f.params.iter().map(|p| p.mode).collect(),
                    ret: f.ret,
                },
            )
        })
        .collect()
}

/// A resolved expression type — a superset of `ast::Type` (it can also be
/// `usize`, a string slice, or simply unknown).
#[derive(Clone, Copy, PartialEq)]
enum RType {
    I16,
    I32,
    I64,
    U8,
    Usize,
    F32,
    F64,
    Bool,
    Str,
    Strng,
    Unknown,
}

impl RType {
    fn is_numeric(self) -> bool {
        matches!(
            self,
            RType::I16 | RType::I32 | RType::I64 | RType::U8 | RType::Usize | RType::F32 | RType::F64
        )
    }

    fn is_float(self) -> bool {
        matches!(self, RType::F32 | RType::F64)
    }
}

fn rtype_of(ty: Type) -> RType {
    match ty {
        Type::Integer => RType::I16,
        Type::Long => RType::I32,
        Type::LongLong => RType::I64,
        Type::Single => RType::F32,
        Type::Double => RType::F64,
        Type::Boolean => RType::Bool,
        Type::Byte => RType::U8,
        Type::Date => RType::I64,
        Type::Text => RType::Strng,
    }
}

/// Result type of a known builtin, used so e.g. `Len` into a `Long` casts.
fn builtin_rtype(name: &str) -> Option<RType> {
    Some(match name.to_ascii_lowercase().as_str() {
        "len" => RType::Usize,
        "left" | "right" | "mid" | "trim" => RType::Str,
        "ucase" | "lcase" | "replace" | "str" => RType::Strng,
        "sqr" | "abs" | "int" | "round" | "sin" | "cos" | "tan" | "log" | "exp" => RType::F64,
        // instr → Option, val → Result: not a plain value type yet.
        _ => return None,
    })
}

/// Rewrite a function body in place. Returns the set of *locals* passed by
/// `ByRef` to some call (they must be declared `mut`).
pub fn resolve_body(
    stmts: &mut [Stmt],
    params: &[Param],
    fns: &FnTable,
    diags: &mut Diagnostics,
) -> HashSet<String> {
    let byref: HashSet<String> = params
        .iter()
        .filter(|p| p.mode == ParamMode::ByRef)
        .map(|p| snake(&p.name))
        .collect();

    // Variable types in scope. VB has no block scope, so a flat map matches the
    // mental model; a ByRef parameter is stored as its pointee type.
    let mut vars: HashMap<String, Type> =
        params.iter().map(|p| (snake(&p.name), p.ty)).collect();

    let mut passed = HashSet::new();
    let mut ctx = Ctx {
        deref: byref,
        fns,
        diags,
        vars: &mut vars,
        passed: &mut passed,
    };
    resolve_stmts(stmts, &mut ctx);
    passed
}

struct Ctx<'a> {
    /// Names whose uses are references and must be dereferenced: ByRef params
    /// plus the (scoped) variables of an enclosing `For Each`.
    deref: HashSet<String>,
    fns: &'a FnTable,
    diags: &'a mut Diagnostics,
    vars: &'a mut HashMap<String, Type>,
    passed: &'a mut HashSet<String>,
}

fn resolve_stmts(stmts: &mut [Stmt], ctx: &mut Ctx) {
    for stmt in stmts {
        match stmt {
            Stmt::Dim { name, ty, init, .. } => {
                // Only plain scalars take part in numeric coercion / inference.
                if let DeclType::Plain(t) = ty {
                    if let Some(e) = init {
                        resolve_expr(e, ctx);
                        maybe_cast(e, *t, ctx);
                    }
                    ctx.vars.insert(snake(name), *t);
                } else if let Some(e) = init {
                    resolve_expr(e, ctx);
                }
            }
            Stmt::Assign { name, value } => {
                resolve_expr(value, ctx);
                if let Some(ty) = ctx.vars.get(&snake(name)).copied() {
                    maybe_cast(value, ty, ctx);
                }
            }
            Stmt::Set { value, .. } => resolve_expr(value, ctx),
            Stmt::Return(Some(e)) | Stmt::Expr(e) | Stmt::Print(e) => resolve_expr(e, ctx),
            Stmt::Return(None) | Stmt::Comment(_) => {}
            Stmt::If { branches, else_body } => {
                for (cond, body) in branches {
                    resolve_expr(cond, ctx);
                    resolve_stmts(body, ctx);
                }
                if let Some(body) = else_body {
                    resolve_stmts(body, ctx);
                }
            }
            Stmt::For { var, from, to, step, body } => {
                resolve_expr(from, ctx);
                resolve_expr(to, ctx);
                if let Some(s) = step {
                    resolve_expr(s, ctx);
                }
                // The loop variable is an integer in scope for the body.
                ctx.vars.insert(snake(var), Type::Long);
                resolve_stmts(body, ctx);
            }
            Stmt::ForEach { var1, var2, iter, body } => {
                resolve_expr(iter, ctx);
                // Loop variables are references inside the body, so their uses
                // get dereferenced — scoped to this loop only.
                let v1 = snake(var1);
                let added1 = ctx.deref.insert(v1.clone());
                let v2 = var2.as_ref().map(|v| {
                    let s = snake(v);
                    let added = ctx.deref.insert(s.clone());
                    (s, added)
                });
                resolve_stmts(body, ctx);
                if added1 {
                    ctx.deref.remove(&v1);
                }
                if let Some((s, true)) = v2 {
                    ctx.deref.remove(&s);
                }
            }
            Stmt::Select { scrutinee, arms, else_body, .. } => {
                resolve_expr(scrutinee, ctx);
                for arm in arms.iter_mut() {
                    for pat in arm.patterns.iter_mut() {
                        match pat {
                            CasePattern::Value(e) => resolve_expr(e, ctx),
                            CasePattern::Range(lo, hi) => {
                                resolve_expr(lo, ctx);
                                resolve_expr(hi, ctx);
                            }
                        }
                    }
                    resolve_stmts(&mut arm.body, ctx);
                }
                if let Some(body) = else_body {
                    resolve_stmts(body, ctx);
                }
            }
        }
    }
}

/// Insert a numeric `as` cast if `value`'s type differs from `target`. Literals
/// are left alone — the renderer already adapts them to their context.
fn maybe_cast(value: &mut Expr, target: Type, ctx: &mut Ctx) {
    if matches!(value, Expr::Int(_) | Expr::Float(_)) {
        return;
    }
    let target_rt = rtype_of(target);
    if !target_rt.is_numeric() {
        return;
    }
    let src = infer(value, ctx);
    if src.is_numeric() && src != target_rt {
        ctx.diags.note(
            "numeric-cast",
            "VB converts between number types silently; Rust wants it spelled out, so VBR \
             inserts `as` for you. A narrowing conversion (e.g. Long → Integer, or a float \
             to an integer) can lose data.",
        );
        let inner = std::mem::replace(value, Expr::Int(0));
        *value = Expr::Cast(Box::new(inner), target);
    }
}

fn resolve_expr(e: &mut Expr, ctx: &mut Ctx) {
    match e {
        Expr::Ident(name) if ctx.deref.contains(&snake(name)) => {
            *e = Expr::Deref(Box::new(Expr::Ident(name.clone())));
        }
        Expr::Ident(_) => {}
        Expr::Binary { lhs, rhs, .. } => {
            resolve_expr(lhs, ctx);
            resolve_expr(rhs, ctx);
        }
        Expr::MethodCall { recv, args, .. } => {
            resolve_expr(recv, ctx);
            for a in args.iter_mut() {
                resolve_expr(a, ctx);
            }
        }
        Expr::Call { name, args } => {
            for a in args.iter_mut() {
                resolve_expr(a, ctx);
            }
            if let Some(sig) = ctx.fns.get(&snake(name)) {
                for (i, arg) in args.iter_mut().enumerate() {
                    if sig.modes.get(i) == Some(&ParamMode::ByRef) {
                        if let Expr::Ident(v) = arg {
                            ctx.passed.insert(snake(v));
                        }
                        let inner = std::mem::replace(arg, Expr::Int(0));
                        *arg = Expr::MutRef(Box::new(inner));
                    }
                }
            }
        }
        Expr::Deref(inner) | Expr::MutRef(inner) | Expr::Cast(inner, _) | Expr::Try(inner) => {
            resolve_expr(inner, ctx)
        }
        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Str(_) => {}
    }
}

/// Best-effort type inference. `Unknown` whenever we can't be sure — callers
/// only act on a confidently numeric result.
fn infer(e: &Expr, ctx: &Ctx) -> RType {
    match e {
        Expr::Int(_) => RType::I32,
        Expr::Float(_) => RType::F64,
        Expr::Bool(_) => RType::Bool,
        Expr::Str(_) => RType::Str,
        Expr::Ident(name) => ctx.vars.get(&snake(name)).copied().map_or(RType::Unknown, rtype_of),
        Expr::Deref(inner) => infer(inner, ctx),
        Expr::Cast(_, ty) => rtype_of(*ty),
        // `?` unwraps a Result/Option to its payload; we don't track that yet.
        Expr::Try(_) => RType::Unknown,
        Expr::MutRef(_) => RType::Unknown,
        Expr::Binary { op, lhs, rhs } => match op {
            BinOp::Concat => RType::Strng,
            BinOp::Pow => RType::F64,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => RType::Bool,
            _ => join(infer(lhs, ctx), infer(rhs, ctx)),
        },
        Expr::Call { name, args } => builtin_rtype(name).unwrap_or_else(|| {
            // Not a builtin? A plain return type is numeric; Result/Option aren't.
            let _ = args;
            match ctx.fns.get(&snake(name)).and_then(|s| s.ret) {
                Some(RetType::Plain(t)) => rtype_of(t),
                _ => RType::Unknown,
            }
        }),
        Expr::MethodCall { .. } => RType::Unknown,
    }
}

/// The result type of arithmetic between two operands (a rough widening).
fn join(a: RType, b: RType) -> RType {
    if a == RType::Unknown || b == RType::Unknown {
        return RType::Unknown;
    }
    if a.is_float() || b.is_float() {
        if a == RType::F64 || b == RType::F64 {
            RType::F64
        } else {
            RType::F32
        }
    } else if a == RType::I64 || b == RType::I64 {
        RType::I64
    } else if a == RType::Usize || b == RType::Usize {
        RType::Usize
    } else {
        RType::I32
    }
}
