//! A target-neutral typing pass — the first bone of the shared, desugared
//! "typed AST" that the Rust resolver, the Python backend and the new C backend
//! all want. It walks the parsed AST (after the shared `convert_returns`
//! desugaring the backends also apply) and records the resolved [`DeclType`] of
//! every expression into a span-keyed table.
//!
//! Unlike the resolver's own inference, this does **no** Rust-specific rewriting
//! (`.to_owned()`, `as` casts, derefs) and raises no diagnostics — it only
//! answers "what type is this expression?", which the C backend needs to declare
//! variables and pick the right print/format helpers. The resolver's `infer`
//! will fold into this once the shared IR is lifted out; for now this is the
//! neutral seed, kept small and honest (slice-1 forms only).

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::pattern::{self, Pat};
use crate::span::Span;
use crate::transpiler::convert_returns;

/// The resolved type of every expression that carries one, keyed by its source
/// span. Absent spans (forms this slice doesn't type yet) are simply not present.
pub type TypeTable = HashMap<Span, DeclType>;

/// Type every function body in `program`, returning the span → type table.
pub fn type_program(program: &Program) -> TypeTable {
    let structs = program
        .structs
        .iter()
        .map(|s| {
            let fields = s
                .fields
                .iter()
                .map(|f| (f.name.to_ascii_lowercase(), f.ty.clone()))
                .collect();
            (s.name.to_ascii_lowercase(), fields)
        })
        .collect();
    let methods = program
        .functions
        .iter()
        .filter_map(|f| {
            let recv = f.receiver.as_ref()?;
            let ret = f.ret.clone().unwrap_or(DeclType::Plain(Type::Long));
            Some(((recv.to_ascii_lowercase(), f.name.to_ascii_lowercase()), ret))
        })
        .collect();
    let enums: HashSet<String> = program.enums.iter().map(|e| e.name.to_ascii_lowercase()).collect();
    let variant_payloads = program
        .enums
        .iter()
        .flat_map(|e| {
            e.variants.iter().map(move |v| {
                ((e.name.to_ascii_lowercase(), v.name.to_ascii_lowercase()), v.payload.clone())
            })
        })
        .collect();
    let mut typer = Typer {
        fns: program
            .functions
            .iter()
            .filter(|f| f.receiver.is_none())
            .filter_map(|f| f.ret.clone().map(|r| (f.name.to_ascii_lowercase(), r)))
            .collect(),
        consts: program
            .constants
            .iter()
            .map(|c| (c.name.to_ascii_lowercase(), c.ty))
            .collect(),
        structs,
        methods,
        enums,
        variant_payloads,
        env: HashMap::new(),
        table: HashMap::new(),
    };
    for f in &program.functions {
        typer.function(f);
    }
    typer.table
}

struct Typer {
    /// Free-function return types, for typing a `Call`.
    fns: HashMap<String, DeclType>,
    /// Module constants, by lowercased name.
    consts: HashMap<String, Type>,
    /// Struct fields: struct name → (field name → type), all lowercased.
    structs: HashMap<String, HashMap<String, DeclType>>,
    /// Method return types: (struct, method) → type, for a `recv.M()` call.
    methods: HashMap<(String, String), DeclType>,
    /// Enum type names (lowercased) — an `Enum.Variant` reference/construction is
    /// the enum's own type.
    enums: HashSet<String>,
    /// Variant payload types: (enum, variant) → field types, for binding a
    /// destructuring pattern's names.
    variant_payloads: HashMap<(String, String), Vec<DeclType>>,
    /// The current function's variables (params, `Dim`s, `For` counters).
    env: HashMap<String, DeclType>,
    table: TypeTable,
}

impl Typer {
    fn function(&mut self, func: &Function) {
        // VB has no block scope, so a flat per-function map matches the model.
        self.env.clear();
        for p in &func.params {
            self.env.insert(p.name.to_ascii_lowercase(), p.ty.clone());
        }
        // Inside a method, `Me` is the receiver struct — so `Me.field` infers.
        if let Some(recv) = &func.receiver {
            self.env.insert("me".to_string(), DeclType::Named(recv.clone()));
        }
        // Walk the same desugared body the backend emits (so spans line up).
        let mut body = func.body.clone();
        convert_returns(&mut body, &func.name);
        self.block(&body);
    }

