//! Type-resolution pass.
//!
//! Builds a table of user-function signatures and a per-function typed
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
//!
//! Everything known about a name lives in one environment (`name → Binding`),
//! keyed by the emitted (lowercased) name: the declared `DeclType`, whether the
//! generated Rust already holds it as a borrow, or — for an opaque handle —
//! the honest admission that only Rust knows its type.

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diagnostics::Diagnostics;
use crate::transpiler::rust_name as snake;
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

/// Struct name → (field snake name → field type). Lets `infer` see through a
/// `p.field` access, so field values get the same coercions variables do.
pub type StructTable = HashMap<String, HashMap<String, DeclType>>;

pub fn build_struct_table(program: &Program) -> StructTable {
    program
        .structs
        .iter()
        .map(|s| {
            (
                s.name.clone(),
                s.fields
                    .iter()
                    .map(|f| (snake(&f.name), f.ty.clone()))
                    .collect(),
            )
        })
        .collect()
}

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

/// Does this method body mutate `Me` — assign to one of its fields, or call a
/// mutating method (`push`, `sort`, …) on one — so it needs `&mut self`?
pub fn method_mutates_self(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::Assign { target, .. } => is_me_field(target),
        Stmt::Expr(e) => expr_mutates_me(e),
        Stmt::If { branches, else_body } => {
            branches.iter().any(|(_, b)| method_mutates_self(b))
                || else_body.as_ref().map_or(false, |b| method_mutates_self(b))
        }
        Stmt::For { body, .. }
        | Stmt::ForEach { body, .. }
        | Stmt::DoLoop { body, .. } => method_mutates_self(body),
        Stmt::Match { arms, .. } => arms.iter().any(|a| method_mutates_self(&a.body)),
        _ => false,
    })
}

fn is_me_field(e: &Expr) -> bool {
    match e {
        Expr::Field(inner, _) => matches!(&**inner, Expr::Ident(n) if n == "Me") || is_me_field(inner),
        _ => false,
    }
}

/// Does this expression call a mutating method on `Me` or a place rooted in it
/// (`Me.items.Push(x)`, `Me.grid(r).sort()`)?
fn expr_mutates_me(e: &Expr) -> bool {
    match e {
        Expr::MethodCall { recv, method, args } => {
            (crate::transpiler::is_mutating_method(&snake(method)) && is_me_rooted(recv))
                || expr_mutates_me(recv)
                || args.iter().any(expr_mutates_me)
        }
        _ => false,
    }
}

