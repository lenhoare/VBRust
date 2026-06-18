//! Abstract syntax tree for VBR.
//!
//! This is the vertical-slice subset of spec_01: functions, primitive `Dim`,
//! `Debug.Print`, arithmetic, `If`, and `For`. It will grow one slice at a time.

/// A VBR primitive type. Spec_01 is authoritative on the Rust mapping
/// (note: `Integer` is `i16` here, not `i32` as the stale README says).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Integer,  // i16
    Long,     // i32
    LongLong, // i64
    Single,   // f32
    Double,   // f64
    Boolean,  // bool
    Byte,     // u8
    Date,     // i64 — no date semantics, just a number
    Text,     // String — unknown size, ownership rules apply
}

impl Type {
    /// The Rust type this maps to (the owned form for `Text`).
    pub fn rust(self) -> &'static str {
        match self {
            Type::Integer => "i16",
            Type::Long => "i32",
            Type::LongLong => "i64",
            Type::Single => "f32",
            Type::Double => "f64",
            Type::Boolean => "bool",
            Type::Byte => "u8",
            Type::Date => "i64",
            Type::Text => "String",
        }
    }

    /// Fixed-size types copy freely; unknown-size types need explicit
    /// borrowing or cloning (the rule the `✘` ownership error explains).
    pub fn is_fixed_size(self) -> bool {
        !matches!(self, Type::Text)
    }

    /// Floating-point types — an integer literal assigned to one needs a `.0`.
    pub fn is_float(self) -> bool {
        matches!(self, Type::Single | Type::Double)
    }

    /// The VB-facing name, for diagnostics.
    pub fn vb_name(self) -> &'static str {
        match self {
            Type::Integer => "Integer",
            Type::Long => "Long",
            Type::LongLong => "LongLong",
            Type::Single => "Single",
            Type::Double => "Double",
            Type::Boolean => "Boolean",
            Type::Byte => "Byte",
            Type::Date => "Date",
            Type::Text => "String",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Program {
    pub leading_comments: Vec<String>,
    pub constants: Vec<ConstDef>,
    pub structs: Vec<StructDef>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct ConstDef {
    pub name: String,
    pub public: bool,
    pub ty: Type,
    pub value: Expr,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub public: bool,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub public: bool,
    pub ty: DeclType,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    /// `Some(struct)` for a method `Function Struct.Name()`, else a free function.
    pub receiver: Option<String>,
    pub params: Vec<Param>,
    pub ret: Option<RetType>,
    pub body: Vec<Stmt>,
    pub line: usize,
}

/// A function's return type. `Result<T>`/`Option<T>` are kept separate from the
/// plain `Type` so `Type` can stay `Copy` and simple.
#[derive(Debug, Clone, Copy)]
pub enum RetType {
    Plain(Type),
    Result(Type), // -> Result<T, String>
    Option(Type), // -> Option<T>
}

/// The declared type of a `Dim`/field — a plain type, a named struct, or a
/// growable collection.
#[derive(Debug, Clone)]
pub enum DeclType {
    Plain(Type),
    Named(String), // a user struct, e.g. Person
    Vec(Type),
    Map(Type, Type),
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub mode: ParamMode,
}

/// How a parameter is passed. `ByVal` copies/borrows-as-`&str`; `ByRef` is `&mut`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamMode {
    ByVal,
    ByRef,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Dim {
        name: String,
        ty: DeclType,
        init: Option<Expr>,
        line: usize,
    },
    /// `Set a = b` / `Set Mut a = b` — borrow instead of copy.
    Set {
        name: String,
        mutable: bool,
        value: Expr,
    },
    Assign {
        target: Expr,
        value: Expr,
    },
    /// `Return value` or `FunctionName = value` — both become a Rust return.
    Return(Option<Expr>),
    /// A bare expression used as a statement — chiefly a call for its effect,
    /// e.g. `AddTo(total, 5)`.
    Expr(Expr),
    Print(Expr),
    If {
        branches: Vec<(Expr, Vec<Stmt>)>,
        else_body: Option<Vec<Stmt>>,
    },
    For {
        var: String,
        from: Expr,
        to: Expr,
        step: Option<Expr>,
        body: Vec<Stmt>,
    },
    /// `Do … Loop` in its various forms → `while` / `loop`.
    DoLoop {
        cond: Option<DoCond>,
        body: Vec<Stmt>,
    },
    /// `Exit Do` / `Exit For` → `break`.
    Break,
    /// `Continue` → `continue` (a VBR extension over classic VBA).
    Continue,
    /// `For Each item In coll` / `For Each k, v In map` → `for … in &coll`.
    ForEach {
        var1: String,
        var2: Option<String>,
        iter: Expr,
        body: Vec<Stmt>,
    },
    /// `Select Case <scrutinee>` → `match`. Must have `Case Else` (the `_` arm).
    Select {
        scrutinee: Expr,
        arms: Vec<SelectArm>,
        else_body: Option<Vec<Stmt>>,
        line: usize,
    },
    Comment(String),
}

#[derive(Debug, Clone)]
pub struct SelectArm {
    /// One or more comma-separated patterns, joined with `|` in Rust.
    pub patterns: Vec<CasePattern>,
    pub body: Vec<Stmt>,
}

/// The condition attached to a `Do` loop, and where it sits.
#[derive(Debug, Clone)]
pub enum DoCond {
    PreWhile(Expr),  // Do While c … Loop      → while c
    PreUntil(Expr),  // Do Until c … Loop      → while !c
    PostWhile(Expr), // Do … Loop While c      → loop { …; if !c { break } }
    PostUntil(Expr), // Do … Loop Until c      → loop { …; if c { break } }
}

#[derive(Debug, Clone)]
pub enum CasePattern {
    Value(Expr),      // `Case 2`        → `2`
    Range(Expr, Expr), // `Case 4 To 10`  → `4..=10`
}

#[derive(Debug, Clone)]
pub enum Expr {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Ident(String),
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    /// A Rust-style method call, e.g. `b.clone()`.
    MethodCall {
        recv: Box<Expr>,
        method: String,
        args: Vec<Expr>,
    },
    /// A function call, e.g. `add(2, 3)`.
    Call {
        name: String,
        args: Vec<Expr>,
    },
    /// `*inner` — inserted by the resolver for uses of a `ByRef` parameter.
    Deref(Box<Expr>),
    /// `&mut inner` — inserted by the resolver for `ByRef` call arguments.
    MutRef(Box<Expr>),
    /// `inner as Type` — inserted by the resolver for numeric coercions VB
    /// would do silently but Rust requires to be explicit.
    Cast(Box<Expr>, Type),
    /// `inner?` — propagate the error/None of a Result/Option.
    Try(Box<Expr>),
    /// `Person { name: ..., age: ... }` — struct construction.
    StructLit {
        name: String,
        fields: Vec<(String, Expr)>,
    },
    /// `expr.field` — field access (no parentheses).
    Field(Box<Expr>, String),
    /// A reference to a module constant, rendered verbatim (SCREAMING_SNAKE_CASE).
    ConstRef(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,    // ^  (lowers to .powi()/.powf())
    Concat, // &
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}
