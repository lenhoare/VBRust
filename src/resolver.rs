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
use crate::transpiler::{stdlib_type, to_screaming};

/// One user function's signature — enough to fix up its call sites.
pub struct FnSig {
    pub modes: Vec<ParamMode>,
    pub param_types: Vec<DeclType>,
    pub ret: Option<DeclType>,
}

pub type FnTable = HashMap<String, FnSig>;

/// `(struct name, method snake name)` → does it take `&mut self`?
pub type MethodTable = HashMap<(String, String), bool>;

/// Module-constant original name → its SCREAMING_SNAKE_CASE Rust name.
pub fn build_const_map(program: &Program) -> HashMap<String, String> {
    program
        .constants
        .iter()
        .map(|c| (c.name.clone(), to_screaming(&c.name)))
        .collect()
}

/// Map each method to whether it mutates `self` (assigns to a `Me` field).
pub fn build_method_table(program: &Program) -> MethodTable {
    program
        .functions
        .iter()
        .filter_map(|f| {
            f.receiver
                .as_ref()
                .map(|recv| ((recv.clone(), snake(&f.name)), method_mutates_self(&f.body)))
        })
        .collect()
}

/// Does this method body assign to a `Me` field (so it needs `&mut self`)?
pub fn method_mutates_self(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::Assign { target, .. } => is_me_field(target),
        Stmt::If { branches, else_body } => {
            branches.iter().any(|(_, b)| method_mutates_self(b))
                || else_body.as_ref().map_or(false, |b| method_mutates_self(b))
        }
        Stmt::For { body, .. }
        | Stmt::ForEach { body, .. }
        | Stmt::DoLoop { body, .. } => method_mutates_self(body),
        Stmt::Select { arms, else_body, .. } => {
            arms.iter().any(|a| method_mutates_self(&a.body))
                || else_body.as_ref().map_or(false, |b| method_mutates_self(b))
        }
        _ => false,
    })
}

fn is_me_field(e: &Expr) -> bool {
    match e {
        Expr::Field(inner, _) => matches!(&**inner, Expr::Ident(n) if n == "Me") || is_me_field(inner),
        _ => false,
    }
}

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
                    param_types: f.params.iter().map(|p| p.ty.clone()).collect(),
                    ret: f.ret.clone(),
                },
            )
        })
        .collect()
}

/// A resolved expression type — a superset of `ast::Type` (it can also be
/// `usize`, a string slice, or simply unknown).
#[derive(Clone, Copy, PartialEq)]
enum RType {
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
            RType::I32 | RType::I64 | RType::U8 | RType::Usize | RType::F32 | RType::F64
        )
    }

    fn is_float(self) -> bool {
        matches!(self, RType::F32 | RType::F64)
    }
}

/// Wrap a `&str`-typed expression in `.to_string()`, making it an owned String.
/// Used wherever a String is expected: Dim init, assignment, return, Ok/Some.
fn to_owned_string(e: &mut Expr) {
    let inner = std::mem::replace(e, Expr::Int(0));
    *e = Expr::MethodCall {
        recv: Box::new(inner),
        method: "to_string".to_string(),
        args: Vec::new(),
    };
}

/// The VB type for an inferred numeric `RType` (for inserting `as` casts).
/// `Usize`/`Str`/`Bool`/`Unknown` have no VB-type target.
fn rtype_to_type(rt: RType) -> Option<Type> {
    Some(match rt {
        RType::I32 => Type::Integer,
        RType::I64 => Type::Long,
        RType::U8 => Type::Byte,
        RType::F32 => Type::Single,
        RType::F64 => Type::Double,
        _ => return None,
    })
}

fn rtype_of(ty: Type) -> RType {
    match ty {
        Type::Integer => RType::I32,
        Type::Long => RType::I64,
        Type::LongLong => RType::I64,
        Type::Single => RType::F32,
        Type::Double => RType::F64,
        Type::Boolean => RType::Bool,
        Type::Byte => RType::U8,
        Type::Text => RType::Strng,
    }
}