/// Is this place expression `Me` itself or reached through it?
fn is_me_rooted(e: &Expr) -> bool {
    match e {
        Expr::Ident(n) => n == "Me",
        Expr::Field(inner, _) | Expr::Index(inner, _) => is_me_rooted(inner),
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

// ---- The typed environment ------------------------------------------------------

/// Everything the resolver knows about one name in scope.
#[derive(Clone, Debug)]
struct Binding {
    /// The declared type. `None` for an opaque `Dim h = Rust …` handle, whose
    /// type lives only inside Rust.
    ty: Option<DeclType>,
    /// A ByVal parameter the generated Rust holds as a borrow (`&str` for a
    /// String, `&Vec`/`&HashMap` for a collection) — already a reference, so
    /// uses of it must not borrow it a second time.
    borrowed: bool,
}

/// A resolved expression type: a full `DeclType` where we know it, plus the
/// two Rust-only cases coercion cares about (`&str` slices and `usize`
/// lengths), and honest ignorance. Callers only act on what they recognise —
/// anything `Unknown` passes through untouched, with rustc as the backstop.
#[derive(Clone, Debug, PartialEq)]
enum VType {
    Decl(DeclType),
    /// A borrowed `&str` slice (a literal, a ByVal String param, `Trim(..)`).
    Str,
    /// A `usize` length/count (`.len()`, `.count()`).
    Usize,
    Unknown,
}

/// Shorthand for a plain scalar `VType`.
fn vt(t: Type) -> VType {
    VType::Decl(DeclType::Plain(t))
}

impl VType {
    /// The plain scalar type, if that's what this is.
    fn plain(&self) -> Option<Type> {
        match self {
            VType::Decl(DeclType::Plain(t)) => Some(*t),
            _ => None,
        }
    }

    /// An owned `String` value (needs `&` to feed a `&str` parameter).
    fn is_owned_string(&self) -> bool {
        self.plain() == Some(Type::Text)
    }
}

/// The Rust numeric representation of a `VType`, for cast/widening decisions.
/// `Long` and `LongLong` are both `i64` — the same repr never needs a cast.
#[derive(Clone, Copy, Debug, PartialEq)]
enum NumTy {
    I32,
    I64,
    U8,
    Usize,
    F32,
    F64,
}

impl NumTy {
    fn is_float(self) -> bool {
        matches!(self, NumTy::F32 | NumTy::F64)
    }
}

/// The numeric repr of a value type, or `None` if it isn't numeric.
fn num_ty(v: &VType) -> Option<NumTy> {
    match v {
        VType::Usize => Some(NumTy::Usize),
        VType::Decl(DeclType::Plain(t)) => num_of_type(*t),
        _ => None,
    }
}

/// The numeric repr of a VB scalar type, or `None` (Boolean/String aren't).
fn num_of_type(t: Type) -> Option<NumTy> {
    Some(match t {
        Type::Integer => NumTy::I32,
        Type::Long | Type::LongLong => NumTy::I64,
        Type::Byte => NumTy::U8,
        Type::Single => NumTy::F32,
        Type::Double => NumTy::F64,
        Type::Boolean | Type::Text => return None,
    })
}

/// The VB type a numeric repr casts to (for inserting `as` casts). `usize` has
/// no VB-type target, so it never becomes a cast target itself.
fn type_of_num(n: NumTy) -> Option<Type> {
    Some(match n {
        NumTy::I32 => Type::Integer,
        NumTy::I64 => Type::Long,
        NumTy::U8 => Type::Byte,
        NumTy::F32 => Type::Single,
        NumTy::F64 => Type::Double,
        NumTy::Usize => return None,
    })
}

/// The widened type of arithmetic between two numeric reprs — the VB type both
/// operands should be cast up to. Floats beat integers, `f64` beats `f32`,
/// `i64` beats `usize` beats the 32-bit-and-under tier.
fn widen(a: NumTy, b: NumTy) -> Option<Type> {
    fn rank(n: NumTy) -> u8 {
        match n {
            NumTy::F64 => 6,
            NumTy::F32 => 5,
            NumTy::I64 => 4,
            NumTy::Usize => 3,
            NumTy::I32 | NumTy::U8 => 2,
        }
    }
    let top = if rank(a) >= rank(b) { a } else { b };
    match rank(top) {
        6 => Some(Type::Double),
        5 => Some(Type::Single),
        4 => Some(Type::Long),
        3 => None, // usize: no VB cast target
        _ => Some(Type::Integer),
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

/// Result type of a known builtin, used so e.g. `Len` into a `Long` casts.
fn builtin_vtype(name: &str) -> Option<VType> {
    Some(match name.to_ascii_lowercase().as_str() {
        "len" => VType::Usize,
        "left" | "right" | "mid" | "trim" => VType::Str,
        "ucase" | "lcase" | "replace" | "str" | "inputbox" => vt(Type::Text),
        "sqr" | "abs" | "int" | "round" | "sin" | "cos" | "tan" | "log" | "exp" => vt(Type::Double),
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
    enums: &HashSet<String>,
    structs: &StructTable,
    receiver: Option<&str>,
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

    // The typed environment. VB has no block scope, so a flat map matches the
    // mental model. Parameters seed it; `Dim`s add to it as they're met.
    let mut env: HashMap<String, Binding> = HashMap::new();
    for p in params {
        // A ByVal String is a `&str`; a ByVal collection is a `&Vec`/`&HashMap` —
        // both already borrows in the generated Rust.
        let borrowed = p.mode == ParamMode::ByVal
            && matches!(
                p.ty,
                DeclType::Plain(Type::Text) | DeclType::Vec(_) | DeclType::Map(..)
            );
        env.insert(
            snake(&p.name),
            Binding { ty: Some(p.ty.clone()), borrowed },
        );
    }
    // Inside a method, `Me` is the receiver struct — so `Me.field` infers.
    if let Some(recv) = receiver {
        env.insert(
            "me".to_string(),
            Binding { ty: Some(DeclType::Named(recv.to_string())), borrowed: false },
        );
    }

    let mut passed = HashSet::new();
    let mut ctx = Ctx {
        deref: byref,
        fns,
        methods,
        consts,
        ret_coerce,
        can_propagate,
        diags,
        env: &mut env,
        passed: &mut passed,
        modules,
        enums,
        structs,
    };
    resolve_stmts(stmts, &mut ctx);
    passed
}

/// Rewrite a GUI/TUI event body in place — the same resolution an ordinary
/// function body gets (stdlib method names, string/numeric coercions, iterator
/// chains, teaching diagnostics). The window's state fields join the
/// environment as owned values (the backend rewrites them to `state.field`
/// afterwards); the event's params are message payloads, also owned (never
/// `&str` borrows).
#[allow(clippy::too_many_arguments)]
pub fn resolve_event_body(
    stmts: &mut [Stmt],
    params: &[Param],
    state: &HashMap<String, DeclType>,
    fns: &FnTable,
    methods: &MethodTable,
    consts: &HashMap<String, String>,
    enums: &HashSet<String>,
    structs: &StructTable,
    diags: &mut Diagnostics,
) {
    let mut env: HashMap<String, Binding> = HashMap::new();
    for p in params {
        env.insert(snake(&p.name), Binding { ty: Some(p.ty.clone()), borrowed: false });
    }
    for (name, ty) in state {
        env.insert(name.clone(), Binding { ty: Some(ty.clone()), borrowed: false });
    }
    let modules = HashSet::new();
    let mut passed = HashSet::new();
    let mut ctx = Ctx {
        deref: HashSet::new(),
        fns,
        methods,
        consts,
        ret_coerce: None,
        can_propagate: false,
        diags,
        env: &mut env,
        passed: &mut passed,
        modules: &modules,
        enums,
        structs,
    };
    resolve_stmts(stmts, &mut ctx);
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
    /// The one typed environment: emitted (snake_case) name → what we know.
    env: &'a mut HashMap<String, Binding>,
    passed: &'a mut HashSet<String>,
    /// Other project modules (snake-cased file stems). A `Module.func(...)` call
    /// on one rewrites to a qualified `crate::module::func(...)`.
    modules: &'a HashSet<String>,
    /// Enum names. `Color.Red` (a field access on an enum name) rewrites to the
    /// path `Color::Red`.
    enums: &'a HashSet<String>,
    /// User struct definitions, so `p.field` infers to the field's type.
    structs: &'a StructTable,
}

impl Ctx<'_> {
    /// Record a declared variable.
    fn bind(&mut self, name: &str, ty: DeclType) {
        self.env.insert(snake(name), Binding { ty: Some(ty), borrowed: false });
    }

    fn binding(&self, name: &str) -> Option<&Binding> {
        self.env.get(&snake(name))
    }

    /// An opaque `Dim h = Rust …` handle — a name whose type only Rust knows.
    fn is_handle(&self, name: &str) -> bool {
        self.binding(name).map_or(false, |b| b.ty.is_none())
    }

    /// A ByVal String parameter — a read-only `&str` in the generated Rust.
    fn is_str_param(&self, name: &str) -> bool {
        self.binding(name).map_or(false, |b| {
            b.borrowed && matches!(b.ty, Some(DeclType::Plain(Type::Text)))
        })
    }

    /// The declared plain scalar type of a variable, if it has one.
    fn scalar_of(&self, name: &str) -> Option<Type> {
        match self.binding(name)?.ty.as_ref()? {
            DeclType::Plain(t) => Some(*t),
            _ => None,
        }
    }

    /// The struct (named-type) name a variable was declared as, if any.
    fn struct_of(&self, name: &str) -> Option<&str> {
        match self.binding(name)?.ty.as_ref()? {
            DeclType::Named(n) => Some(n),
            _ => None,
        }
    }

    /// An indexable collection (`Vec`, fixed array, `HashMap`) — `x(i)` on one
    /// is the friendly "index Rust-style" error, and `Vec::contains` borrows
    /// its argument.
    fn is_indexable(&self, name: &str) -> bool {
        matches!(
            self.binding(name).and_then(|b| b.ty.as_ref()),
            Some(
                DeclType::Vec(_) | DeclType::Array(..) | DeclType::Array2D(..) | DeclType::Map(..)
            )
        )
    }

    /// A ByVal collection parameter — already a `&Vec`/`&HashMap`, so a
    /// `For Each` over it must reborrow (`&*p`) rather than double-borrow.
    fn is_borrowed_collection(&self, name: &str) -> bool {
        self.binding(name).map_or(false, |b| {
            b.borrowed && matches!(b.ty, Some(DeclType::Vec(_) | DeclType::Map(..)))
        })
    }
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
                        if *t == Type::Text
                            && infer(e, ctx) == VType::Str
                            && !matches!(e, Expr::Str(_))
                        {
                            to_owned_string(e);
                        }
                    }
                } else if let Some(e) = init {
                    resolve_expr(e, ctx);
                }
                ctx.bind(name, ty.clone());
            }
            Stmt::DestructureDim { names, ty, value } => {
                resolve_expr(value, ctx);
                // A typed destructure (`Dim (a, b) As (T, U) = …`) binds each name.
                if let Some(DeclType::Tuple(ts)) = ty {
                    for (n, t) in names.iter().zip(ts) {
                        ctx.bind(n, t.clone());
                    }
                }
            }
            // The body is raw Rust (not an Expr), so there's nothing to resolve —
            // just record the name so later value-uses of it are caught.
            Stmt::HandleDim { name, .. } => {
                ctx.env
                    .insert(snake(name), Binding { ty: None, borrowed: false });
            }
            Stmt::Assign { target, value, .. } => {
                // Writing to a ByVal String parameter — it's a read-only `&str`.
                if let Expr::Ident(name) = &*target {
                    if ctx.is_str_param(name) {
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
                    Expr::Ident(name) => ctx.scalar_of(name),
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
                    if ty == Type::Text && infer(value, ctx) == VType::Str {
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
                    Some(Type::Text) if infer(e, ctx) == VType::Str => to_owned_string(e),
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
                             `Match` over Ok/Err (or Some/None), or assign it with `Dim`.",
                            kind
                        ),
                    );
                }
            }
            // Draw commands only appear in canvas bodies, which the GUI codegen
            // rewrites/renders directly (they never reach the resolver); a
            // LineMark is bookkeeping for the emitter, nothing to resolve.
            Stmt::Return(None) | Stmt::Comment(_) | Stmt::Draw(_) | Stmt::LineMark(_) => {}
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
                // The loop variable's type is what Rust infers from the range
                // bounds: `For i = 1 To 10` → `i32`, but `For i = 1 To count`
                // (count a `Long`) → `i64`. Cast both bounds to that common type so
                // a mixed-width range (`For i = lo To hi`, different widths) still
                // compiles, and track the var type for body arithmetic.
                let var_ty = match (num_ty(&infer(from, ctx)), num_ty(&infer(to, ctx))) {
                    (Some(a), Some(b)) => widen(a, b).unwrap_or(Type::Integer),
                    _ => Type::Integer,
                };
                maybe_cast(from, var_ty, ctx);
                maybe_cast(to, var_ty, ctx);
                ctx.bind(var, DeclType::Plain(var_ty));
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
                    if ctx.is_borrowed_collection(n) {
                        let inner = std::mem::replace(iter, Expr::Int(0));
                        *iter = Expr::Deref(Box::new(inner));
                    }
                }
                // The loop variables carry the collection's element type, so
                // body arithmetic and coercions see through them.
                match infer(iter, ctx) {
                    VType::Decl(DeclType::Vec(t)) => ctx.bind(var1, *t),
                    VType::Decl(DeclType::Array(t, _)) => ctx.bind(var1, DeclType::Plain(t)),
                    VType::Decl(DeclType::Map(k, v)) => {
                        ctx.bind(var1, *k);
                        if let Some(v2) = var2 {
                            ctx.bind(v2, *v);
                        }
                    }
                    _ => {}
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
            Stmt::Match { scrutinee, arms, .. } => {
                resolve_expr(scrutinee, ctx);
                // Patterns are raw Rust text (bindings live only inside the arm),
                // so there's nothing to resolve there — just the guard and body.
                for arm in arms.iter_mut() {
                    if let Some(g) = &mut arm.guard {
                        resolve_expr(g, ctx);
                    }
                    resolve_stmts(&mut arm.body, ctx);
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
    let Some(target_n) = num_of_type(target) else {
        return;
    };
    let Some(src) = num_ty(&infer(value, ctx)) else {
        return;
    };
    if src != target_n {
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

/// Operators whose operands must be the same numeric type (so a width mismatch
/// needs a widening cast). Arithmetic and comparison; not `&`/logical/`^`.
fn is_arith_or_cmp(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Add
            | BinOp::Sub
            | BinOp::Mul
            | BinOp::Div
            | BinOp::Mod
            | BinOp::Eq
            | BinOp::Ne
            | BinOp::Lt
            | BinOp::Gt
            | BinOp::Le
            | BinOp::Ge
    )
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
            Some(DeclType::Result(..)) => return Some("Result"),
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
    // Iterator chains on a `Vec`/array (`nums.filter(|x| …).map(…)`) are
    // handled whole — the resolver knows the element type, so it builds the
    // chain root (`.iter().copied()` for Copy elements, `.cloned()` for owned
    // ones) and types the closure parameters. A receiver that turns out not
    // to be a sequence falls through to normal method resolution.
    if let Expr::MethodCall { method, args, .. } = &*e {
        if is_iter_adapter(&snake(method), args) && resolve_iter_chain(e, ctx).is_some() {
            return;
        }
    }
    match e {
        // An opaque Rust handle appearing as a value — the one thing it can't do.
        Expr::Ident(name) if ctx.is_handle(name) => {
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
                if let Some(base) = num_ty(&infer(lhs, ctx)) {
                    if !base.is_float() {
                        let recv = std::mem::replace(&mut **lhs, Expr::Int(0));
                        let exp = std::mem::replace(&mut **rhs, Expr::Int(0));
                        *e = Expr::MethodCall {
                            recv: Box::new(recv),
                            method: "pow".to_string(),
                            args: vec![exp],
                        };
                    }
                }
                return;
            }
            let (ln, rn) = (num_ty(&infer(lhs, ctx)), num_ty(&infer(rhs, ctx)));
            // A `usize` operand (e.g. from `.Len()`) meeting a signed integer won't
            // compile; cast the usize side to the other operand's type.
            match (ln, rn) {
                (Some(NumTy::Usize), Some(r)) if r != NumTy::Usize => {
                    if let Some(t) = type_of_num(r) {
                        maybe_cast(lhs, t, ctx);
                    }
                }
                (Some(l), Some(NumTy::Usize)) if l != NumTy::Usize => {
                    if let Some(t) = type_of_num(l) {
                        maybe_cast(rhs, t, ctx);
                    }
                }
                (Some(l), Some(r)) if is_arith_or_cmp(*op) && l != r => {
                    // Mixed numeric widths (e.g. `Long * Integer`, `Integer <= Long`):
                    // VB widens silently, but Rust needs both sides the same type — so
                    // cast the narrower up to the wider (`widen`). Literals are left
                    // alone (they infer); only the narrower non-literal is cast.
                    if let Some(t) = widen(l, r) {
                        maybe_cast(lhs, t, ctx);
                        maybe_cast(rhs, t, ctx);
                    }
                }
                _ => {}
            }
        }
        Expr::MethodCall { recv, method, args } => {
            // DataFrame transforms take column *formulas*, not ordinary
            // expressions: bare names are columns and operators broadcast. Lower
            // them here — where we know `recv` is a DataFrame — into polars
            // expressions, and skip the normal argument resolution.
            if is_df_expr(recv, ctx) {
                resolve_expr(recv, ctx);
                // Map the VBR spelling to the wrapper's real method name
                // (`WithColumn` → `with_column`) before dispatching on it.
                if let Some(real) = crate::transpiler::stdlib_method(&snake(method)) {
                    *method = real.to_string();
                }
                match snake(method).as_str() {
                    "filter" => {
                        for a in args.iter_mut() {
                            lower_formula(a, ctx);
                        }
                    }
                    "with_column" => {
                        // arg 0 is the new column's name (a value); arg 1 the formula.
                        if let Some(a) = args.get_mut(1) {
                            lower_formula(a, ctx);
                        }
                    }
                    "select" => {
                        // Rendered as a slice of name literals; tag for the emitter.
                        *method = "__df_select".to_string();
                    }
                    "group_by" => {
                        // Key columns render as a slice of names, like `select`.
                        *method = "__df_group_by".to_string();
                    }
                    m @ ("join" | "left_join" | "outer_join") => {
                        // arg 0 is the other frame (borrowed — the wrapper
                        // takes `&DataFrame`); the rest are key column names.
                        let tag = format!("__df_{}", m);
                        if let Some(a) = args.first_mut() {
                            resolve_expr(a, ctx);
                            let inner = std::mem::replace(a, Expr::Int(0));
                            *a = Expr::Ref(Box::new(inner));
                        }
                        for a in args.iter_mut().skip(1) {
                            resolve_expr(a, ctx);
                        }
                        *method = tag;
                    }
                    "agg" => {
                        // Each argument is an aggregation formula: `Sum(sales)`,
                        // `Mean(price * qty)` → `col("sales").sum()`, ….
                        for a in args.iter_mut() {
                            lower_agg(a, ctx);
                        }
                        *method = "__df_agg".to_string();
                    }
                    _ => {
                        // Head/Sort/Column/Sum/Mean/Min/Max/…: plain value args.
                        for a in args.iter_mut() {
                            resolve_expr(a, ctx);
                        }
                    }
                }
                return;
            }
            // Calling a `&mut self` method on a variable means it must be `mut`.
            let recv_var = match &**recv {
                Expr::Ident(v) => Some(snake(v)),
                _ => None,
            };
            // A stdlib wrapper method keeps its real snake_case name
            // (`GetString` → `get_string`) — unless the receiver is a user
            // struct that defines its own method of that (lowercased) name.
            if let Some(real) = crate::transpiler::stdlib_method(&snake(method)) {
                let user_method = recv_var
                    .as_ref()
                    .and_then(|v| ctx.struct_of(v))
                    .map_or(false, |s| {
                        ctx.methods.contains_key(&(s.to_string(), snake(method)))
                    });
                if !user_method {
                    *method = real.to_string();
                }
            }
            resolve_expr(recv, ctx);
            for a in args.iter_mut() {
                // A closure argument to a pass-through method (`sort_by_key`,
                // …) is legal — resolve its body directly (and check it only
                // reads its captures); anything else resolves normally.
                if let Expr::Closure { params, body, .. } = a {
                    let params = params.clone();
                    resolve_expr(body, ctx);
                    check_closure_captures(&params, body, ctx);
                } else {
                    resolve_expr(a, ctx);
                }
            }
            // `coll.push(s)` / `coll.insert(s)` of a `&str` need an owned String
            // payload (a `Vec<String>`/`HashMap` slot owns its value).
            if matches!(snake(method).as_str(), "push" | "insert") {
                for arg in args.iter_mut() {
                    if infer(arg, ctx) == VType::Str && !matches!(arg, Expr::Str(_)) {
                        to_owned_string(arg);
                    }
                }
            }
            // `vec.contains(x)` takes `&T` (unlike `str::contains`), so borrow the
            // argument when the receiver is a known collection — and own a string
            // element first (`Vec<String>` holds `String`, not `&str`).
            if snake(method) == "contains" {
                if let Expr::Ident(r) = &**recv {
                    if ctx.is_indexable(r) {
                        for arg in args.iter_mut() {
                            if infer(arg, ctx) == VType::Str {
                                to_owned_string(arg);
                            }
                            let inner = std::mem::replace(arg, Expr::Int(0));
                            *arg = Expr::Ref(Box::new(inner));
                        }
                    }
                }
            }
            // `.Clone()` on a ByVal String parameter (a `&str`) yields a `&str`,
            // not an owned String — use `.to_string()` so it fits a String slot.
            if method.eq_ignore_ascii_case("clone") {
                if let Expr::Ident(n) = &**recv {
                    if ctx.is_str_param(n) {
                        *method = "to_string".to_string();
                    }
                }
            }
            // Stdlib functions take string args by `&str`; borrow an owned String.
            // (Collections like `Http.Post`'s headers map are passed by value.)
            if matches!(&**recv, Expr::Ident(n) if stdlib_type(n).is_some()) {
                for arg in args.iter_mut() {
                    if infer(arg, ctx).is_owned_string() {
                        let inner = std::mem::replace(arg, Expr::Int(0));
                        *arg = Expr::Ref(Box::new(inner));
                    }
                }
            }
            if let Some(v) = recv_var {
                if let Some(struct_name) = ctx.struct_of(&v) {
                    let key = (struct_name.to_string(), snake(method));
                    if ctx.methods.get(&key) == Some(&true) {
                        ctx.passed.insert(v);
                    }
                }
            }
            // `Shape.Circle(r)` on an enum → the variant constructor
            // `Shape::Circle(r)` (variant kept PascalCase). A string payload is
            // owned — VBR enum text payloads are `String`, never `&str`.
            if let Expr::Ident(m) = &**recv {
                if ctx.enums.contains(m) {
                    let path = format!("{}::{}", m, method);
                    for arg in args.iter_mut() {
                        if infer(arg, ctx) == VType::Str {
                            to_owned_string(arg);
                        }
                    }
                    let taken = std::mem::take(args);
                    *e = Expr::Call { name: path, args: taken };
                    return;
                }
            }
            // `Utils.DoThing(x)` on a project module → `crate::utils::do_thing(x)`.
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
            if ctx.is_indexable(name) {
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
            // `Ok(s)` / `Err(s)` / `Some(s)` of a `&str` need an owned `String`
            // payload. For `Err` this only fires when the payload is a string —
            // i.e. the error type is `String`; a typed error (`Err(MyErr.X)`)
            // isn't a `&str`, so it's left alone.
            if matches!(name.as_str(), "Ok" | "Err" | "Some")
                && args.len() == 1
                && infer(&args[0], ctx) == VType::Str
            {
                to_owned_string(&mut args[0]);
            }
            // Maths builtins need a floating-point receiver — cast an integer
            // argument so e.g. `Sqr(n)` becomes `(n as f64).sqrt()`.
            if maths_needs_float(name) && args.len() == 1 && !is_literal(&args[0]) {
                if let Some(t) = num_ty(&infer(&args[0], ctx)) {
                    if !t.is_float() {
                        let inner = std::mem::replace(&mut args[0], Expr::Int(0));
                        args[0] = Expr::Cast(Box::new(inner), Type::Double);
                    }
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
                                    infer(arg, ctx) != VType::Str
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
        // `Color.Red` (a field on an enum name) is an enum variant → the path
        // `Color::Red`, not a value field access.
        Expr::Field(inner, variant)
            if matches!(&**inner, Expr::Ident(n) if ctx.enums.contains(n)) =>
        {
            if let Expr::Ident(n) = &**inner {
                *e = Expr::ConstRef(format!("{}::{}", n, variant));
            }
        }
        Expr::Deref(inner) | Expr::MutRef(inner) | Expr::Ref(inner) | Expr::Cast(inner, _)
        | Expr::Field(inner, _) | Expr::TupleIndex(inner, _) => resolve_expr(inner, ctx),
        // A closure anywhere except a method argument: its type has no name,
        // so it can't be stored in a variable, returned, or passed to a VBR
        // function. (Method arguments are consumed before reaching here.)
        Expr::Closure { body, .. } => {
            ctx.diags.error_once(
                "closure-value",
                "A closure (`|x| …`) can't be stored in a variable or passed to a \
                 function — its type has no name you (or VBR) could write. Use it \
                 directly as a method argument (`v.filter(|x| x > 2)`), or give the \
                 logic a name with a `Function`.",
            );
            resolve_expr(body, ctx);
        }
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
                     error here with `Match` over `Ok`/`Err`.",
                );
            }
        }
        Expr::Tuple(elems) | Expr::List(elems) => {
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
        // Inline Rust/Python are opaque — no resolution.
        Expr::InlineRust(_) | Expr::InlinePython { .. } => {}
        Expr::Not(inner) => resolve_expr(inner, ctx),
        Expr::Await(inner) => resolve_expr(inner, ctx),
        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Str(_) => {}
    }
}

// ---- Iterator chains -------------------------------------------------------
//
// `nums.filter(|x| x > 2).map(|x| x * x).collect()` — the links VBR
// understands on a `Vec`/fixed array. The chain root iterates by reference
// and then makes items owned: `.iter().copied()` when the element is a Copy
// primitive (free), `.iter().cloned()` when it owns data (a real copy — the
// same explicit-cost trade as `.clone()`). Closure parameters are bound to
// the element type while their body is resolved, so the usual coercions
// apply inside; unknown methods still pass through verbatim to rustc.

/// Is this method (with these arguments) an iterator link VBR understands?
/// `min`/`max` with an argument are the numeric receiver-typed methods
/// (`n.min(other)`), not the iterator consumers, so argument shape decides.
fn is_iter_adapter(m: &str, args: &[Expr]) -> bool {
    match m {
        // Closure-taking links and consumers.
        "filter" | "map" | "any" | "all" | "find" | "position" => {
            matches!(args.first(), Some(Expr::Closure { .. }))
        }
        // Numeric-argument links.
        "take" | "skip" => args.len() == 1,
        // No-argument links and consumers.
        "rev" | "enumerate" | "count" | "sum" | "collect" | "min" | "max" => args.is_empty(),
        _ => false,
    }
}

/// Is `e` itself an iterator link (so a method call on it continues a chain)?
fn is_iter_chain(e: &Expr) -> bool {
    matches!(e, Expr::MethodCall { method, args, .. } if is_iter_adapter(&snake(method), args))
}

/// A Copy element iterates with `.copied()` (and `filter` can destructure its
/// `&T` as `|&x|`); everything owned — `String`, structs, collections — is
/// `.cloned()` instead.
fn elem_is_copy(v: &VType) -> bool {
    matches!(v, VType::Decl(DeclType::Plain(t)) if !matches!(t, Type::Text))
}

/// Resolve one iterator link (recursing down the chain first). Returns the
/// element type flowing *out* of this link, or `None` when the chain root
/// isn't a sequence — then the caller falls back to normal resolution and
/// nothing has been rewritten.
fn resolve_iter_chain(e: &mut Expr, ctx: &mut Ctx) -> Option<VType> {
    let Expr::MethodCall { recv, method, args } = e else {
        return None;
    };
    let m = snake(method);
    // The element type flowing IN: from the link below, or the chain root.
    let elem = if is_iter_chain(recv) {
        resolve_iter_chain(recv, ctx)?
    } else {
        resolve_expr(recv, ctx);
        let elem = match infer(recv, ctx) {
            VType::Decl(DeclType::Vec(t)) => VType::Decl(*t),
            VType::Decl(DeclType::Array(t, _)) => vt(t),
            _ => return None, // not a sequence — not ours
        };
        let owning = if elem_is_copy(&elem) { "copied" } else { "cloned" };
        let inner = std::mem::replace(&mut **recv, Expr::Int(0));
        **recv = mcall(mcall(inner, "iter", vec![]), owning, vec![]);
        elem
    };
    // This link's arguments.
    let mut map_out = VType::Unknown;
    match m.as_str() {
        // These hand the closure a `&T`: a Copy element destructures it
        // (`|&x| …`); an owned one keeps `|x|` and derefs uses in the body.
        "filter" | "find" => {
            let by_ref = elem_is_copy(&elem) || !matches!(elem, VType::Decl(_));
            resolve_iter_closure(args, &elem, !by_ref, by_ref, ctx);
        }
        // These receive the item by value (`position` included — unlike
        // `find`, its predicate takes `Self::Item`, not a reference).
        "map" | "any" | "all" | "position" => {
            map_out = resolve_iter_closure(args, &elem, false, false, ctx);
        }
        "take" | "skip" => {
            for a in args.iter_mut() {
                resolve_expr(a, ctx);
            }
        }
        _ => {}
    }
    // The element type flowing OUT, for the next link's closure.
    Some(match m.as_str() {
        "map" => map_out,
        "filter" | "take" | "skip" | "rev" => elem,
        // `enumerate` yields `(usize, T)`; consumers end the chain.
        _ => VType::Unknown,
    })
}

/// Bind an iterator closure's parameter to the element type, resolve its body
/// under that (scoped) binding, and return the body's type (for `map`).
/// `deref_uses` puts the parameter in the deref set (`&T` param, owned
/// element); `by_ref` emits the `|&x|` destructuring pattern instead.
fn resolve_iter_closure(
    args: &mut [Expr],
    elem: &VType,
    deref_uses: bool,
    by_ref: bool,
    ctx: &mut Ctx,
) -> VType {
    let Some(Expr::Closure { params, body, by_ref_params }) = args.first_mut() else {
        return VType::Unknown;
    };
    *by_ref_params = by_ref;
    // Scoped binding: shadow whatever the name means outside, restore after.
    let key = params.first().map(|p| snake(p));
    let mut prev: Option<Binding> = None;
    let mut deref_added = false;
    if let (Some(k), VType::Decl(d)) = (&key, elem) {
        prev = ctx.env.insert(k.clone(), Binding { ty: Some(d.clone()), borrowed: false });
        if deref_uses {
            deref_added = ctx.deref.insert(k.clone());
        }
    }
    resolve_expr(body, ctx);
    let params_owned: Vec<String> = params.clone();
    check_closure_captures(&params_owned, body, ctx);
    let out = infer(body, ctx);
    if let Some(k) = key {
        if deref_added {
            ctx.deref.remove(&k);
        }
        match prev {
            Some(b) => {
                ctx.env.insert(k, b);
            }
            None => {
                if matches!(elem, VType::Decl(_)) {
                    ctx.env.remove(&k);
                }
            }
        }
    }
    out
}

/// A closure may *read* the variables around it, never change them — Rust
/// would type such a closure `FnMut` and the borrow rules bite immediately
/// (mutating the very collection being iterated is the classic case). Reject
/// a body that calls a mutating method on a captured variable.
fn check_closure_captures(params: &[String], body: &Expr, ctx: &mut Ctx) {
    if let Some(name) = mutated_capture(body, params, ctx) {
        ctx.diags.error_once(
            &format!("closure-capture-{}", snake(&name)),
            format!(
                "A closure can only *read* the variables it captures — this one \
                 changes '{}'. Do the mutation in a `For Each` loop instead, and \
                 keep the closure a pure question about its item.",
                name
            ),
        );
    }
}

/// The first captured variable this expression mutates (a mutating method
/// call whose receiver is rooted in an enclosing-scope name, not a closure
/// parameter), or `None`.
fn mutated_capture(e: &Expr, params: &[String], ctx: &Ctx) -> Option<String> {
    match e {
        Expr::MethodCall { recv, method, args } => {
            if crate::transpiler::is_mutating_method(&snake(method)) {
                if let Some(root) = place_root(recv) {
                    let key = snake(&root);
                    if !params.iter().any(|p| snake(p) == key) && ctx.env.contains_key(&key) {
                        return Some(root);
                    }
                }
            }
            mutated_capture(recv, params, ctx)
                .or_else(|| args.iter().find_map(|a| mutated_capture(a, params, ctx)))
        }
        Expr::Binary { lhs, rhs, .. } => mutated_capture(lhs, params, ctx)
            .or_else(|| mutated_capture(rhs, params, ctx)),
        Expr::Call { args, .. } | Expr::Tuple(args) => {
            args.iter().find_map(|a| mutated_capture(a, params, ctx))
        }
        Expr::Not(inner)
        | Expr::Deref(inner)
        | Expr::Ref(inner)
        | Expr::MutRef(inner)
        | Expr::Cast(inner, _)
        | Expr::Field(inner, _)
        | Expr::Try(inner)
        | Expr::TupleIndex(inner, _)
        | Expr::Closure { body: inner, .. } => mutated_capture(inner, params, ctx),
        Expr::Index(inner, idx) => mutated_capture(inner, params, ctx)
            .or_else(|| mutated_capture(idx, params, ctx)),
        _ => None,
    }
}

/// The variable at the root of a place expression (`seen` in `seen`,
/// `state.log` in… the root ident), if there is one.
fn place_root(e: &Expr) -> Option<String> {
    match e {
        Expr::Ident(n) => Some(n.clone()),
        Expr::Field(inner, _) | Expr::Index(inner, _) | Expr::Deref(inner) => place_root(inner),
        _ => None,
    }
}

/// Best-effort type inference. `Unknown` whenever we can't be sure — callers
/// only act on a confidently-known result.
fn infer(e: &Expr, ctx: &Ctx) -> VType {
    match e {
        Expr::Int(_) => vt(Type::Integer),
        Expr::Float(_) => vt(Type::Double),
        Expr::Bool(_) => vt(Type::Boolean),
        Expr::Str(_) => VType::Str,
        Expr::Await(inner) => infer(inner, ctx),
        Expr::Ident(name) if ctx.is_str_param(name) => VType::Str,
        Expr::Ident(name) => match ctx.binding(name).and_then(|b| b.ty.clone()) {
            Some(ty) => VType::Decl(ty),
            None => VType::Unknown,
        },
        Expr::Deref(inner) => infer(inner, ctx),
        Expr::Cast(_, ty) => vt(*ty),
        // `?` unwraps a Result/Option to its payload; we don't track that yet.
        Expr::Try(_) => VType::Unknown,
        Expr::MutRef(_) | Expr::Ref(_) => VType::Unknown,
        // `p.field` — the field's declared type, when the receiver is a known
        // struct (including `Me` inside a method).
        Expr::Field(recv, field) => match infer(recv, ctx) {
            VType::Decl(DeclType::Named(s)) => ctx
                .structs
                .get(&s)
                .and_then(|fields| fields.get(&snake(field)))
                .map_or(VType::Unknown, |t| VType::Decl(t.clone())),
            _ => VType::Unknown,
        },
        // `v(i)` / `arr(i)` — the element type. Indexing a 2-D array once
        // yields its row (an inner array), so `g(r)(c)` infers through.
        Expr::Index(inner, _) => match infer(inner, ctx) {
            VType::Decl(DeclType::Vec(t)) => VType::Decl(*t),
            VType::Decl(DeclType::Array(t, _)) => vt(t),
            VType::Decl(DeclType::Array2D(t, _, c)) => VType::Decl(DeclType::Array(t, c)),
            _ => VType::Unknown,
        },
        // `t.0` — the tuple element's type.
        Expr::TupleIndex(inner, i) => match infer(inner, ctx) {
            VType::Decl(DeclType::Tuple(ts)) => {
                ts.get(*i).map_or(VType::Unknown, |t| VType::Decl(t.clone()))
            }
            _ => VType::Unknown,
        },
        // An inline list `[a, b, …]` → `Vec<T>`, with T from the first element
        // (a bare String element is owned, so a `Text` list is `Vec<String>`).
        Expr::List(elems) => match elems.first().map(|e| infer(e, ctx)) {
            Some(VType::Decl(dt)) => VType::Decl(DeclType::Vec(Box::new(dt))),
            Some(VType::Str) => VType::Decl(DeclType::Vec(Box::new(DeclType::Plain(Type::Text)))),
            _ => VType::Unknown,
        },
        // Struct/tuple literals, const refs, closures: not tracked yet.
        Expr::StructLit { .. }
        | Expr::ConstRef(_)
        | Expr::Closure { .. }
        | Expr::Tuple(_)
        | Expr::InlineRust(_)
        | Expr::InlinePython { .. } => VType::Unknown,
        Expr::Not(_) => vt(Type::Boolean),
        Expr::Binary { op, lhs, rhs } => match op {
            BinOp::Concat => vt(Type::Text),
            BinOp::Pow => vt(Type::Double),
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                vt(Type::Boolean)
            }
            BinOp::And | BinOp::Or | BinOp::Xor => vt(Type::Boolean),
            _ => {
                // Arithmetic: the widened type of the two operands.
                match (num_ty(&infer(lhs, ctx)), num_ty(&infer(rhs, ctx))) {
                    (Some(a), Some(b)) => match widen(a, b) {
                        Some(t) => vt(t),
                        None => VType::Usize,
                    },
                    _ => VType::Unknown,
                }
            }
        },
        Expr::Call { name, .. } => builtin_vtype(name).unwrap_or_else(|| {
            // Not a builtin? Use the user function's declared return type.
            match ctx.fns.get(&snake(name)).and_then(|s| s.ret.clone()) {
                Some(ty) => VType::Decl(ty),
                None => VType::Unknown,
            }
        }),
        // Rust methods pass through verbatim; this curated table just tells the
        // coercion logic what the common ones *return*, so e.g. assigning
        // `s.trim()` (a `&str`) to a String still gets its `.to_string()`.
        Expr::MethodCall { recv, method, .. } => {
            let m = snake(method);
            // Numeric methods that yield the receiver's own type (`abs`, `min`,
            // `sqrt`, `floor`, …) — infer through to the receiver so the numeric
            // type, and any `as` casts that depend on it, stay correct.
            if is_receiver_typed_method(&m) {
                infer(recv, ctx)
            } else {
                method_vtype(&m)
            }
        }
    }
}

/// Numeric methods whose result has the same type as their receiver — `n.abs()`
/// is `i64` when `n` is, `x.sqrt()` is `f64` when `x` is. (Float-only methods
/// like `sqrt` require a float receiver anyway, so passing the type through is
/// always right for code that compiles.)
fn is_receiver_typed_method(m: &str) -> bool {
    matches!(
        m,
        // Same type as receiver, int or float.
        "abs" | "min" | "max" | "clamp" | "signum" | "pow" | "rem_euclid" | "div_euclid"
        // Float results (receiver is a float, so its type carries through).
            | "sqrt" | "cbrt" | "floor" | "ceil" | "round" | "trunc" | "fract"
            | "powi" | "powf" | "hypot" | "recip"
            | "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "atan2"
            | "ln" | "log10" | "log2" | "exp"
    )
}

/// The return type of a known Rust method, by name — feeds the same coercion
/// glue VB functions use. Strings first; extend with Vec/number rows as needed.
/// Unknown methods still pass through to Rust untouched; they just don't get
/// the smooth auto-coercion (rustc is the backstop).
fn method_vtype(m: &str) -> VType {
    match m {
        // Borrowed `&str` slices — need `.to_string()` to land in a String slot.
        "trim" | "trim_start" | "trim_end" | "trim_matches" => VType::Str,
        // Owned `String` results — already own their value.
        "to_uppercase" | "to_lowercase" | "to_ascii_uppercase" | "to_ascii_lowercase"
        | "replace" | "replacen" | "repeat" | "to_string" | "concat" | "join" => vt(Type::Text),
        // `usize` — counts and lengths (drive `as` casts in comparisons).
        "len" | "count" | "capacity" => VType::Usize,
        // Predicates — string and numeric.
        "is_empty" | "contains" | "starts_with" | "ends_with" | "eq_ignore_ascii_case"
        | "is_nan" | "is_finite" | "is_infinite" | "is_sign_positive" | "is_sign_negative"
        | "is_power_of_two" => vt(Type::Boolean),
        // Iterators, parses, and anything else: leave to Rust (no coercion).
        _ => VType::Unknown,
    }
}

// ---- DataFrame column formulas -------------------------------------------------
//
// The argument of a DataFrame transform (`Filter`, `WithColumn`) is a *column
// formula*: it reads like an Excel array formula and applies down the whole
// column. `lower_formula` rewrites an ordinary VBR expression into the polars
// expression tree it means — `col(...)` / `lit(...)` / `when/then/otherwise` and
// comparison/logical methods — which the emitter then renders verbatim.

/// Is `e` a DataFrame-valued expression? A variable declared `As DataFrame`, a
/// `DataFrame.ReadCsv(...)`-style constructor, or a transform chained off one.
fn is_df_expr(e: &Expr, ctx: &Ctx) -> bool {
    match e {
        Expr::Ident(n) => ctx.struct_of(n) == Some("DataFrame"),
        Expr::MethodCall { recv, .. } => {
            matches!(&**recv, Expr::Ident(n) if n == "DataFrame") || is_df_expr(recv, ctx)
        }
        _ => false,
    }
}

/// In a column formula, a name you've `Dim`'d is a *value* (`lit`); any other bare
/// name is a *column* (`col`). This tells the two apart.
fn is_value_var(name: &str, ctx: &Ctx) -> bool {
    match ctx.binding(name) {
        Some(b) => b.ty.is_some() && ctx.struct_of(name) != Some("DataFrame"),
        None => false,
    }
}

/// Rewrite a VBR expression in column-formula context into polars expressions.
fn lower_formula(e: &mut Expr, ctx: &Ctx) {
    match e {
        // A bare name: a `Dim`'d value → `lit(v)`; otherwise a column → `col("name")`.
        Expr::Ident(name) => {
            *e = if is_value_var(name, ctx) {
                lit_of(Expr::Ident(name.clone()))
            } else {
                call_expr("col", vec![Expr::Str(name.clone())])
            };
        }
        // Literals are values.
        Expr::Str(_) | Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) => {
            let v = std::mem::replace(e, Expr::Int(0));
            *e = lit_of(v);
        }
        // `IsNull(x)` → `x.is_null()` — nulls appear where a LeftJoin/OuterJoin
        // found no matching key; this is the mask that finds (or, with `Not`,
        // removes) those rows.
        Expr::Call { name, args } if name.eq_ignore_ascii_case("IsNull") && args.len() == 1 => {
            let mut inner = args.drain(..).next().unwrap();
            lower_formula(&mut inner, ctx);
            *e = mcall(inner, "is_null", vec![]);
        }
        // `Col(x)` / a backtick name → `col(x)`; the argument is a plain value.
        Expr::Call { name, args } if name == "Col" => {
            let arg = args.drain(..).next().unwrap_or_else(|| Expr::Str(String::new()));
            *e = call_expr("col", vec![arg]);
        }
        // `IIf(c, t, e)` → `when(c).then(t).otherwise(e)`.
        Expr::Call { name, args } if name.eq_ignore_ascii_case("IIf") && args.len() == 3 => {
            let mut drain = args.drain(..);
            let mut c = drain.next().unwrap();
            let mut t = drain.next().unwrap();
            let mut el = drain.next().unwrap();
            drop(drain);
            lower_formula(&mut c, ctx);
            lower_formula(&mut t, ctx);
            lower_formula(&mut el, ctx);
            let w = call_expr("when", vec![c]);
            let then = mcall(w, "then", vec![t]);
            *e = mcall(then, "otherwise", vec![el]);
        }
        Expr::Binary { op, lhs, rhs } => {
            lower_formula(lhs, ctx);
            lower_formula(rhs, ctx);
            // Comparisons and logical ops become methods (polars doesn't overload
            // `>`/`&&`); arithmetic (`+ - * /`) stays as operators (polars does).
            let method = match op {
                BinOp::Gt => Some("gt"),
                BinOp::Lt => Some("lt"),
                BinOp::Ge => Some("gt_eq"),
                BinOp::Le => Some("lt_eq"),
                BinOp::Eq => Some("eq"),
                BinOp::Ne => Some("neq"),
                BinOp::And => Some("and"),
                BinOp::Or => Some("or"),
                _ => None,
            };
            if let Some(m) = method {
                let l = std::mem::replace(&mut **lhs, Expr::Int(0));
                let r = std::mem::replace(&mut **rhs, Expr::Int(0));
                *e = mcall(l, m, vec![r]);
            }
        }
        Expr::Not(inner) => {
            lower_formula(inner, ctx);
            let i = std::mem::replace(&mut **inner, Expr::Int(0));
            *e = mcall(i, "not", vec![]);
        }
        _ => {}
    }
}

/// Lower one `Agg(…)` argument. An aggregation call — `Sum(x)`, `Mean(x)`,
/// `Min(x)`, `Max(x)`, `Count(x)` — lowers its inner *formula* and applies the
/// polars aggregation method; a bare formula passes through `lower_formula`.
fn lower_agg(e: &mut Expr, ctx: &Ctx) {
    if let Expr::Call { name, args } = e {
        let agg = ["sum", "mean", "min", "max", "count"]
            .iter()
            .find(|a| name.eq_ignore_ascii_case(a))
            .copied();
        if let (Some(m), 1) = (agg, args.len()) {
            let mut inner = args.drain(..).next().unwrap();
            lower_formula(&mut inner, ctx);
            *e = mcall(inner, m, vec![]);
            return;
        }
    }
    lower_formula(e, ctx);
}

fn call_expr(name: &str, args: Vec<Expr>) -> Expr {
    Expr::Call { name: name.to_string(), args }
}
fn mcall(recv: Expr, method: &str, args: Vec<Expr>) -> Expr {
    Expr::MethodCall { recv: Box::new(recv), method: method.to_string(), args }
}
fn lit_of(v: Expr) -> Expr {
    Expr::Call { name: "lit".to_string(), args: vec![v] }
}
