//! Abstract syntax tree for VBR.
//!
//! This is the vertical-slice subset of spec_01: functions, primitive `Dim`,
//! `Debug.Print`, arithmetic, `If`, and `For`. It will grow one slice at a time.

/// A VBR primitive type. Spec_01 is authoritative on the Rust mapping
/// (note: `Integer` is `i16` here, not `i32` as the stale README says).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Long,    // i32
    Integer, // i16
    Double,  // f64
    Boolean, // bool
    Text,    // String — unknown size, ownership rules apply
}

impl Type {
    /// The Rust type this maps to (the owned form for `Text`).
    pub fn rust(self) -> &'static str {
        match self {
            Type::Long => "i32",
            Type::Integer => "i16",
            Type::Double => "f64",
            Type::Boolean => "bool",
            Type::Text => "String",
        }
    }

    /// Fixed-size types copy freely; unknown-size types need explicit
    /// borrowing or cloning (the rule the `✘` ownership error explains).
    pub fn is_fixed_size(self) -> bool {
        !matches!(self, Type::Text)
    }

    /// The VB-facing name, for diagnostics.
    pub fn vb_name(self) -> &'static str {
        match self {
            Type::Long => "Long",
            Type::Integer => "Integer",
            Type::Double => "Double",
            Type::Boolean => "Boolean",
            Type::Text => "String",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Program {
    pub leading_comments: Vec<String>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Option<Type>,
    pub body: Vec<Stmt>,
    pub line: usize,
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
        ty: Type,
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
        name: String,
        value: Expr,
    },
    /// `Return value` or `FunctionName = value` — both become a Rust return.
    Return(Option<Expr>),
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
    Comment(String),
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Concat, // &
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}
