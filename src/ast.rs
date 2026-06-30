//! Abstract syntax tree for VBR.
//!
//! This is the vertical-slice subset of spec_01: functions, primitive `Dim`,
//! `Debug.Print`, arithmetic, `If`, and `For`. It will grow one slice at a time.

/// A VBR primitive type. Spec_01 is authoritative on the Rust mapping
/// (Rust-first: `Integer` → `i32`, `Long` → `i64` — not VBA's 16/32-bit widths).
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
    /// A text entry box bound to a `String` state field. `on_input` names the
    /// event fired on each keystroke (which receives the new text).
    TextInput {
        placeholder: Expr,
        value: String,
        on_input: Option<String>,
    },
    /// A checkbox bound to a `Boolean` state field. `on_toggle` names the event
    /// fired when ticked/unticked (which receives the new `bool`).
    Checkbox {
        label: Expr,
        value: String,
        on_toggle: Option<String>,
    },
    /// A slider over `min..=max` bound to a numeric state field. `on_change`
    /// names the event fired as it moves (receiving the new value); Iced requires
    /// it, so it is mandatory.
    Slider {
        min: Expr,
        max: Expr,
        value: String,
        on_change: String,
    },
    /// `Match <expr>` inside a view — each arm produces the widget(s) to show.
    /// Lowers to a Rust `match` whose arms each yield an `Element`.
    Match {
        scrutinee: Expr,
        arms: Vec<ViewArm>,
    },
    /// `If <cond> Then … [ElseIf …] [Else …] End If` inside a view — show
    /// different widget(s) by condition. Lowers to a Rust `if`/`else` whose
    /// branches each yield an `Element` (a missing `Else` shows nothing).
    If {
        branches: Vec<(Expr, Vec<ViewNode>)>,
        else_body: Option<Vec<ViewNode>>,
    },
}

/// One arm of a view `Match`: a pattern (raw Rust, as in a statement `Match`)
/// and the widget(s) it shows.
#[derive(Debug, Clone)]
pub struct ViewArm {
    pub pattern: String,
    pub guard: Option<Expr>,
    pub body: Vec<ViewNode>,
}

/// A window event handler — maps to an Iced `Message` variant + `update` arm.
/// `params` carry data from the widget (e.g. a `TextInput`'s new text), so the
/// variant becomes `Name(T, …)` and the arm binds them.
#[derive(Debug, Clone)]
pub struct GuiEvent {
    pub name: String,
    pub params: Vec<Param>,
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
    pub ret: Option<DeclType>,
    pub body: Vec<Stmt>,
    pub line: usize,
}

/// The one recursive type expression — used wherever a type is written: `Dim`,
/// field, parameter, and return. `Result`/`Option`/`Vec`/`Map`/`Tuple` nest
/// freely (e.g. `Result<Vec<String>>`). Arrays stay special: fixed size with a
/// primitive element, as they always were in VB.
#[derive(Debug, Clone)]
pub enum DeclType {
    Plain(Type),
    Named(String), // a user struct/stdlib type, e.g. Person, Json
    Vec(Box<DeclType>),
    Map(Box<DeclType>, Box<DeclType>),
    Result(Box<DeclType>), // → Result<T, String>
    Option(Box<DeclType>), // → Option<T>
    Tuple(Vec<DeclType>),
    Array(Type, usize),          // Dim x(N)      → [T; N]
    Array2D(Type, usize, usize), // Dim grid(R, C) → [[T; C]; R]
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
    /// `Match <scrutinee>` … `End Match` → Rust `match`. Each arm is
    /// `pattern => body`; exhaustiveness is left to rustc (no forced catch-all).
    Match {
        scrutinee: Expr,
        arms: Vec<MatchArm>,
        line: usize,
    },
    Comment(String),
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    /// The pattern, captured as raw Rust text and emitted verbatim — so the full
    /// pattern grammar (`Ok(n)`, `1 | 2`, `1..=10`, `Point { x, y }`, `_`) is
    /// available. Name bindings should be written snake_case (Rust style).
    pub pattern: String,
    /// An optional `If` guard: `n If n < 0 =>` → `n if n < 0 =>`.
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
    /// `Await inner` — only valid inside a Window event. The GUI codegen splits
    /// the event around it; it never reaches normal expression rendering.
    Await(Box<Expr>),
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
