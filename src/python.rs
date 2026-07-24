//! VBR → Python backend (slice 1: pure computation).
//!
//! A second target beside the Rust transpiler. Where the Rust emitter lowers to
//! ownership-and-types Rust, this lowers the *same* parsed AST to idiomatic
//! Python. Slice 1 is deliberately the pure-computation core — `Function`/`Main`,
//! `Dim`, arithmetic, `If`/`For`/`Do`, `Debug.Print`, `&` concat, the maths
//! builtins — the constructs that translate cleanly. Everything Rust-specific
//! (inline `Rust`, Rust-only method chains, exotic `Match` patterns) is
//! target-native by nature: it can't cross, so it's flagged with a warning
//! rather than mistranslated. Structs, enums, `Match` and collections come in
//! later slices.
//!
//! Ground truth is `vbr run` (the Rust output): the golden test runs both and
//! diffs stdout, so this emitter matches Rust's *display* conventions — hence
//! the `_vb` helper (bool → `true`/`false`, whole floats without a trailing
//! `.0`) and a Rust-compatible `Round`.

use std::collections::HashSet;

use crate::ast::*;
use crate::transpiler::{convert_returns, rust_name};

/// The result of emitting Python for one VBR source.
pub struct PyProgram {
    /// The generated Python source.
    pub code: String,
    /// Constructs that couldn't cross to Python cleanly — surfaced as `⚠` notes
    /// so the user knows what was stubbed (the "we expect you to know that" made
    /// explicit).
    pub warnings: Vec<String>,
}

/// Emit Python for a whole parsed program.
pub fn emit_python(program: &Program) -> PyProgram {
    let mut e = Emitter::default();
    e.program(program);
    e.finish(program)
}

#[derive(Default)]
struct Emitter {
    body: String,
    warnings: Vec<String>,
    // Names that keep their exact casing when referenced: module constants
    // (`MAX_RETRIES`) rather than being lowercased like ordinary identifiers.
    const_names: HashSet<String>,
    // Prelude features, switched on as the body needs them.
    needs_vb: bool,
    needs_round: bool,
    needs_math: bool,
    needs_dataclass: bool,
}

impl Emitter {
    fn warn(&mut self, msg: impl Into<String>) {
        self.warnings.push(format!("⚠ {}", msg.into()));
    }

    fn line(&mut self, indent: usize, text: &str) {
        for _ in 0..indent {
            self.body.push_str("    ");
        }
        self.body.push_str(text);
        self.body.push('\n');
    }

    fn program(&mut self, program: &Program) {
        self.const_names = program.constants.iter().map(|c| c.name.clone()).collect();

        // Slice 2 lowers `Type`/methods/`Const`; enums land with `Match` (they're
        // one unit — every enum is used by matching on it). Surfaces stay Rust.
        if !program.enums.is_empty() {
            self.warn("`Enum` lowers to Python with `Match` in the next slice — skipped for now.");
        }
        if !program.windows.is_empty() || !program.screens.is_empty() || !program.pages.is_empty() {
            self.warn(
                "GUI/TUI/Web surfaces (`Window`/`Screen`/`Page`) are Rust-only — \
                 the Python target is for the core language.",
            );
        }

        // Module constants first (they keep their exact casing — see `const_names`).
        for c in &program.constants {
            let hint = self.type_hint(&DeclType::Plain(c.ty));
            let value = self.expr(&c.value);
            self.line(0, &format!("{}: {} = {}", c.name, hint, value));
        }

        // Each `Type` → a `@dataclass`, its methods nested inside it.
        for s in &program.structs {
            self.top_separator();
            self.dataclass(s, program);
        }

        // Free functions (methods were emitted with their struct).
        for func in &program.functions {
            if func.receiver.is_some() {
                continue;
            }
            self.top_separator();
            self.function(func, 0, false);
        }
    }

    /// One blank line before a top-level item, unless it's the first thing.
    fn top_separator(&mut self) {
        if !self.body.is_empty() {
            self.body.push('\n');
        }
    }