    fn block(&mut self, stmts: &[Stmt]) {
        for s in stmts {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Dim { name, ty, init, .. } => {
                if let Some(e) = init {
                    self.infer(e);
                }
                self.env.insert(name.to_ascii_lowercase(), ty.clone());
            }
            Stmt::Assign { target, value, .. } => {
                self.infer(target);
                self.infer(value);
            }
            // `Dim (a, b) As (T, U) = value` — type the value's sub-expressions
            // and bind each name to its tuple-element type.
            Stmt::DestructureDim { names, ty, value } => {
                self.infer(value);
                if let Some(DeclType::Tuple(ts)) = ty {
                    for (n, t) in names.iter().zip(ts) {
                        self.env.insert(n.to_ascii_lowercase(), t.clone());
                    }
                }
            }
            Stmt::Print(e) | Stmt::Return(Some(e)) => {
                self.infer(e);
            }
            Stmt::Expr(e) => {
                self.infer(e);
            }
            Stmt::If { branches, else_body } => {
                for (cond, body) in branches {
                    self.infer(cond);
                    self.block(body);
                }
                if let Some(body) = else_body {
                    self.block(body);
                }
            }
            Stmt::For { var, from, to, step, body } => {
                self.infer(from);
                self.infer(to);
                if let Some(s) = step {
                    self.infer(s);
                }
                // A `For` counter over an integer range is a `Long`.
                self.env.insert(var.to_ascii_lowercase(), DeclType::Plain(Type::Long));
                self.block(body);
            }
            Stmt::DoLoop { cond, body } => {
                if let Some(
                    DoCond::PreWhile(e) | DoCond::PreUntil(e) | DoCond::PostWhile(e) | DoCond::PostUntil(e),
                ) = cond
                {
                    self.infer(e);
                }
                self.block(body);
            }
            // `For Each x In v` binds the element; the two-variable form binds a
            // map's key and value.
            Stmt::ForEach { var1, var2, iter, body } => {
                let ity = self.infer(iter);
                match (&ity, var2) {
                    (DeclType::Vec(elem), _) => {
                        self.env.insert(var1.to_ascii_lowercase(), (**elem).clone());
                    }
                    (DeclType::Map(k, v), Some(v2)) => {
                        self.env.insert(var1.to_ascii_lowercase(), (**k).clone());
                        self.env.insert(v2.to_ascii_lowercase(), (**v).clone());
                    }
                    (DeclType::Map(k, _), None) => {
                        self.env.insert(var1.to_ascii_lowercase(), (**k).clone());
                    }
                    _ => {
                        self.env.insert(var1.to_ascii_lowercase(), DeclType::Plain(Type::Long));
                    }
                }
                self.block(body);
            }
            Stmt::Match { scrutinee, arms, .. } => {
                let scrut_ty = self.infer(scrutinee);
                for arm in arms {
                    // Pattern bindings are scoped to the arm — snapshot + restore.
                    let saved = self.env.clone();
                    self.bind_pattern(&pattern::parse(&arm.pattern), &scrut_ty);
                    if let Some(g) = &arm.guard {
                        self.infer(g);
                    }
                    self.block(&arm.body);
                    self.env = saved;
                }
            }
            // `Destroy`/`Break`/`Continue`/`Comment`/`LineMark` carry no slice-1
            // expression to type; other statement kinds arrive in later slices.
            _ => {}
        }
    }

    /// Add a pattern's bindings to the environment: a bare `x` takes the
    /// scrutinee's type; a data-variant's names take their payload field types;
    /// `Some`/`Ok`/`Err` unwrap the Option/Result inner type first.
    fn bind_pattern(&mut self, pat: &Pat, scrut_ty: &DeclType) {
        match pat {
            Pat::Binding(x) => {
                self.env.insert(x.to_ascii_lowercase(), scrut_ty.clone());
            }
            Pat::Variant { enom, variant, binds } => {
                if let Some(payloads) =
                    self.variant_payloads.get(&(enom.to_ascii_lowercase(), variant.to_ascii_lowercase()))
                {
                    for (b, t) in binds.iter().zip(payloads.clone()) {
                        self.env.insert(b.to_ascii_lowercase(), t);
                    }
                }
            }
            Pat::Some(inner) | Pat::Ok(inner) => {
                if let DeclType::Option(t) | DeclType::Result(t, _) = scrut_ty {
                    self.bind_pattern(inner, t);
                }
            }
            Pat::Err(inner) => {
                if let DeclType::Result(_, e) = scrut_ty {
                    self.bind_pattern(inner, e);
                }
            }
            _ => {}
        }
    }

    /// Infer an expression's type, record it in the table, and return it.
    fn infer(&mut self, e: &Expr) -> DeclType {
        let ty = self.infer_kind(e);
        self.table.insert(e.span, ty.clone());
        ty
    }

