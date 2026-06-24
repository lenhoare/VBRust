//! Abstract syntax tree for VBR.
//!
//! This is the vertical-slice subset of spec_01: functions, primitive `Dim`,
//! `Debug.Print`, arithmetic, `If`, and `For`. It will grow one slice at a time.

/// A VBR primitive type. Spec_01 is authoritative on the Rust mapping
/// (note: `Integer` is `i16` here, not `i32` as the stale README says).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Integer,  // i32 — the Rust default int (not VBA's 16-bit)
    Long,     // i64
    LongLong, // i64 — same as Long now (kept for familiarity)
    Single,   // f32
    Double,   // f64
    Boolean,  // bool
    Byte,     // u8
    Text,     // String — unknown size, ownership rules apply
}

impl Type {
    /// The Rust type this maps to (the owned form for `Text`).
    pub fn rust(self) -> &'static str {
        match self {
            Type::Integer => "i32",
            Type::Long => "i64",
            Type::LongLong => "i64",
            Type::Single => "f32",
            Type::Double => "f64",
            Type::Boolean => "bool",
            Type::Byte => "u8",
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
            Type::Text => "String",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Program {
    pub leading_comments: Vec<String>,
    pub uses: Vec<UseDecl>,
    pub constants: Vec<ConstDef>,
    pub structs: Vec<StructDef>,
    pub functions: Vec<Function>,
    pub windows: Vec<Window>,
}

/// A GUI window: state (the source of truth), a view derived from it, and events
/// that update it. Compiles to an Iced application. (GUI slice 1.)
#[derive(Debug, Clone)]
pub struct Window {
    pub name: String,
    pub title: Option<String>,
    pub state: Vec<StateField>,
    pub view: ViewNode,
    pub events: Vec<GuiEvent>,
}

/// One field of a window's `State` block — a `Dim` with an initial value.
#[derive(Debug, Clone)]
pub struct StateField {
    pub name: String,
    pub ty: Type,
    pub init: Expr,
}

/// A node in the view tree.
#[derive(Debug, Clone)]
pub enum ViewNode {
    Column(Vec<ViewNode>),
    Row(Vec<ViewNode>),
    Text(Expr),
    Button {
        label: Expr,
        on_click: Option<String>,
    },
}

/// A window event handler — maps to an Iced `Message` variant + `update` arm.
#[derive(Debug, Clone)]
pub struct GuiEvent {
    pub name: String,
    pub body: Vec<Stmt>,
}

/// A `Use <crate> <version>` declaration → a Cargo `[dependencies]` line.
#[derive(Debug, Clone)]
pub struct UseDecl {
    pub crate_name: String,
    pub version: String,
    pub line: usize,
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
    /// `Public Function` — visible to other modules (emitted `pub fn`).
    pub public: bool,
    /// `Some(struct)` for a method `Function Struct.Name()`, else a free function.
    pub receiver: Option<String>,
    pub params: Vec<Param>,
    pub ret: Option<RetType>,
    pub body: Vec<Stmt>,
    pub line: usize,
}

/// A function's return type.
#[derive(Debug, Clone)]
pub enum RetType {
    Plain(Type),
    Named(String), // -> Person (an owned struct)
    Result(Type),  // -> Result<T, String>
    Option(Type),  // -> Option<T>
    Tuple(Vec<Type>),
}

/// The declared type of a `Dim`/field — a plain type, a named struct, a tuple,
/// or a growable collection.
#[derive(Debug, Clone)]
pub enum DeclType {
    Plain(Type),
    Named(String), // a user struct, e.g. Person
    Tuple(Vec<Type>),
    Vec(ElemType),
    Vec2D(Type),               // Dim grid(,) → Vec<Vec<T>>
    Array(Type, usize),        // Dim x(N)    → [T; N]
    Array2D(Type, usize, usize), // Dim grid(R, C) → [[T; C]; R]
    Map(ElemType, ElemType),
}

/// An element type inside a `Vec`/`HashMap` — a primitive or a named
/// struct/stdlib type (so `Vec<Person>` and `Vec<Json>` are expressible).
#[derive(Debug, Clone)]
pub enum ElemType {
    Plain(Type),
    Named(String),
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: DeclType,
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
        /// `Some(op)` for a compound assignment (`+=`, `-=`, `*=`, `/=`);
        /// `None` for a plain `=`.
        op: Option<BinOp>,
    },
    /// `Dim a, b = expr` — destructure a tuple into several bindings.
    DestructureDim {
        names: Vec<String>,
        value: Expr,
    },
    /// `Dim name = Rust … End Rust` — an opaque Rust handle. No `As` type: the
    /// value's type lives only in Rust (inferred there). VBR can pass it back
    /// into another inline-Rust block but never use it as a value.
    HandleDim {
        name: String,
        raw: String,
        line: usize,
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
    /// An optional `If` guard: `Case n If n < 0` → `n if n < 0 =>`.
    pub guard: Option<Expr>,
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
    /// `&inner` — inserted by the resolver for `ByVal` struct/collection args.
    Ref(Box<Expr>),
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
    /// `|x| body` — a closure, chiefly for iterator adapters.
    Closure {
        params: Vec<String>,
        body: Box<Expr>,
    },
    /// `(a, b, …)` — a tuple literal.
    Tuple(Vec<Expr>),
    /// `expr.0` — tuple element access.
    TupleIndex(Box<Expr>, usize),
    /// `expr[index]` — array/Vec indexing.
    Index(Box<Expr>, Box<Expr>),
    /// A `Rust … End Rust` block — raw Rust spliced in as a block expression.
    InlineRust(String),
    /// `Not inner` — logical negation → `!(inner)`.
    Not(Box<Expr>),
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
    And, // logical, short-circuit → &&
    Or,  // logical, short-circuit → ||
    Xor, // logical → ^ (on bool)
    Mod, // remainder → % (Rust rules, multiplicative precedence)
}