    /// A `Type` → a `@dataclass`: its fields as annotated attributes, then any
    /// `Function Struct.Method` bodies nested as methods.
    fn dataclass(&mut self, s: &StructDef, program: &Program) {
        self.needs_dataclass = true;
        self.line(0, "@dataclass");
        self.line(0, &format!("class {}:", s.name));
        for f in &s.fields {
            let hint = self.type_hint(&f.ty);
            self.line(1, &format!("{}: {}", rust_name(&f.name), hint));
        }
        let methods: Vec<&Function> = program
            .functions
            .iter()
            .filter(|f| f.receiver.as_deref() == Some(s.name.as_str()))
            .collect();
        for m in methods {
            self.body.push('\n');
            self.function(m, 1, true);
        }
    }

    /// A function (free, or a struct method at `indent` 1 with an implicit
    /// `self`). VB's assign-to-own-name return is desugared first (shared with
    /// the Rust backend).
    fn function(&mut self, func: &Function, indent: usize, is_method: bool) {
        let name = rust_name(&func.name);
        let mut params: Vec<String> = Vec::new();
        if is_method {
            params.push("self".to_string());
        }
        for p in &func.params {
            if p.mode == ParamMode::ByRef {
                self.warn(format!(
                    "`ByRef` parameter `{}` can't be emulated for a scalar in Python \
                     (assignment won't reach the caller) — passed by value.",
                    p.name
                ));
            }
            let hint = self.type_hint(&p.ty);
            params.push(format!("{}: {}", rust_name(&p.name), hint));
        }
        let ret = match &func.ret {
            Some(t) => format!(" -> {}", self.type_hint(t)),
            None => String::new(),
        };
        self.line(indent, &format!("def {}({}){}:", name, params.join(", "), ret));

        let mut body = func.body.clone();
        convert_returns(&mut body, &name);

        if body.iter().all(|s| matches!(s, Stmt::LineMark(_) | Stmt::Comment(_))) {
            self.line(indent + 1, "pass");
        }
        self.block(&body, indent + 1);
    }

    fn block(&mut self, stmts: &[Stmt], indent: usize) {
        for stmt in stmts {
            self.stmt(stmt, indent);
        }
    }

    fn stmt(&mut self, stmt: &Stmt, indent: usize) {
        match stmt {
            Stmt::LineMark(_) => {}
            Stmt::Comment(c) => self.line(indent, &format!("# {}", c.trim_start_matches(['\'', ' ']))),
            Stmt::Dim { name, ty, init, .. } => {
                let value = match init {
                    Some(e) => self.expr(e),
                    None => self.default_value(ty),
                };
                let hint = self.type_hint(ty);
                self.line(indent, &format!("{}: {} = {}", rust_name(name), hint, value));
            }
            Stmt::Assign { target, value, op } => {
                let t = self.expr(target);
                let v = self.expr(value);
                match op {
                    Some(o) => self.line(indent, &format!("{} {}= {}", t, self.bin_op(*o), v)),
                    None => self.line(indent, &format!("{} = {}", t, v)),
                }
            }
            Stmt::Return(Some(e)) => {
                let v = self.expr(e);
                self.line(indent, &format!("return {}", v));
            }
            Stmt::Return(None) => self.line(indent, "return"),
            Stmt::Expr(e) => {
                let v = self.expr(e);
                self.line(indent, &v);
            }
            Stmt::Print(e) => {
                let text = self.print_arg(e);
                self.line(indent, &format!("print({})", text));
            }
            Stmt::If { branches, else_body } => {
                for (i, (cond, body)) in branches.iter().enumerate() {
                    let kw = if i == 0 { "if" } else { "elif" };
                    let c = self.expr(cond);
                    self.line(indent, &format!("{} {}:", kw, c));
                    self.block_or_pass(body, indent + 1);
                }
                if let Some(body) = else_body {
                    self.line(indent, "else:");
                    self.block_or_pass(body, indent + 1);
                }
            }
            Stmt::For { var, from, to, step, body } => {
                let header = self.for_range(var, from, to, step.as_ref());
                self.line(indent, &header);
                self.block_or_pass(body, indent + 1);
            }
            Stmt::ForEach { var1, var2, iter, body } => {
                let it = self.expr(iter);
                let head = match var2 {
                    None => format!("for {} in {}:", rust_name(var1), it),
                    Some(v2) => {
                        // A two-variable For Each walks a map — collections slice.
                        format!("for {}, {} in {}.items():", rust_name(var1), rust_name(v2), it)
                    }
                };
                self.line(indent, &head);
                self.block_or_pass(body, indent + 1);
            }
            Stmt::DoLoop { cond, body } => self.do_loop(cond, body, indent),
            Stmt::Break => self.line(indent, "break"),
            Stmt::Continue => self.line(indent, "continue"),
            other => {
                self.warn(format!("`{}` doesn't lower to Python yet.", stmt_name(other)));
                self.line(indent, &format!("pass  # [VBR→Python] unsupported: {}", stmt_name(other)));
            }
        }
    }