    fn infer_kind(&mut self, e: &Expr) -> DeclType {
        match &e.kind {
            ExprKind::Int(_) => DeclType::Plain(Type::Long),
            ExprKind::Float(_) => DeclType::Plain(Type::Double),
            ExprKind::Bool(_) => DeclType::Plain(Type::Boolean),
            ExprKind::Str(_) => DeclType::Plain(Type::Text),
            ExprKind::Ident(name) => {
                let key = name.to_ascii_lowercase();
                if let Some(t) = self.env.get(&key) {
                    t.clone()
                } else if let Some(t) = self.consts.get(&key) {
                    DeclType::Plain(*t)
                } else {
                    DeclType::Plain(Type::Long)
                }
            }
            ExprKind::ConstRef(name) => self
                .consts
                .get(&name.to_ascii_lowercase())
                .map(|t| DeclType::Plain(*t))
                .unwrap_or(DeclType::Plain(Type::Long)),
            ExprKind::Binary { op, lhs, rhs } => {
                let l = self.infer(lhs);
                let r = self.infer(rhs);
                match op {
                    BinOp::Concat => DeclType::Plain(Type::Text),
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge | BinOp::And
                    | BinOp::Or | BinOp::Xor => DeclType::Plain(Type::Boolean),
                    // `^` is always floating-point (like Rust's `powf`); the rest
                    // widen their operands.
                    BinOp::Pow => DeclType::Plain(Type::Double),
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => join(&l, &r),
                }
            }
            ExprKind::Not(_) => DeclType::Plain(Type::Boolean),
            // `expr?` yields the unwrapped success value.
            ExprKind::Try(inner) => match self.infer(inner) {
                DeclType::Option(t) | DeclType::Result(t, _) => *t,
                _ => DeclType::Plain(Type::Long),
            },
            ExprKind::Call { name, args } => {
                for a in args {
                    self.infer(a);
                }
                if let Some(t) = builtin_return(name) {
                    DeclType::Plain(t)
                } else if let Some(t) = self.fns.get(&name.to_ascii_lowercase()) {
                    t.clone()
                } else {
                    DeclType::Plain(Type::Long)
                }
            }
            // `Person { name: …, age: … }` — the struct is its own named type.
            ExprKind::StructLit { name, fields } => {
                for (_, v) in fields {
                    self.infer(v);
                }
                DeclType::Named(name.clone())
            }
            // `Enum.Variant` — a C-like value or a unit data variant; either way
            // the enum's own type.
            ExprKind::Field(recv, _)
                if matches!(&recv.kind, ExprKind::Ident(n) if self.enums.contains(&n.to_ascii_lowercase())) =>
            {
                if let ExprKind::Ident(n) = &recv.kind {
                    DeclType::Named(n.clone())
                } else {
                    unreachable!()
                }
            }
            // `recv.field` — look the field up in the receiver's struct.
            ExprKind::Field(recv, fname) => {
                let recv_ty = self.infer(recv);
                if let DeclType::Named(s) = recv_ty {
                    self.structs
                        .get(&s.to_ascii_lowercase())
                        .and_then(|fs| fs.get(&fname.to_ascii_lowercase()))
                        .cloned()
                        .unwrap_or(DeclType::Plain(Type::Long))
                } else {
                    DeclType::Plain(Type::Long)
                }
            }
            // `Enum.Variant(args)` — constructing a data variant yields the enum.
            // A standard-library namespace call (`FileSystem.Read(...)`) — its
            // declared return type (mostly `Result<…>`).
            ExprKind::MethodCall { recv, method, args }
                if matches!(&recv.kind,
                    ExprKind::Ident(n) if stdlib_return(&n.to_ascii_lowercase(), &method.to_ascii_lowercase()).is_some()) =>
            {
                for a in args {
                    self.infer(a);
                }
                match &recv.kind {
                    ExprKind::Ident(n) => {
                        stdlib_return(&n.to_ascii_lowercase(), &method.to_ascii_lowercase()).unwrap()
                    }
                    _ => unreachable!(),
                }
            }
            ExprKind::MethodCall { recv, args, .. }
                if matches!(&recv.kind, ExprKind::Ident(n) if self.enums.contains(&n.to_ascii_lowercase())) =>
            {
                for a in args {
                    self.infer(a);
                }
                match &recv.kind {
                    ExprKind::Ident(n) => DeclType::Named(n.clone()),
                    _ => unreachable!(),
                }
            }
            // `recv.Method(args)` — collections, methods, and `.Unwrap()`.
            ExprKind::MethodCall { recv, method, args } => {
                let recv_ty = self.infer(recv);
                for a in args {
                    self.infer(a);
                }
                let m = method.to_ascii_lowercase();
                match &recv_ty {
                    DeclType::Vec(elem) => match m.as_str() {
                        "sum" | "get" => (**elem).clone(),
                        "any" | "all" => DeclType::Plain(Type::Boolean),
                        "len" | "count" => DeclType::Plain(Type::Long),
                        // Adapters/`collect` keep the collection type (for chains).
                        _ => recv_ty.clone(),
                    },
                    DeclType::Map(_, v) => match m.as_str() {
                        "get" => (**v).clone(),
                        "contains_key" => DeclType::Plain(Type::Boolean),
                        "len" | "count" => DeclType::Plain(Type::Long),
                        _ => recv_ty.clone(),
                    },
                    DeclType::Named(s) => self
                        .methods
                        .get(&(s.to_ascii_lowercase(), m))
                        .cloned()
                        .unwrap_or(DeclType::Plain(Type::Long)),
                    // `.Unwrap()` passes its receiver's type through (an
                    // `Option<V>`/`Result<V>` is modelled as `V` this slice).
                    _ if m == "unwrap" => recv_ty.clone(),
                    _ => DeclType::Plain(Type::Long),
                }
            }
            // `[a, b, …]` — a Vec whose element type is the first item's.
            ExprKind::List(items) => {
                let elem = items.first().map(|i| self.infer(i)).unwrap_or(DeclType::Plain(Type::Long));
                for i in items.iter().skip(1) {
                    self.infer(i);
                }
                DeclType::Vec(Box::new(elem))
            }
            // `v[i]` — a Vec element or a Map value.
            ExprKind::Index(recv, idx) => {
                let rty = self.infer(recv);
                self.infer(idx);
                match rty {
                    DeclType::Vec(elem) => (*elem).clone(),
                    DeclType::Map(_, v) => (*v).clone(),
                    _ => DeclType::Plain(Type::Long),
                }
            }
            // Later-slice forms (method calls, fields, collections…) aren't typed
            // yet; a conservative `Long` keeps the C backend's formatter total.
            _ => DeclType::Plain(Type::Long),
        }
    }
}

