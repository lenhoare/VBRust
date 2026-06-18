//! Type-resolution pass (first installment).
//!
//! Builds a table of user-function signatures, then rewrites each function body
//! so that calling conventions become correct Rust:
//!
//!   * a `ByRef` argument at a call site is wrapped in `&mut` (`MutRef`),
//!   * every use of a `ByRef` parameter inside the body is dereferenced (`Deref`),
//!   * locals passed to a `ByRef` parameter are reported so they become `let mut`.
//!
//! Doing this as an AST rewrite keeps the rendering code simple: it just learns
//! two new node kinds and otherwise stays a plain tree walk.

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::transpiler::to_snake as snake;

/// One user function's signature — enough to fix up its call sites.
pub struct FnSig {
    pub modes: Vec<ParamMode>,
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
                },
            )
        })
        .collect()
}

/// Rewrite a function body in place. `byref` is the set of this function's
/// `ByRef` parameter names; returns the set of *locals* passed by `ByRef` to
/// some call (they must be declared `mut`).
pub fn resolve_body(stmts: &mut [Stmt], byref: &HashSet<String>, fns: &FnTable) -> HashSet<String> {
    let mut passed = HashSet::new();
    resolve_stmts(stmts, byref, fns, &mut passed);
    passed
}

fn resolve_stmts(stmts: &mut [Stmt], byref: &HashSet<String>, fns: &FnTable, passed: &mut HashSet<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::Dim { init: Some(e), .. } => resolve_expr(e, byref, fns, passed),
            Stmt::Dim { init: None, .. } => {}
            Stmt::Set { value, .. } | Stmt::Assign { value, .. } => {
                resolve_expr(value, byref, fns, passed)
            }
            Stmt::Return(Some(e)) | Stmt::Expr(e) | Stmt::Print(e) => {
                resolve_expr(e, byref, fns, passed)
            }
            Stmt::Return(None) | Stmt::Comment(_) => {}
            Stmt::If { branches, else_body } => {
                for (cond, body) in branches {
                    resolve_expr(cond, byref, fns, passed);
                    resolve_stmts(body, byref, fns, passed);
                }
                if let Some(body) = else_body {
                    resolve_stmts(body, byref, fns, passed);
                }
            }
            Stmt::For { from, to, step, body, .. } => {
                resolve_expr(from, byref, fns, passed);
                resolve_expr(to, byref, fns, passed);
                if let Some(s) = step {
                    resolve_expr(s, byref, fns, passed);
                }
                resolve_stmts(body, byref, fns, passed);
            }
            Stmt::Select { scrutinee, arms, else_body, .. } => {
                resolve_expr(scrutinee, byref, fns, passed);
                for arm in arms.iter_mut() {
                    for pat in arm.patterns.iter_mut() {
                        match pat {
                            CasePattern::Value(e) => resolve_expr(e, byref, fns, passed),
                            CasePattern::Range(lo, hi) => {
                                resolve_expr(lo, byref, fns, passed);
                                resolve_expr(hi, byref, fns, passed);
                            }
                        }
                    }
                    resolve_stmts(&mut arm.body, byref, fns, passed);
                }
                if let Some(body) = else_body {
                    resolve_stmts(body, byref, fns, passed);
                }
            }
        }
    }
}

fn resolve_expr(e: &mut Expr, byref: &HashSet<String>, fns: &FnTable, passed: &mut HashSet<String>) {
    match e {
        Expr::Ident(name) if byref.contains(&snake(name)) => {
            // A use of a ByRef parameter — dereference it.
            *e = Expr::Deref(Box::new(Expr::Ident(name.clone())));
        }
        Expr::Ident(_) => {}
        Expr::Binary { lhs, rhs, .. } => {
            resolve_expr(lhs, byref, fns, passed);
            resolve_expr(rhs, byref, fns, passed);
        }
        Expr::MethodCall { recv, args, .. } => {
            resolve_expr(recv, byref, fns, passed);
            for a in args.iter_mut() {
                resolve_expr(a, byref, fns, passed);
            }
        }
        Expr::Call { name, args } => {
            for a in args.iter_mut() {
                resolve_expr(a, byref, fns, passed);
            }
            if let Some(sig) = fns.get(&snake(name)) {
                for (i, arg) in args.iter_mut().enumerate() {
                    if sig.modes.get(i) == Some(&ParamMode::ByRef) {
                        // A plain local being lent mutably must be declared `mut`.
                        if let Expr::Ident(v) = arg {
                            passed.insert(snake(v));
                        }
                        let inner = std::mem::replace(arg, Expr::Int(0));
                        *arg = Expr::MutRef(Box::new(inner));
                    }
                }
            }
        }
        Expr::Deref(inner) | Expr::MutRef(inner) => resolve_expr(inner, byref, fns, passed),
        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Str(_) => {}
    }
}