    /// A `Do … Loop` in its four forms.
    fn do_loop(&mut self, cond: &Option<DoCond>, body: &[Stmt], indent: usize) {
        match cond {
            None => {
                self.line(indent, "while True:");
                self.block_or_pass(body, indent + 1);
            }
            Some(DoCond::PreWhile(c)) => {
                let c = self.expr(c);
                self.line(indent, &format!("while {}:", c));
                self.block_or_pass(body, indent + 1);
            }
            Some(DoCond::PreUntil(c)) => {
                let c = self.expr(c);
                self.line(indent, &format!("while not ({}):", c));
                self.block_or_pass(body, indent + 1);
            }
            Some(DoCond::PostWhile(c)) => {
                // Python has no do-while: loop forever, break when the guard fails.
                self.line(indent, "while True:");
                self.block_or_pass(body, indent + 1);
                let c = self.expr(c);
                self.line(indent + 1, &format!("if not ({}):", c));
                self.line(indent + 2, "break");
            }
            Some(DoCond::PostUntil(c)) => {
                self.line(indent, "while True:");
                self.block_or_pass(body, indent + 1);
                let c = self.expr(c);
                self.line(indent + 1, &format!("if {}:", c));
                self.line(indent + 2, "break");
            }
        }
    }

    fn block_or_pass(&mut self, stmts: &[Stmt], indent: usize) {
        if stmts.iter().all(|s| matches!(s, Stmt::LineMark(_))) {
            self.line(indent, "pass");
            return;
        }
        self.block(stmts, indent);
    }

    /// `For i = a To b [Step s]` → a Python `range`. `To` is inclusive, so the
    /// stop bound is nudged by one in the step's direction.
    fn for_range(&mut self, var: &str, from: &Expr, to: &Expr, step: Option<&Expr>) -> String {
        let var = rust_name(var);
        let from = self.expr(from);
        match step {
            None => {
                let stop = self.inclusive_stop(to, 1);
                format!("for {} in range({}, {}):", var, from, stop)
            }
            Some(Expr { kind: ExprKind::Int(s), .. }) => {
                let stop = self.inclusive_stop(to, *s);
                format!("for {} in range({}, {}, {}):", var, from, stop, s)
            }
            Some(other) => {
                self.warn(
                    "a non-literal `For … Step` can't fix its `range` direction at emit time — \
                     the generated bound assumes a positive step; check it.",
                );
                let stop = self.inclusive_stop(to, 1);
                let s = self.expr(other);
                format!("for {} in range({}, {}, {}):", var, from, stop, s)
            }
        }
    }