/// The widened numeric type of arithmetic between two operand types — mirrors
/// `resolver::widen` (floats beat ints, `f64` beats `f32`, `Long` beats the
/// 32-bit-and-under tier). TODO(ir): this duplicates the resolver's numeric
/// join; both collapse into the shared typed IR once it's lifted out.
fn join(a: &DeclType, b: &DeclType) -> DeclType {
    fn rank(t: &DeclType) -> u8 {
        match t {
            DeclType::Plain(Type::Double) => 6,
            DeclType::Plain(Type::Single) => 5,
            DeclType::Plain(Type::Long | Type::LongLong) => 4,
            DeclType::Plain(Type::Integer | Type::Byte) => 2,
            _ => 0,
        }
    }
    let top = if rank(a) >= rank(b) { a } else { b };
    if rank(top) == 0 {
        a.clone()
    } else {
        top.clone()
    }
}

/// The declared return type of a standard-library **namespace** call
/// (`FileSystem.Read(...)`), or `None` if `ns` isn't a known namespace method.
/// Shared knowledge for the non-Rust backends (the C backend needs it to type a
/// `.Unwrap()`; Python lowers stdlib calls structurally and doesn't).
pub fn stdlib_return(ns: &str, method: &str) -> Option<DeclType> {
    let text = || DeclType::Plain(Type::Text);
    let unit = || DeclType::Tuple(Vec::new());
    // `Result<T>` shorthand — the error is always `String`.
    let res = |t: DeclType| DeclType::Result(Box::new(t), Box::new(DeclType::Plain(Type::Text)));
    Some(match (ns, method) {
        ("filesystem", "read") => res(text()),
        ("filesystem", "readlines") => res(DeclType::Vec(Box::new(text()))),
        ("filesystem", "exists" | "folderexists") => DeclType::Plain(Type::Boolean),
        (
            "filesystem",
            "write" | "append" | "delete" | "copy" | "movefile" | "createfolder" | "createfolderall"
            | "deletefolder" | "deletefolderall",
        ) => res(unit()),
        _ => return None,
    })
}

/// The result type of a known builtin (the maths/string functions in play this
/// slice), or `None` for a user function. Mirrors `resolver::builtin_vtype`.
fn builtin_return(name: &str) -> Option<Type> {
    Some(match name.to_ascii_lowercase().as_str() {
        "sqr" | "abs" | "int" | "round" | "sin" | "cos" | "tan" | "log" | "exp" | "val" => Type::Double,
        "ucase" | "lcase" | "replace" | "str" | "cstr" | "chr" | "left" | "right" | "mid" | "trim" => {
            Type::Text
        }
        "len" => Type::Long,
        _ => return None,
    })
}
