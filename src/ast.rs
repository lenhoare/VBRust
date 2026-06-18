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
}

impl Type {
    /// The Rust type this maps to.
    pub fn rust(self) -> &'static str {
        match self {
            Type::Long => "i32",
            Type::Integer => "i16",
            Type::Double => "f64",
            Type::Boolean => "bool",
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
    pub ret: Option<Type>,
    pub body: Vec<Stmt>,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Dim {
        name: String,
        ty: Type,
        init: Option<Expr>,
        #[allow(dead_code)] // used once Dim emits its own ownership diagnostics
        line: usize,
    },
    Assign {
        name: String,
        value: Expr,
    },
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