/// Result type of a known builtin, used so e.g. `Len` into a `Long` casts.
fn builtin_rtype(name: &str) -> Option<RType> {
    Some(match name.to_ascii_lowercase().as_str() {
        "len" => RType::Usize,
        "left" | "right" | "mid" | "trim" => RType::Str,
        "ucase" | "lcase" | "replace" | "str" | "inputbox" => RType::Strng,
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
    methods: &MethodTable,
    consts: &HashMap<String, String>,
    modules: &HashSet<String>,
    ret_coerce: Option<Type>,
    can_propagate: bool,
    diags: &mut Diagnostics,
) -> HashSet<String> {
    // Only ByRef *primitive* params are dereferenced — struct/collection field
    // and method access auto-derefs, so those don't need an explicit `*`.
    let byref: HashSet<String> = params
        .iter()
        .filter(|p| p.mode == ParamMode::ByRef && matches!(p.ty, DeclType::Plain(_)))
        .map(|p| snake(&p.name))
        .collect();

    // Variable types in scope. VB has no block scope, so a flat map matches the
    // mental model.
    let mut vars: HashMap<String, Type> = HashMap::new();
    let mut struct_vars = HashMap::new();
    let mut array_vars = HashSet::new();
    let mut str_params = HashSet::new();
    // ByVal collection params are passed as `&Vec`/`&HashMap`, so they're already
    // a reference — `For Each` must not borrow them a second time.
    let mut borrowed_collections = HashSet::new();
    for p in params {
        match &p.ty {
            // A ByVal String parameter is a `&str` (already a slice).
            DeclType::Plain(Type::Text) if p.mode == ParamMode::ByVal => {
                str_params.insert(snake(&p.name));
            }
            DeclType::Plain(t) => {
                vars.insert(snake(&p.name), *t);
            }
            DeclType::Named(n) => {
                struct_vars.insert(snake(&p.name), n.clone());
            }
            DeclType::Vec(_) | DeclType::Map(..) if p.mode == ParamMode::ByVal => {
                array_vars.insert(snake(&p.name));
                borrowed_collections.insert(snake(&p.name));
            }
            DeclType::Vec(_) | DeclType::Array(..) | DeclType::Array2D(..) => {
                array_vars.insert(snake(&p.name));
            }
            _ => {}
        }
    }
    let mut passed = HashSet::new();
    let mut handles = HashSet::new();
    let mut ctx = Ctx {
        deref: byref,
        fns,
        methods,
        consts,
        ret_coerce,
        can_propagate,
        diags,
        vars: &mut vars,
        struct_vars: &mut struct_vars,
        array_vars: &mut array_vars,
        str_params: &mut str_params,
        passed: &mut passed,
        handles: &mut handles,
        borrowed_collections: &mut borrowed_collections,
        modules,
    };
    resolve_stmts(stmts, &mut ctx);
    passed
}

struct Ctx<'a> {
    /// Names whose uses are references and must be dereferenced: ByRef params
    /// plus the (scoped) variables of an enclosing `For Each`.
    deref: HashSet<String>,
    fns: &'a FnTable,
    methods: &'a MethodTable,
    consts: &'a HashMap<String, String>,
    /// The function's plain numeric return type, for coercing `Return` values.
    ret_coerce: Option<Type>,
    /// Whether the enclosing function returns `Result`/`Option` — i.e. whether
    /// `?` is allowed here.
    can_propagate: bool,
    diags: &'a mut Diagnostics,
    vars: &'a mut HashMap<String, Type>,
    /// Variable name → struct type, for receiver-mutation detection.
    struct_vars: &'a mut HashMap<String, String>,
    /// Array/Vec variable names — `x(i)` on one is a friendly error.
    array_vars: &'a mut HashSet<String>,
    /// ByVal String parameters — they are `&str`, so they don't get re-borrowed.
    str_params: &'a mut HashSet<String>,
    passed: &'a mut HashSet<String>,
    /// Opaque Rust handles (`Dim h = Rust …`). Their only legal use is being
    /// spliced into another inline-Rust block (which the resolver never sees as
    /// an AST ident), so *any* ident-use of one is a value-use error.
    handles: &'a mut HashSet<String>,
    /// ByVal collection params (already `&Vec`/`&HashMap`); `For Each` over one
    /// must reborrow (`&*p`) rather than double-borrow (`&&Vec`).
    borrowed_collections: &'a mut HashSet<String>,
    /// Other project modules (snake-cased file stems). A `Module.func(...)` call
    /// on one rewrites to a qualified `crate::module::func(...)`.
    modules: &'a HashSet<String>,
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
                        // A `&str` *expression* (Mid, a String param, Trim…) into a
                        // String var becomes owned. A bare literal is handled by the
                        // emitter; an owned-String move still hits the ownership error.
                        if *t == Type::Text && infer(e, ctx) == RType::Str && !matches!(e, Expr::Str(_)) {
                            to_owned_string(e);
                        }
                    }
                    ctx.vars.insert(snake(name), *t);
                } else if let DeclType::Named(struct_name) = ty {
                    if let Some(e) = init {
                        resolve_expr(e, ctx);
                    }
                    ctx.struct_vars.insert(snake(name), struct_name.clone());
                } else {
                    if let Some(e) = init {
                        // Tuple / collection — type not tracked numerically.
                        resolve_expr(e, ctx);
                    }
                    if matches!(
                        ty,
                        DeclType::Vec(_) | DeclType::Array(..) | DeclType::Array2D(..)
                    ) {
                        ctx.array_vars.insert(snake(name));
                    }
                }
            }
            Stmt::DestructureDim { value, .. } => resolve_expr(value, ctx),
            // The body is raw Rust (not an Expr), so there's nothing to resolve —
            // just record the name so later value-uses of it are caught.
            Stmt::HandleDim { name, .. } => {
                ctx.handles.insert(snake(name));
            }
            Stmt::Assign { target, value, .. } => {
                // Writing to a ByVal String parameter — it's a read-only `&str`.
                if let Expr::Ident(name) = &*target {
                    if ctx.str_params.contains(&snake(name)) {
                        ctx.diags.error_once(
                            &format!("byval-string-write-{}", snake(name)),
                            format!(
                                "'{}' is passed read-only (ByVal), so it can't be changed here. \
                                 To modify the caller's string, declare it `ByRef {} As String`.",
                                name, name
                            ),
                        );
                    }
                }
                // Coerce based on the target variable's type (plain Ident targets only).
                let target_ty = match &*target {
                    Expr::Ident(name) => ctx.vars.get(&snake(name)).copied(),
                    _ => None,
                };
                // A plain Ident target is dereferenced by the emitter (if ByRef);
                // resolve other targets (e.g. field accesses).
                if !matches!(&*target, Expr::Ident(_)) {
                    resolve_expr(target, ctx);
                }
                resolve_expr(value, ctx);
                if let Some(ty) = target_ty {
                    maybe_cast(value, ty, ctx);
                    // Assigning a `&str` (a literal like `""`, a param, Mid…) to a
                    // String variable → owned.
                    if ty == Type::Text && infer(value, ctx) == RType::Str {
                        to_owned_string(value);
                    }
                }
            }
            Stmt::Set { value, .. } => resolve_expr(value, ctx),
            Stmt::Print(e) => resolve_expr(e, ctx),
            Stmt::Return(Some(e)) => {
                resolve_expr(e, ctx);
                match ctx.ret_coerce {
                    // A String-returning function: an existing &str (literal, &str
                    // param, Trim(..)) becomes an owned String.
                    Some(Type::Text) if infer(e, ctx) == RType::Str => to_owned_string(e),
                    // Coerce a numeric return value to the declared numeric type.
                    Some(t) => maybe_cast(e, t, ctx),
                    None => {}
                }
            }
            Stmt::Expr(e) => {
                resolve_expr(e, ctx);
                // A bare call that yields a Result/Option must not be discarded.
                if let Some(kind) = ignored_result(e, ctx) {
                    ctx.diags.error_once(
                        "ignored-result",
                        format!(
                            "This {} is being thrown away. Handle it: `?` to propagate, \
                             `Select Case` over Ok/Err (or Some/None), or assign it with `Dim`.",
                            kind
                        ),
                    );
                }
            }
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
                // The loop variable is an `i32` in scope for the body — Rust infers
                // that from literal bounds (`for i in 1..=10`), so it must be
                // `Integer`, not `Long` (i64), to stay consistent.
                ctx.vars.insert(snake(var), Type::Integer);
                resolve_stmts(body, ctx);
            }
            Stmt::DoLoop { cond, body } => {
                if let Some(
                    DoCond::PreWhile(c)
                    | DoCond::PreUntil(c)
                    | DoCond::PostWhile(c)
                    | DoCond::PostUntil(c),
                ) = cond
                {
                    resolve_expr(c, ctx);
                }
                resolve_stmts(body, ctx);
            }
            Stmt::Break | Stmt::Continue => {}
            Stmt::ForEach { var1, var2, iter, body } => {
                resolve_expr(iter, ctx);
                // A ByVal collection param is already `&Vec`; reborrow it (`&*p`)
                // so the emitter's `&` doesn't produce a `&&Vec` double-borrow.
                if let Expr::Ident(n) = &*iter {
                    if ctx.borrowed_collections.contains(&snake(n)) {
                        let inner = std::mem::replace(iter, Expr::Int(0));
                        *iter = Expr::Deref(Box::new(inner));
                    }
                }
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
            Stmt::Select { scrutinee, arms, else_body, line } => {
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
                    if let Some(g) = &mut arm.guard {
                        resolve_expr(g, ctx);
                    } else {
                        // After const-resolution, a bare-identifier pattern (other
                        // than `_`/`None`) in a guardless arm can only be a variable
                        // or unknown name. In a Rust `match` that *binds* and matches
                        // everything — it does NOT compare like VB. Reject it.
                        for pat in &arm.patterns {
                            if let CasePattern::Value(Expr::Ident(name)) = pat {
                                if name != "_" && name != "None" {
                                    ctx.diags.error(
                                        *line,
                                        format!(
                                            "`Case {name}` can't compare against a variable — \
                                             Rust's `match` would treat `{name}` as a catch-all \
                                             that matches everything. To compare, use a guard: \
                                             `Case v If v = {name}`. For the default, use `Case Else`.",
                                        ),
                                    );
                                }
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

/// Maths builtins that require a floating-point receiver (`abs` works on ints).
fn maths_needs_float(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "sqr" | "sin" | "cos" | "tan" | "log" | "exp" | "int" | "round"
    )
}

fn is_literal(e: &Expr) -> bool {
    matches!(e, Expr::Int(_) | Expr::Float(_))
}

/// Can this expression be borrowed mutably (i.e. is it a place)?
fn is_lvalue(e: &Expr) -> bool {
    matches!(
        e,
        Expr::Ident(_) | Expr::Field(..) | Expr::Index(..) | Expr::TupleIndex(..) | Expr::Deref(_)
    )
}

/// If `e` is a bare call yielding a Result/Option, returns its kind name.
fn ignored_result(e: &Expr, ctx: &Ctx) -> Option<&'static str> {
    if let Expr::Call { name, .. } = e {
        match ctx.fns.get(&snake(name)).and_then(|s| s.ret.as_ref()) {
            Some(DeclType::Result(_)) => return Some("Result"),
            Some(DeclType::Option(_)) => return Some("Option"),
            _ => {}
        }
        match name.to_ascii_lowercase().as_str() {
            "instr" => return Some("Option"),
            "val" => return Some("Result"),
            _ => {}
        }
    }
    None
}

fn resolve_expr(e: &mut Expr, ctx: &mut Ctx) {
    match e {
        // An opaque Rust handle appearing as a value — the one thing it can't do.
        Expr::Ident(name) if ctx.handles.contains(&snake(name)) => {
            ctx.diags.error_once(
                &format!("handle-value-{}", snake(name)),
                format!(
                    "'{}' is an opaque Rust handle — its type lives only inside Rust. \
                     You can pass it back into another `Rust … End Rust` block, but VBR \
                     can't print it, compare it, assign it, or pass it to a function.",
                    name
                ),
            );
        }
        Expr::Ident(name) if ctx.deref.contains(&snake(name)) => {
            *e = Expr::Deref(Box::new(Expr::Ident(name.clone())));
        }
        // A reference to a module constant → its SCREAMING_SNAKE name, verbatim.
        Expr::Ident(name) if ctx.consts.contains_key(name) => {
            *e = Expr::ConstRef(ctx.consts[name].clone());
        }
        Expr::Ident(_) | Expr::ConstRef(_) => {}
        Expr::Binary { op, lhs, rhs } => {
            resolve_expr(lhs, ctx);
            resolve_expr(rhs, ctx);
            // Integer `^` → `base.pow(exp)` (Rust's integer pow); only a float base
            // uses `.powi`/`.powf` (handled in the renderer).
            if *op == BinOp::Pow {
                let base = infer(lhs, ctx);
                if base.is_numeric() && !base.is_float() {
                    let recv = std::mem::replace(&mut **lhs, Expr::Int(0));
                    let exp = std::mem::replace(&mut **rhs, Expr::Int(0));
                    *e = Expr::MethodCall {
                        recv: Box::new(recv),
                        method: "pow".to_string(),
                        args: vec![exp],
                    };
                }
                return;
            }
            // A `usize` operand (e.g. from `.Len()`) meeting a signed integer won't
            // compile; cast the usize side to the other operand's type.
            let (lt, rt) = (infer(lhs, ctx), infer(rhs, ctx));
            if lt == RType::Usize && rt.is_numeric() && rt != RType::Usize {
                if let Some(t) = rtype_to_type(rt) {
                    maybe_cast(lhs, t, ctx);
                }
            } else if rt == RType::Usize && lt.is_numeric() && lt != RType::Usize {
                if let Some(t) = rtype_to_type(lt) {
                    maybe_cast(rhs, t, ctx);
                }
            }
        }
        Expr::MethodCall { recv, method, args } => {
            // Calling a `&mut self` method on a variable means it must be `mut`.
            let recv_var = match &**recv {
                Expr::Ident(v) => Some(snake(v)),
                _ => None,
            };
            resolve_expr(recv, ctx);
            for a in args.iter_mut() {
                resolve_expr(a, ctx);
            }
            // `coll.push(s)` / `coll.insert(s)` of a `&str` need an owned String
            // payload (a `Vec<String>`/`HashMap` slot owns its value).
            if matches!(snake(method).as_str(), "push" | "insert") {
                for arg in args.iter_mut() {
                    if infer(arg, ctx) == RType::Str && !matches!(arg, Expr::Str(_)) {
                        to_owned_string(arg);
                    }
                }
            }
            // `.Clone()` on a ByVal String parameter (a `&str`) yields a `&str`,
            // not an owned String — use `.to_string()` so it fits a String slot.
            if method.eq_ignore_ascii_case("clone") {
                if let Expr::Ident(n) = &**recv {
                    if ctx.str_params.contains(&snake(n)) {
                        *method = "to_string".to_string();
                    }
                }
            }
            // Stdlib functions take string args by `&str`; borrow an owned String.
            if matches!(&**recv, Expr::Ident(n) if stdlib_type(n).is_some()) {
                for arg in args.iter_mut() {
                    if infer(arg, ctx) == RType::Strng {
                        let inner = std::mem::replace(arg, Expr::Int(0));
                        *arg = Expr::Ref(Box::new(inner));
                    }
                }
            }
            if let Some(v) = recv_var {
                if let Some(struct_name) = ctx.struct_vars.get(&v) {
                    let key = (struct_name.clone(), snake(method));
                    if ctx.methods.get(&key) == Some(&true) {
                        ctx.passed.insert(v);
                    }
                }
            }
            // `Utils.DoThing(x)` on another project module → `crate::utils::do_thing(x)`.
            let qualified = match &**recv {
                Expr::Ident(m) if ctx.modules.contains(&snake(m)) => {
                    Some(format!("crate::{}::{}", snake(m), snake(method)))
                }
                _ => None,
            };
            if let Some(path) = qualified {
                let taken = std::mem::take(args);
                *e = Expr::Call { name: path, args: taken };
            }
        }
        Expr::Call { name, args } => {
            // `x(i)` where x is an array is the VB way — point at Rust indexing.
            if ctx.array_vars.contains(&snake(name)) {
                ctx.diags.error_once(
                    &format!("array-call-{}", snake(name)),
                    format!(
                        "'{}' is an array — index it Rust-style with `{}[i]`, or use \
                         `{}.get(i)` for a safe Option.",
                        name, name, name
                    ),
                );
            }
            for a in args.iter_mut() {
                resolve_expr(a, ctx);
            }
            // `Ok(s)` / `Some(s)` of a `&str` need an owned String payload
            // (VBR's `Result<String>`/`Option<String>` own their value).
            if matches!(name.as_str(), "Ok" | "Some") && args.len() == 1
                && infer(&args[0], ctx) == RType::Str
            {
                to_owned_string(&mut args[0]);
            }
            // Maths builtins need a floating-point receiver — cast an integer
            // argument so e.g. `Sqr(n)` becomes `(n as f64).sqrt()`.
            if maths_needs_float(name) && args.len() == 1 && !is_literal(&args[0]) {
                let t = infer(&args[0], ctx);
                if t.is_numeric() && !t.is_float() {
                    let inner = std::mem::replace(&mut args[0], Expr::Int(0));
                    args[0] = Expr::Cast(Box::new(inner), Type::Double);
                }
            }
            if let Some(sig) = ctx.fns.get(&snake(name)) {
                for (i, arg) in args.iter_mut().enumerate() {
                    match sig.modes.get(i) {
                        // ByRef: borrow mutably; the local must be `mut`.
                        Some(ParamMode::ByRef) => {
                            if !is_lvalue(arg) {
                                ctx.diags.error_once(
                                    "byref-lvalue",
                                    "A ByRef parameter must be given a variable (so it can be \
                                     borrowed and changed in place), not a literal or an \
                                     expression.",
                                );
                            }
                            if let Expr::Ident(v) = arg {
                                ctx.passed.insert(snake(v));
                            }
                            let inner = std::mem::replace(arg, Expr::Int(0));
                            *arg = Expr::MutRef(Box::new(inner));
                        }
                        // ByVal of an unknown-size type borrows immutably (`&arg`).
                        Some(ParamMode::ByVal) => {
                            let needs_ref = match sig.param_types.get(i) {
                                Some(
                                    DeclType::Named(_) | DeclType::Vec(_) | DeclType::Map(..),
                                ) => true,
                                // A `&str` param: borrow an owned String, but leave an
                                // existing slice (literal, `&str` param, `Trim(..)`) alone.
                                Some(DeclType::Plain(Type::Text)) => {
                                    infer(arg, ctx) != RType::Str
                                }
                                _ => false,
                            };
                            if needs_ref {
                                let inner = std::mem::replace(arg, Expr::Int(0));
                                *arg = Expr::Ref(Box::new(inner));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Expr::Deref(inner) | Expr::MutRef(inner) | Expr::Ref(inner) | Expr::Cast(inner, _)
        | Expr::Field(inner, _) | Expr::Closure { body: inner, .. }
        | Expr::TupleIndex(inner, _) => resolve_expr(inner, ctx),
        Expr::Try(inner) => {
            resolve_expr(inner, ctx);
            // `?` returns the error to the caller on failure, so the enclosing
            // function must itself be able to fail (return Result / Option).
            if !ctx.can_propagate {
                ctx.diags.error_once(
                    "try-needs-result",
                    "`?` can only be used in a function that returns `Result` (or `Option`). \
                     It hands the error back to the caller on failure, so this function's \
                     signature must allow failure: declare it `As Result<T>`, or handle the \
                     error here with `Select Case` over `Ok`/`Err`.",
                );
            }
        }
        Expr::Tuple(elems) => {
            for el in elems.iter_mut() {
                resolve_expr(el, ctx);
            }
        }
        Expr::Index(inner, idx) => {
            resolve_expr(inner, ctx);
            resolve_expr(idx, ctx);
        }
        Expr::StructLit { fields, .. } => {
            for (_, v) in fields.iter_mut() {
                resolve_expr(v, ctx);
            }
        }
        // Inline Rust is opaque — no resolution.
        Expr::InlineRust(_) => {}
        Expr::Not(inner) => resolve_expr(inner, ctx),
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
        Expr::Ident(name) if ctx.str_params.contains(&snake(name)) => RType::Str,
        Expr::Ident(name) => ctx.vars.get(&snake(name)).copied().map_or(RType::Unknown, rtype_of),
        Expr::Deref(inner) => infer(inner, ctx),
        Expr::Cast(_, ty) => rtype_of(*ty),
        // `?` unwraps a Result/Option to its payload; we don't track that yet.
        Expr::Try(_) => RType::Unknown,
        Expr::MutRef(_) | Expr::Ref(_) => RType::Unknown,
        // Struct/tuple values, field types, const refs, closures: not numeric.
        Expr::StructLit { .. }
        | Expr::Field(..)
        | Expr::ConstRef(_)
        | Expr::Closure { .. }
        | Expr::Tuple(_)
        | Expr::TupleIndex(..)
        | Expr::Index(..)
        | Expr::InlineRust(_) => RType::Unknown,
        Expr::Not(_) => RType::Bool,
        Expr::Binary { op, lhs, rhs } => match op {
            BinOp::Concat => RType::Strng,
            BinOp::Pow => RType::F64,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => RType::Bool,
            BinOp::And | BinOp::Or | BinOp::Xor => RType::Bool,
            _ => join(infer(lhs, ctx), infer(rhs, ctx)),
        },
        Expr::Call { name, args } => builtin_rtype(name).unwrap_or_else(|| {
            // Not a builtin? A plain return type is numeric; Result/Option aren't.
            let _ = args;
            match ctx.fns.get(&snake(name)).and_then(|s| s.ret.as_ref()) {
                Some(DeclType::Plain(t)) => rtype_of(*t),
                _ => RType::Unknown,
            }
        }),
        // `.len()`/`.count()` return `usize` — needed so comparisons/assignments
        // against signed ints get the right `as` cast.
        Expr::MethodCall { method, .. } => match snake(method).as_str() {
            "len" | "count" => RType::Usize,
            _ => RType::Unknown,
        },
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