    /// The exclusive `range` stop for an inclusive `To` bound, given the step's
    /// sign: `+1` for an ascending loop, `-1` past the end for a descending one.
    /// A literal bound folds (`10` → `11`); anything else adds at runtime.
    fn inclusive_stop(&mut self, to: &Expr, step: i64) -> String {
        let delta = if step >= 0 { 1 } else { -1 };
        match &to.kind {
            ExprKind::Int(n) => (n + delta).to_string(),
            _ => {
                let e = self.expr(to);
                if delta >= 0 {
                    format!("{} + 1", e)
                } else {
                    format!("{} - 1", e)
                }
            }
        }
    }

    /// The argument to `print(...)` for a `Debug.Print`. A bare string stays a
    /// literal; a `&` chain becomes an f-string; anything else is `_vb`-wrapped
    /// so a number/bool prints exactly as Rust's `Display` would.
    fn print_arg(&mut self, e: &Expr) -> String {
        match &e.kind {
            ExprKind::Str(s) => py_str(s),
            ExprKind::Binary { op: BinOp::Concat, .. } => self.concat_fstring(e),
            _ => {
                self.needs_vb = true;
                format!("_vb({})", self.expr(e))
            }
        }
    }

    /// Flatten an `&` chain into one Python f-string. Literals fold into the
    /// text; every other operand becomes `{_vb(expr)}` (Rust-matching display).
    fn concat_fstring(&mut self, e: &Expr) -> String {
        let mut parts = String::new();
        self.walk_concat(e, &mut parts);
        format!("f\"{}\"", parts)
    }

    fn walk_concat(&mut self, e: &Expr, out: &mut String) {
        match &e.kind {
            ExprKind::Binary { op: BinOp::Concat, lhs, rhs } => {
                self.walk_concat(lhs, out);
                self.walk_concat(rhs, out);
            }
            ExprKind::Str(s) => out.push_str(&fstring_text(s)),
            _ => {
                self.needs_vb = true;
                let inner = self.expr(e);
                out.push_str(&format!("{{_vb({})}}", inner));
            }
        }
    }

    fn expr(&mut self, e: &Expr) -> String {
        match &e.kind {
            ExprKind::Int(n) => n.to_string(),
            ExprKind::Float(f) => py_float(*f),
            ExprKind::Bool(b) => if *b { "True".into() } else { "False".into() },
            ExprKind::Str(s) => py_str(s),
            ExprKind::Ident(name) if name == "None" => "None".into(),
            ExprKind::Ident(name) if name == "Me" => "self".into(),
            // A module constant keeps its exact casing; everything else lowercases.
            ExprKind::Ident(name) if self.const_names.contains(name) => name.clone(),
            ExprKind::Ident(name) => rust_name(name),
            ExprKind::Field(recv, field) => {
                let r = self.expr(recv);
                format!("{}.{}", r, rust_name(field))
            }
            ExprKind::StructLit { name, fields } => {
                let args: Vec<String> = fields
                    .iter()
                    .map(|(fname, val)| {
                        let v = self.expr(val);
                        format!("{}={}", rust_name(fname), v)
                    })
                    .collect();
                format!("{}({})", name, args.join(", "))
            }
            ExprKind::MethodCall { recv, method, args } => {
                let r = self.expr(recv);
                let a: Vec<String> = args.iter().map(|x| self.expr(x)).collect();
                format!("{}.{}({})", r, rust_name(method), a.join(", "))
            }
            ExprKind::Not(inner) => {
                let i = self.expr(inner);
                format!("not ({})", i)
            }
            ExprKind::Binary { op: BinOp::Concat, .. } => self.concat_fstring(e),
            ExprKind::Binary { op, lhs, rhs } => {
                let l = self.operand(lhs);
                let r = self.operand(rhs);
                if *op == BinOp::Pow {
                    format!("{} ** {}", l, r)
                } else {
                    if *op == BinOp::Div {
                        self.warn(
                            "`/` on integers truncates in Rust but is float division in Python — \
                             check whether you meant integer division (`//`).",
                        );
                    }
                    format!("{} {} {}", l, self.bin_op(*op), r)
                }
            }
            ExprKind::Call { name, args } => self.call(name, args),
            other => {
                self.warn(format!("`{}` doesn't lower to Python yet.", expr_name(other)));
                format!("None  # [VBR→Python] unsupported: {}", expr_name(other))
            }
        }
    }

