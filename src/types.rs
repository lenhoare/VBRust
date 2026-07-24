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

use std::collections::HashMap;

use crate::ast::*;
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
            // `Destroy`/`Break`/`Continue`/`Comment`/`LineMark` carry no slice-1
            // expression to type; other statement kinds arrive in later slices.
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
            // `recv.Method(args)` — the method's declared return type.
            ExprKind::MethodCall { recv, method, args } => {
                let recv_ty = self.infer(recv);
                for a in args {
                    self.infer(a);
                }
                if let DeclType::Named(s) = recv_ty {
                    self.methods
                        .get(&(s.to_ascii_lowercase(), method.to_ascii_lowercase()))
                        .cloned()
                        .unwrap_or(DeclType::Plain(Type::Long))
                } else {
                    DeclType::Plain(Type::Long)
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