    /// An operand inside a binary expression — parenthesised when it is itself a
    /// (non-concat) binary, so Python's precedence can't regroup our tree.
    fn operand(&mut self, e: &Expr) -> String {
        let s = self.expr(e);
        match &e.kind {
            ExprKind::Binary { op: BinOp::Concat, .. } => s,
            ExprKind::Binary { .. } => format!("({})", s),
            _ => s,
        }
    }

    /// A function call — a maths builtin maps to `math`/a helper, everything else
    /// passes straight through as a Python call.
    fn call(&mut self, name: &str, args: &[Expr]) -> String {
        let rendered: Vec<String> = args.iter().map(|a| self.expr(a)).collect();
        if args.len() == 1 {
            let a = &rendered[0];
            let math = |e: &mut Emitter, f: &str| {
                e.needs_math = true;
                format!("math.{}({})", f, a)
            };
            match name.to_ascii_lowercase().as_str() {
                "sqr" => return math(self, "sqrt"),
                "sin" => return math(self, "sin"),
                "cos" => return math(self, "cos"),
                "tan" => return math(self, "tan"),
                "exp" => return math(self, "exp"),
                "log" => return math(self, "log"), // natural log, like Rust's ln
                "int" => return math(self, "floor"),
                "abs" => return format!("abs({})", a),
                "round" => {
                    self.needs_round = true;
                    return format!("_vb_round({})", a);
                }
                _ => {}
            }
        }
        format!("{}({})", rust_name(name), rendered.join(", "))
    }

    fn bin_op(&self, op: BinOp) -> &'static str {
        match op {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::Pow => "**",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Gt => ">",
            BinOp::Le => "<=",
            BinOp::Ge => ">=",
            BinOp::And => "and",
            BinOp::Or => "or",
            BinOp::Xor => "^", // logical on Python bools
            BinOp::Concat => "+", // never reached (handled specially)
        }
    }

    fn type_hint(&mut self, ty: &DeclType) -> String {
        match ty {
            DeclType::Plain(Type::Integer | Type::Long | Type::LongLong | Type::Byte) => "int".into(),
            DeclType::Plain(Type::Single | Type::Double) => "float".into(),
            DeclType::Plain(Type::Boolean) => "bool".into(),
            DeclType::Plain(Type::Text) => "str".into(),
            DeclType::Vec(t) => format!("list[{}]", self.type_hint(t)),
            DeclType::Map(k, v) => format!("dict[{}, {}]", self.type_hint(k), self.type_hint(v)),
            DeclType::Option(t) => format!("{} | None", self.type_hint(t)),
            DeclType::Named(n) => n.clone(),
            other => {
                self.warn(format!("type `{}` has no Python hint yet.", other.vb()));
                "object".into()
            }
        }
    }

    fn default_value(&self, ty: &DeclType) -> String {
        match ty {
            DeclType::Plain(Type::Integer | Type::Long | Type::LongLong | Type::Byte) => "0".into(),
            DeclType::Plain(Type::Single | Type::Double) => "0.0".into(),
            DeclType::Plain(Type::Boolean) => "False".into(),
            DeclType::Plain(Type::Text) => "\"\"".into(),
            DeclType::Vec(_) => "[]".into(),
            DeclType::Map(_, _) => "{}".into(),
            _ => "None".into(),
        }
    }

    fn finish(self, program: &Program) -> PyProgram {
        let mut code = String::new();
        for c in &program.leading_comments {
            code.push_str(&format!("# {}\n", c.trim_start_matches(['\'', ' '])));
        }
        if !program.leading_comments.is_empty() {
            code.push('\n');
        }
        if self.needs_math {
            code.push_str("import math\n");
        }
        if self.needs_dataclass {
            code.push_str("from dataclasses import dataclass\n");
        }
        if self.needs_math || self.needs_dataclass {
            code.push('\n');
        }
        if self.needs_vb {
            code.push_str(VB_DISPLAY_HELPER);
            code.push('\n');
        }
        if self.needs_round {
            code.push_str(VB_ROUND_HELPER);
            code.push('\n');
        }
        code.push_str(&self.body);

        // Call `main` on run, mirroring the Rust entry point.
        if program.functions.iter().any(|f| f.receiver.is_none() && rust_name(&f.name) == "main") {
            code.push_str("\n\nif __name__ == \"__main__\":\n    main()\n");
        }

        PyProgram { code, warnings: self.warnings }
    }
}

/// Rust's `Display` for the values that print differently in Python: `bool`
/// lowercases, and a whole `float` drops its trailing `.0`. Keeps stdout
/// byte-identical to `vbr run` for the golden test.
const VB_DISPLAY_HELPER: &str = "\
def _vb(x):
    if isinstance(x, bool):
        return \"true\" if x else \"false\"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)
";

/// VB `Round` rounds half away from zero (as Rust's `f64::round` does), unlike
/// Python's banker's rounding — so `Round(2.5)` is `3`, matching `vbr run`.
const VB_ROUND_HELPER: &str = "\
import math as _math
def _vb_round(x):
    return _math.floor(x + 0.5) if x >= 0 else _math.ceil(x - 0.5)
";

/// A Python double-quoted string literal.
fn py_str(s: &str) -> String {
    format!("\"{}\"", py_escape(s))
}

/// The literal-text portion of an f-string: the usual escapes, plus `{`/`}`
/// doubled so they aren't read as interpolations.
fn fstring_text(s: &str) -> String {
    py_escape(s).replace('{', "{{").replace('}', "}}")
}

fn py_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

/// A float literal for Python source — always with a decimal point so it reads
/// as a float (`2` → `2.0`).
fn py_float(f: f64) -> String {
    let s = f.to_string();
    if s.contains('.') || s.contains('e') || s.contains('E') {
        s
    } else {
        format!("{}.0", s)
    }
}

fn stmt_name(s: &Stmt) -> &'static str {
    match s {
        Stmt::Dim { .. } => "Dim",
        Stmt::Set { .. } => "Set",
        Stmt::Assign { .. } => "assignment",
        Stmt::DestructureDim { .. } => "destructuring Dim",
        Stmt::HandleDim { .. } => "Rust handle",
        Stmt::Return(_) => "Return",
        Stmt::Expr(_) => "expression statement",
        Stmt::Print(_) => "Debug.Print",
        Stmt::Log(_, _) => "Log",
        Stmt::If { .. } => "If",
        Stmt::For { .. } => "For",
        Stmt::DoLoop { .. } => "Do…Loop",
        Stmt::Break => "Exit",
        Stmt::Continue => "Continue",
        Stmt::ForEach { .. } => "For Each",
        Stmt::Match { .. } => "Match",
        Stmt::Draw(_) => "Draw",
        Stmt::Assert(_) => "Assert",
        Stmt::Comment(_) => "comment",
        Stmt::LineMark(_) => "line mark",
    }
}

fn expr_name(e: &ExprKind) -> &'static str {
    match e {
        ExprKind::MethodCall { .. } => "method call",
        ExprKind::StructLit { .. } => "struct literal",
        ExprKind::Field(_, _) => "field access",
        ExprKind::ConstRef(_) => "constant",
        ExprKind::Closure { .. } => "closure",
        ExprKind::Tuple(_) => "tuple",
        ExprKind::List(_) => "list literal",
        ExprKind::TupleIndex(_, _) => "tuple index",
        ExprKind::Index(_, _) => "indexing",
        ExprKind::InlineRust(_) => "inline Rust",
        ExprKind::InlinePython { .. } => "inline Python",
        ExprKind::Await(_) => "Await",
        ExprKind::Try(_) => "error propagation (?)",
        _ => "expression",
    }
}
