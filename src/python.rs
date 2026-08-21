//! Bust → Python backend (slice 1: pure computation).
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
use crate::iter;
use crate::pattern::{self, Pat};
use crate::transpiler::{body_never_returns, convert_returns, rust_name};
use crate::types::{type_program, TypeTable};

/// The result of emitting Python for one Bust source.
pub struct PyProgram {
    /// The generated Python source.
    pub code: String,
    /// Constructs that couldn't cross to Python cleanly — surfaced as `⚠` notes
    /// so the user knows what was stubbed (the "we expect you to know that" made
    /// explicit).
    pub warnings: Vec<String>,
    /// Standard-library namespaces used (`FileSystem`, `Regex`, `Json`). When
    /// non-empty the program is a *project*: `main.py` imports the `vbrpy`
    /// package (the Python parallel of a `vbr runproject` build).
    pub stdlib_used: Vec<String>,
    /// pip requirements — one line each (`numpy==2.0`, `polars>=1.0`) — from
    /// `Use <module> <version>` declarations plus our own deps (polars for a
    /// DataFrame program). When non-empty a `requirements.txt` is written beside
    /// `main.py`, mirroring how a `Use`d crate lands in Cargo's `[dependencies]`.
    pub requirements: Vec<String>,
}

/// Standard-library namespaces/value-types the Python target supports so far
/// (mirrors the `vbrpy` package). Used as a call receiver (`FileSystem.Read`) or
/// a declared type (`As Json`).
const STDLIB_SUPPORTED: &[&str] = &[
    "FileSystem", "Regex", "Json", "Database", "DateTime", "Http", "DataFrame", "Shell", "Process",
];

/// Namespaces that exist in `vbr_stdlib` but aren't lowered to Python yet — so a
/// use gets a clear "later slice" warning rather than silently wrong output.
const STDLIB_PENDING: &[&str] = &[];

/// Emit Python for a whole parsed program.
pub fn emit_python(program: &Program) -> PyProgram {
    let mut e = Emitter { types: type_program(program), ..Default::default() };
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
    // Every enum type name, and the subset that carry data (a sum type). A
    // C-like enum lowers to `enum.Enum`; a data one to a base class + a
    // `@dataclass` per variant, so construction and patterns differ.
    enum_names: HashSet<String>,
    data_enums: HashSet<String>,
    // Per-`Match` counter, so the scrutinee temp (`_m0`) is unique and matches
    // can nest.
    match_counter: usize,
    // The shared typing pass (`types.rs`) — the single authority for "what type
    // is this expression?", keyed by span. Replaces a hand-rolled per-function
    // type map; drives `//` vs `/`, dict-vs-list `.insert`, and DataFrame detection.
    types: TypeTable,
    // Declared (non-DataFrame) variable *names*, for the one thing the type table
    // can't answer: in a DataFrame column formula, a bare name is a `lit(value)`
    // only if it's a known variable — an undeclared name is a `col("...")`, and
    // the table returns a fallback type for both. So this is scope, not inference.
    formula_vars: HashSet<String>,
    // The current function's return type — tells a `?` whether to propagate an
    // `Err` (Result) or a `None` (Option).
    current_ret: Option<DeclType>,
    // Temp counter for `?` hoisting (`_t0`).
    tmp_counter: usize,
    // The indentation at which a `?` in the expression currently being rendered
    // should hoist its temp + early-return. `Some` while a statement's own
    // expressions are being rendered; `None` means no statement context.
    hoist_at: Option<usize>,
    // Standard-library namespaces/types referenced (`FileSystem`, `Json`, …) —
    // these turn the output into a project that imports the `vbrpy` package.
    stdlib_used: std::collections::BTreeSet<String>,
    // External pip modules declared with `Use <module> <version>`. Referencing
    // one (`numpy.Array(…)`) keeps its exact casing (Python names aren't
    // lowercased) and its methods pass straight through.
    use_modules: HashSet<String>,
    // The polars expression builders a DataFrame program needs (`col`/`when`/
    // `read_csv`), re-exported from `vbrpy` — mirrors the Rust side re-exporting
    // `col`/`lit`/`when` from `vbr_stdlib::dataframe`.
    df_builders: std::collections::BTreeSet<&'static str>,
    // Prelude features, switched on as the body needs them.
    needs_vb: bool,
    needs_round: bool,
    needs_math: bool,
    needs_dataclass: bool,
    needs_enum: bool,
    needs_option: bool,
    needs_result: bool,
    needs_unwrap: bool,
    needs_time: bool,
    needs_sys: bool,
    needs_random: bool,
    needs_val: bool,
    needs_cdbl: bool,
    needs_clng: bool,
    needs_cint: bool,
    needs_input: bool,
    skip_auto_try: bool,
    emitting_main: bool,
    wrap_ok: bool,
    user_fns: HashSet<String>,
    user_methods: HashSet<String>,
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

    /// The shared typing pass's verdict for an expression (by span), if any.
    fn type_of(&self, e: &Expr) -> Option<DeclType> {
        self.types.get(&e.span).cloned()
    }

    /// Record a declared variable's *name* for the DataFrame formula scope check
    /// (a DataFrame variable is excluded — a bare `df` in a formula is a column).
    fn declare(&mut self, name: &str, ty: &DeclType) {
        if !matches!(ty, DeclType::Named(n) if n == "DataFrame") {
            self.formula_vars.insert(rust_name(name));
        }
    }

    fn program(&mut self, program: &Program) {
        self.const_names = program.constants.iter().map(|c| c.name.clone()).collect();
        // The name referenced in code is the import name — the `As` alias when
        // given (`Use pillow … As PIL` → `PIL`), else the package name.
        self.use_modules = program
            .uses
            .iter()
            .map(|u| u.alias.clone().unwrap_or_else(|| u.crate_name.clone()))
            .collect();
        self.enum_names = program.enums.iter().map(|e| e.name.clone()).collect();
        self.data_enums = program
            .enums
            .iter()
            .filter(|e| e.variants.iter().any(|v| !v.payload.is_empty()))
            .map(|e| e.name.clone())
            .collect();

        self.user_fns = program
            .functions
            .iter()
            .filter(|f| f.receiver.is_some() || !f.name.eq_ignore_ascii_case("main"))
            .map(|f| rust_name(&f.name))
            .collect();
        self.user_methods = program
            .functions
            .iter()
            .filter(|f| f.receiver.is_some())
            .map(|f| rust_name(&f.name))
            .collect();

        if !program.windows.is_empty() || !program.screens.is_empty() || !program.pages.is_empty() || !program.sketches.is_empty() {
            self.warn(
                "GUI/TUI/Web surfaces (`Window`/`Screen`/`Page`/`Sketch`) are Rust-only — \
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

        // Each `Enum` → an `enum.Enum` (C-like) or a variant class hierarchy.
        for e in &program.enums {
            self.top_separator();
            self.enum_def(e);
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

    /// An `Enum` → Python. A C-like enum (all unit variants) becomes an
    /// `enum.Enum`; a data-carrying one (a sum type) becomes an empty base class
    /// plus one `@dataclass` per variant, so `Match` can destructure it with
    /// structural patterns (the dataclass supplies `__match_args__`).
    fn enum_def(&mut self, e: &EnumDef) {
        if self.data_enums.contains(&e.name) {
            self.needs_dataclass = true;
            self.line(0, &format!("class {}:", e.name));
            self.line(1, "pass");
            for v in &e.variants {
                self.body.push('\n');
                self.line(0, "@dataclass");
                self.line(0, &format!("class {}({}):", v.name, e.name));
                if v.payload.is_empty() {
                    self.line(1, "pass");
                } else {
                    // Positional payload → fields `f0`, `f1`, … (their order is
                    // what a `case Circle(r)` binds against).
                    for (i, ty) in v.payload.iter().enumerate() {
                        let hint = self.type_hint(ty);
                        self.line(1, &format!("f{}: {}", i, hint));
                    }
                }
            }
        } else {
            self.needs_enum = true;
            self.line(0, &format!("class {}(Enum):", e.name));
            for (i, v) in e.variants.iter().enumerate() {
                self.line(1, &format!("{} = {}", v.name, i + 1));
            }
        }
    }

    /// A function (free, or a struct method at `indent` 1 with an implicit
    /// `self`). VB's assign-to-own-name return is desugared first (shared with
    /// the Rust backend).
    fn function(&mut self, func: &Function, indent: usize, is_method: bool) {
        let name = rust_name(&func.name);
        self.formula_vars.clear();
        self.current_ret = func.ret.clone();
        let is_main = !is_method && func.name.eq_ignore_ascii_case("main");
        self.emitting_main = is_main;
        self.wrap_ok = !is_main;
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
            self.declare(&p.name, &p.ty);
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

        let empty = body.iter().all(|s| matches!(s, Stmt::LineMark(_) | Stmt::Comment(_)));
        if empty && !self.wrap_ok {
            self.line(indent + 1, "pass");
        } else {
            self.block(&body, indent + 1);
            if self.wrap_ok && !body_never_returns(&body) {
                self.needs_result = true;
                self.line(indent + 1, "return Ok(None)");
            }
        }
        self.emitting_main = false;
        self.wrap_ok = false;
    }

    fn block(&mut self, stmts: &[Stmt], indent: usize) {
        for stmt in stmts {
            self.stmt(stmt, indent);
        }
    }

    fn stmt(&mut self, stmt: &Stmt, indent: usize) {
        // A `?` anywhere in this statement's own expressions hoists its temp +
        // early-return to lines emitted just above the statement, at this indent.
        let prev_hoist = self.hoist_at;
        self.hoist_at = Some(indent);
        self.stmt_inner(stmt, indent);
        self.hoist_at = prev_hoist;
    }

    fn stmt_inner(&mut self, stmt: &Stmt, indent: usize) {
        match stmt {
            Stmt::LineMark(_) => {}
            Stmt::Comment(c) => self.line(indent, &format!("# {}", c.trim_start_matches(['\'', ' ']))),
            Stmt::Dim { name, ty, init, .. } => {
                // `Dim x [As T] = Python … End Python`: on the Python target the
                // block is spliced verbatim and its last line bound to `x`.
                if let Some(Expr { kind: ExprKind::InlinePython { inputs, body }, .. }) = init {
                    self.declare(name, ty);
                    let target = rust_name(name);
                    self.inline_python(inputs, body, indent, Some(&target));
                    return;
                }
                let value = match init {
                    Some(e) => self.expr(e),
                    None => self.default_value(ty),
                };
                let hint = self.type_hint(ty);
                self.declare(name, ty);
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
                if self.wrap_ok {
                    self.needs_result = true;
                    self.line(indent, &format!("return Ok({})", v));
                } else {
                    self.line(indent, &format!("return {}", v));
                }
            }
            Stmt::Return(None) => {
                if self.wrap_ok {
                    self.needs_result = true;
                    self.line(indent, "return Ok(None)");
                } else {
                    self.line(indent, "return");
                }
            }
            Stmt::RaiseError(e) => {
                self.needs_result = true;
                let msg = match &e.kind {
                    ExprKind::Str(_) => self.expr(e),
                    _ => format!("str({})", self.expr(e)),
                };
                self.line(indent, &format!("return Err({})", msg));
            }
            Stmt::HandleErr { target, call, err_name, body, .. } => {
                self.emit_handle(target.as_ref(), call, err_name, body, indent);
            }
            Stmt::Expr(e) => {
                // A bare `Python … End Python` statement: splice the block in for
                // its side effects; its last line is evaluated and discarded.
                if let ExprKind::InlinePython { inputs, body } = &e.kind {
                    self.inline_python(inputs, body, indent, None);
                    return;
                }
                // A bare `foo()?` statement: hoist the temp + early-return, but
                // the unwrapped value is discarded (no trailing line).
                if let ExprKind::Try(_) = &e.kind {
                    let _ = self.expr(e);
                    return;
                }
                // A dict `.insert(k, v)` is a subscript assignment in Python
                // (`d[k] = v`); a Vec `.insert(i, x)` keeps `list.insert(i, x)`.
                if let ExprKind::MethodCall { recv, method, args } = &e.kind {
                    if method.eq_ignore_ascii_case("insert")
                        && args.len() == 2
                        && self.recv_is_map(recv)
                    {
                        let base = self.expr(recv);
                        let k = self.expr(&args[0]);
                        let val = self.expr(&args[1]);
                        self.line(indent, &format!("{}[{}] = {}", base, k, val));
                        return;
                    }
                }
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
            Stmt::For { var, from, to, step, body, .. } => {
                self.declare(var, &DeclType::Plain(Type::Long));
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
            Stmt::DestructureDim { names, ty, value } => {
                if let Some(DeclType::Tuple(ts)) = ty {
                    for (n, t) in names.iter().zip(ts) {
                        self.declare(n, t);
                    }
                }
                let lhs = names.iter().map(|n| rust_name(n)).collect::<Vec<_>>().join(", ");
                // `Dim (a, b, c) As (…) = Python … End Python`: pull several
                // values out in one block; the last line is a tuple bound to `lhs`.
                if let ExprKind::InlinePython { inputs, body } = &value.kind {
                    self.inline_python(inputs, body, indent, Some(&lhs));
                    return;
                }
                let v = self.expr(value);
                self.line(indent, &format!("{} = {}", lhs, v));
            }
            Stmt::DoLoop { cond, body } => self.do_loop(cond, body, indent),
            Stmt::Match { scrutinee, arms, .. } => self.match_stmt(scrutinee, arms, indent),
            Stmt::Break => self.line(indent, "break"),
            Stmt::Continue => self.line(indent, "continue"),
            // `x = Nothing` → `x = None`; Python's GC reclaims the old value.
            Stmt::Destroy { name, .. } => {
                self.line(indent, &format!("{} = None", rust_name(name)));
            }
            other => {
                self.warn(format!("`{}` doesn't lower to Python yet.", stmt_name(other)));
                self.line(indent, &format!("pass  # [Bust→Python] unsupported: {}", stmt_name(other)));
            }
        }
    }

    /// Splice a `Python … End Python` block into the generated Python. On the
    /// Python target the block *is* Python — the delicious inversion of inline
    /// `Rust` on the Rust target — so its body is emitted verbatim rather than
    /// run through embedded CPython. Passed-in variables (`Python(data)`) are
    /// already in scope as locals; only a casing mismatch needs a re-alias. The
    /// last non-blank line is the value: bound to `bind` when given (a name, or a
    /// `a, b, c` tuple target), otherwise evaluated for its side effects.
    fn inline_python(&mut self, inputs: &[String], body: &str, indent: usize, bind: Option<&str>) {
        // Re-expose each input under the exact name the block wrote, in case Bust
        // lowercased it (`Python(Data)` → the block still says `Data`).
        for name in inputs {
            let local = rust_name(name);
            if *name != local {
                self.line(indent, &format!("{} = {}", name, local));
            }
        }
        let lines = dedent_lines(body);
        let Some((last, prefix)) = lines.split_last() else {
            return;
        };
        for l in prefix {
            if l.trim().is_empty() {
                self.line(0, "");
            } else {
                self.line(indent, l);
            }
        }
        match bind {
            Some(lhs) => self.line(indent, &format!("{} = {}", lhs, last.trim_start())),
            None => self.line(indent, last.trim_start()),
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

    /// `Match … End Match` → Python `match`/`case`. The scrutinee is bound to a
    /// temp first, so a range arm (which Python has no pattern for) can reference
    /// it from a guard.
    fn match_stmt(&mut self, scrutinee: &Expr, arms: &[MatchArm], indent: usize) {
        let subj = format!("_m{}", self.match_counter);
        self.match_counter += 1;
        let value = self.expr(scrutinee);
        self.line(indent, &format!("{} = {}", subj, value));
        self.line(indent, &format!("match {}:", subj));
        for arm in arms {
            let (pat, range_guard) = self.translate_pattern(&arm.pattern, &subj);
            let user_guard = arm.guard.as_ref().map(|g| self.expr(g));
            let guard = match (range_guard, user_guard) {
                (Some(a), Some(b)) => Some(format!("({}) and ({})", a, b)),
                (Some(g), None) | (None, Some(g)) => Some(g),
                (None, None) => None,
            };
            let header = match guard {
                Some(g) => format!("case {} if {}:", pat, g),
                None => format!("case {}:", pat),
            };
            self.line(indent + 1, &header);
            self.block_or_pass(&arm.body, indent + 2);
        }
    }

    /// Translate a raw (Rust-shaped) match pattern to a Python `case` pattern,
    /// plus an optional guard fragment (a range becomes a guard, since Python has
    /// no range pattern). `subj` is the scrutinee temp the range guard reads.
    /// Parses via the shared [`pattern`] model rather than re-tokenising here.
    fn translate_pattern(&mut self, pattern: &str, subj: &str) -> (String, Option<String>) {
        let pat = pattern::parse(pattern);
        if let Pat::Range { lo, hi, inclusive } = pat {
            let op = if inclusive { "<=" } else { "<" };
            return ("_".into(), Some(format!("{} <= {} {} {}", lo, subj, op, hi)));
        }
        (self.render_pat(&pat), None)
    }

    /// One shared-model pattern → its Python `case` spelling (recursive for the
    /// `Some`/`Ok`/`Err` nestings and alternation).
    fn render_pat(&mut self, pat: &Pat) -> String {
        match pat {
            Pat::Wildcard => "_".into(),
            Pat::Binding(x) => x.clone(),
            Pat::Int(n) => n.to_string(),
            Pat::Bool(b) => if *b { "True" } else { "False" }.into(),
            // Only a *top-level* range becomes a guard (in `translate_pattern`);
            // a nested one has no Python form and doesn't arise in practice.
            Pat::Range { .. } => "_".into(),
            Pat::Alt(subs) => subs.iter().map(|p| self.render_pat(p)).collect::<Vec<_>>().join(" | "),
            // A C-like enum matches by value (`Suit.Hearts`); a data enum matches
            // its variant class (`Empty()`).
            Pat::EnumTag { enom, variant } => {
                if self.data_enums.contains(enom) {
                    format!("{}()", variant)
                } else {
                    format!("{}.{}", enom, variant)
                }
            }
            Pat::Variant { variant, binds, .. } => format!("{}({})", variant, binds.join(", ")),
            Pat::Some(inner) => {
                self.needs_option = true;
                format!("Some({})", self.render_pat(inner))
            }
            Pat::None => "None".into(),
            Pat::Ok(inner) => {
                self.needs_result = true;
                format!("Ok({})", self.render_pat(inner))
            }
            Pat::Err(inner) => {
                self.needs_result = true;
                format!("Err({})", self.render_pat(inner))
            }
            Pat::Other(s) => s.replace(' ', ""),
        }
    }

    /// An enum-path pattern → a Python `case`. A C-like enum matches by value
    /// (`Suit.Hearts`); a data enum matches its variant class structurally
    /// (`Circle(r)` / `Empty()`).
    fn block_or_pass(&mut self, stmts: &[Stmt], indent: usize) {
        if stmts.iter().all(|s| matches!(s, Stmt::LineMark(_))) {
            self.line(indent, "pass");
            return;
        }
        self.block(stmts, indent);
    }

    /// A method call → its Python form. The curated table turns Rust/Bust method
    /// names into Python idioms (`.push`→`.append`, `.len()`→`len()`, iterator
    /// chains → comprehensions); anything unrecognised passes straight through.
    fn method_call(&mut self, recv: &Expr, method: &str, args: &[Expr]) -> String {
        let m = method.to_ascii_lowercase();
        let key = m.replace('_', "");

        // A standard-library static call (`FileSystem.Read(...)`, `Json.Parse(...)`)
        // → the matching `vbrpy` class method; the namespace is recorded so the
        // import (and project mode) is emitted.
        if let ExprKind::Ident(ns) = &recv.kind {
            if ns == "DataFrame" {
                // `DataFrame.Read_Csv(path)` → polars `read_csv(path)` (re-exported).
                self.stdlib_used.insert("DataFrame".to_string());
                if key == "readcsv" {
                    self.df_builders.insert("read_csv");
                    let a: Vec<String> = args.iter().map(|x| self.expr(x)).collect();
                    // Read common missing-value tokens as nulls, matching the Rust
                    // stdlib's `read_csv` so both targets produce the same frame.
                    return format!(
                        "read_csv({}, null_values=[\"\", \"NA\", \"N/A\", \"n/a\", \"null\", \"NULL\", \"NaN\"])",
                        a.join(", ")
                    );
                }
            } else if STDLIB_SUPPORTED.contains(&ns.as_str()) {
                self.stdlib_used.insert(ns.clone());
                let a: Vec<String> = args.iter().map(|x| self.expr(x)).collect();
                return format!("{}.{}({})", ns, m, a.join(", "));
            }
            if STDLIB_PENDING.contains(&ns.as_str()) {
                self.warn(format!(
                    "the `{}` standard-library namespace isn't lowered to Python yet — coming in a later slice.",
                    ns
                ));
            }
        }

        // A call into a `Use`-d pip module (`numpy.Array(...)`) passes straight
        // through, keeping the exact method casing — Python names aren't
        // lowercased, so `pandas.DataFrame(...)` stays `DataFrame`. Must run
        // before the terminal consumers below so `numpy.Sum(x)` isn't rewritten
        // to `sum(x)`.
        if let ExprKind::Ident(ns) = &recv.kind {
            if self.use_modules.contains(ns) {
                let a: Vec<String> = args.iter().map(|x| self.expr(x)).collect();
                return format!("{}.{}({})", ns, method, a.join(", "));
            }
        }

        // An instance method on a DataFrame (`df.With_Column(...)`, `df.Filter(...)`)
        // → idiomatic polars, with column-formula arguments lowered.
        if self.is_df_expr(recv) {
            return self.df_method(recv, &m, args);
        }

        // An iterator pipeline (`collect`/`sum`/`count`/`any`/`find`/`max`/…) over
        // a chain of adapters → a comprehension. The *shape* comes from the shared
        // `iter` analysis; the Python rendering (comprehensions, Option-wrapping)
        // lives in `render_chain`.
        if let Some(term) = iter::terminal(&m, args) {
            let (base, steps) = iter::split_adapters(recv);
            return self.render_chain(base, &steps, &term);
        }

        // `.Unwrap()` on an Option/Result → the prelude `_unwrap` (raises on
        // `None`/`Err`, exactly like Rust's `.unwrap()` panics). It also passes a
        // bare value through, so a `dict.get(k).Unwrap()` still works.
        if m == "unwrap" {
            // `_unwrap` names all four wrappers, so ensure their classes exist.
            self.needs_unwrap = true;
            self.needs_option = true;
            self.needs_result = true;
            return format!("_unwrap({})", self.expr(recv));
        }

        // `map.contains_key(k)` → `k in map`.
        if m == "contains_key" && args.len() == 1 {
            let base = self.expr(recv);
            let key = self.expr(&args[0]);
            return format!("{} in {}", key, base);
        }

        // Straight name remaps (receiver method → Python method).
        let mapped = match m.as_str() {
            "push" => Some("append"),
            "to_uppercase" => Some("upper"),
            "to_lowercase" => Some("lower"),
            "starts_with" => Some("startswith"),
            "ends_with" => Some("endswith"),
            "trim" => Some("strip"),
            _ => None,
        };
        let base = self.expr(recv);
        let a: Vec<String> = args.iter().map(|x| self.expr(x)).collect();
        match mapped {
            Some(py) => format!("{}.{}({})", base, py, a.join(", ")),
            None => format!("{}.{}({})", base, rust_name(method), a.join(", ")),
        }
    }

    /// Render a parsed iterator pipeline as a Python expression — a comprehension
    /// or generator, Option-wrapping the consumers (`find`/`max`/…) that return
    /// one, exactly as a Rust `Option` would.
    fn render_chain(&mut self, base: &Expr, steps: &[iter::Step], term: &iter::Terminal) -> String {
        let src = self.render_source(base, steps);
        match term {
            iter::Terminal::Collect => src,
            iter::Terminal::Sum => format!("sum({})", src),
            iter::Terminal::Count => format!("len({})", src),
            iter::Terminal::Any { var, cond } => {
                let v = rust_name(var);
                let c = self.expr(cond);
                format!("any({} for {} in {})", c, v, src)
            }
            iter::Terminal::All { var, cond } => {
                let v = rust_name(var);
                let c = self.expr(cond);
                format!("all({} for {} in {})", c, v, src)
            }
            iter::Terminal::Find { var, cond } => {
                self.needs_option = true;
                let v = rust_name(var);
                let c = self.expr(cond);
                format!("next((Some({v}) for {v} in {src} if {c}), None)")
            }
            iter::Terminal::Position { var, cond } => {
                self.needs_option = true;
                let v = rust_name(var);
                let c = self.expr(cond);
                format!("next((Some(_i) for _i, {v} in enumerate({src}) if {c}), None)")
            }
            iter::Terminal::Max => {
                self.needs_option = true;
                format!("(Some(max({src})) if {src} else None)")
            }
            iter::Terminal::Min => {
                self.needs_option = true;
                format!("(Some(min({src})) if {src} else None)")
            }
        }
    }

    /// Fold a base receiver + its adapter steps into a Python list/slice source:
    /// `filter`/`map` become comprehensions, `take`/`skip`/`rev` become slices.
    fn render_source(&mut self, base: &Expr, steps: &[iter::Step]) -> String {
        let mut src = self.expr(base);
        for step in steps {
            src = match step {
                iter::Step::Filter { var, cond } => {
                    let v = rust_name(var);
                    let c = self.expr(cond);
                    format!("[{v} for {v} in {src} if {c}]")
                }
                iter::Step::Map { var, body } => {
                    let v = rust_name(var);
                    let b = self.expr(body);
                    format!("[{b} for {v} in {src}]")
                }
                iter::Step::Take(n) => {
                    let n = self.expr(n);
                    format!("{src}[:{n}]")
                }
                iter::Step::Skip(n) => {
                    let n = self.expr(n);
                    format!("{src}[{n}:]")
                }
                iter::Step::Rev => format!("{src}[::-1]"),
            };
        }
        src
    }

    /// Lower a `?` (`Try`): bind its operand to a temp on a line just above the
    /// current statement, early-return on failure (`Err` for a Result-returning
    /// function, `None` for an Option one), and yield the unwrapped `.value`.
    /// Works for a `?` anywhere in a statement's expressions — the hoisted lines
    /// appear in evaluation order because they're emitted as each `?` is
    /// rendered.
    fn hoist_try(&mut self, inner: &Expr) -> String {
        let Some(indent) = self.hoist_at else {
            self.warn("`?` couldn't be lowered here (no statement context).");
            return self.expr(inner);
        };
        let val = self.expr(inner);
        let tmp = format!("_t{}", self.tmp_counter);
        self.tmp_counter += 1;
        self.line(indent, &format!("{} = {}", tmp, val));
        if matches!(self.current_ret, Some(DeclType::Option(_))) {
            self.line(indent, &format!("if {} is None:", tmp));
            self.line(indent + 1, &format!("return {}", tmp));
        } else {
            self.needs_result = true;
            self.line(indent, &format!("if isinstance({}, Err):", tmp));
            self.line(indent + 1, &format!("return {}", tmp));
        }
        format!("{}.value", tmp)
    }

    fn hoist_result(&mut self, val: String) -> String {
        let Some(indent) = self.hoist_at else {
            self.warn("`Handle`/`?` couldn't be lowered here (no statement context).");
            return val;
        };
        self.needs_result = true;
        let tmp = format!("_t{}", self.tmp_counter);
        self.tmp_counter += 1;
        self.line(indent, &format!("{} = {}", tmp, val));
        self.line(indent, &format!("if isinstance({}, Err):", tmp));
        if self.emitting_main {
            self.needs_sys = true;
            self.line(indent + 1, &format!("print(f\"Error: {{{}.error}}\", file=sys.stderr)", tmp));
            self.line(indent + 1, "raise SystemExit(1)");
        } else {
            self.line(indent + 1, &format!("return {}", tmp));
        }
        format!("{}.value", tmp)
    }

    fn should_auto_try_call(&self, name: &str) -> bool {
        if self.skip_auto_try {
            return false;
        }
        let lower = name.to_ascii_lowercase();
        if matches!(lower.as_str(), "ok" | "err" | "some" | "none" | "iif" | "sleep") {
            return false;
        }
        if matches!(lower.as_str(), "cdbl" | "clng" | "cint" | "inputbox") {
            return true;
        }
        self.user_fns.contains(&rust_name(name))
    }

    fn should_auto_try_method(&self, recv: &Expr, method: &str, whole: &Expr) -> bool {
        if self.skip_auto_try {
            return false;
        }
        if matches!(self.type_of(whole), Some(DeclType::Result(..))) {
            return true;
        }
        if matches!(&recv.kind, ExprKind::Ident(n) if STDLIB_SUPPORTED.contains(&n.as_str())) {
            return false;
        }
        self.user_methods.contains(&rust_name(method))
    }

    fn emit_handle(
        &mut self,
        target: Option<&Expr>,
        call: &Expr,
        err_name: &str,
        body: &[Stmt],
        indent: usize,
    ) {
        let saved = self.skip_auto_try;
        self.skip_auto_try = true;
        let val = self.expr(call);
        self.skip_auto_try = saved;
        self.needs_result = true;
        let tmp = format!("_t{}", self.tmp_counter);
        self.tmp_counter += 1;
        let err = rust_name(err_name);
        self.line(indent, &format!("{} = {}", tmp, val));
        match target {
            None => {
                self.line(indent, &format!("if isinstance({}, Err):", tmp));
                self.line(indent + 1, &format!("{} = {}.error", err, tmp));
                self.block_or_pass(body, indent + 1);
            }
            Some(tgt) => {
                let lhs = self.expr(tgt);
                self.line(indent, &format!("if isinstance({}, Err):", tmp));
                self.line(indent + 1, &format!("{} = {}.error", err, tmp));
                if py_body_diverges(body) {
                    self.block_or_pass(body, indent + 1);
                    self.line(indent, "else:");
                    self.line(indent + 1, &format!("{} = {}.value", lhs, tmp));
                } else if let Some(Stmt::Expr(e)) = body
                    .iter()
                    .rev()
                    .find(|s| !matches!(s, Stmt::Comment(_) | Stmt::LineMark(_)))
                {
                    let last_i = body.iter().rposition(|s| matches!(s, Stmt::Expr(_))).unwrap();
                    self.block(&body[..last_i], indent + 1);
                    let repl = self.expr(e);
                    self.line(indent + 1, &format!("{} = {}", lhs, repl));
                    self.line(indent, "else:");
                    self.line(indent + 1, &format!("{} = {}.value", lhs, tmp));
                } else {
                    self.block_or_pass(body, indent + 1);
                    self.line(indent, "else:");
                    self.line(indent + 1, &format!("{} = {}.value", lhs, tmp));
                }
            }
        }
    }


    /// Is `recv` known to be a `Map`/`HashMap` (so `.insert` is a subscript)?
    fn recv_is_map(&self, recv: &Expr) -> bool {
        matches!(self.type_of(recv), Some(DeclType::Map(_, _)))
    }

    /// Is `e` a DataFrame-valued expression? A variable declared `As DataFrame`,
    /// a `DataFrame.Read_Csv(...)` constructor, or a transform chained off one.
    fn is_df_expr(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Ident(_) => matches!(self.type_of(e), Some(DeclType::Named(t)) if t == "DataFrame"),
            ExprKind::MethodCall { recv, .. } => {
                matches!(&recv.kind, ExprKind::Ident(n) if n == "DataFrame") || self.is_df_expr(recv)
            }
            _ => false,
        }
    }

    /// A DataFrame instance method → idiomatic polars.
    fn df_method(&mut self, recv: &Expr, m: &str, args: &[Expr]) -> String {
        let base = self.expr(recv);
        let key = m.replace('_', "");
        match key.as_str() {
            "withcolumn" => {
                let name = self.expr(&args[0]);
                let formula = self.lower_formula(&args[1]);
                format!("{}.with_columns({}.alias({}))", base, formula, name)
            }
            "filter" => {
                let mask = self.lower_formula(&args[0]);
                format!("{}.filter({})", base, mask)
            }
            "select" => {
                let cols: Vec<String> = args.iter().map(|a| self.expr(a)).collect();
                format!("{}.select([{}])", base, cols.join(", "))
            }
            "sort" => format!("{}.sort({})", base, self.expr(&args[0])),
            "head" => format!("{}.head({})", base, self.expr(&args[0])),
            "shape" => format!("{}.shape", base),
            "columns" => format!("{}.columns", base),
            "column" => format!("{}[{}].to_list()", base, self.expr(&args[0])),
            "join" | "leftjoin" | "outerjoin" => {
                let other = self.expr(&args[0]);
                let keys: Vec<String> = args[1..].iter().map(|a| self.expr(a)).collect();
                let on = if keys.len() == 1 {
                    keys[0].clone()
                } else {
                    format!("[{}]", keys.join(", "))
                };
                let how = match key.as_str() {
                    "join" => "inner",
                    "leftjoin" => "left",
                    _ => "outer",
                };
                format!("{}.join({}, on={}, how='{}')", base, other, on, how)
            }
            "groupby" => {
                let keys: Vec<String> = args.iter().map(|a| self.expr(a)).collect();
                format!("{}.group_by([{}])", base, keys.join(", "))
            }
            "agg" => {
                let exprs: Vec<String> = args.iter().map(|a| self.lower_agg(a)).collect();
                format!("{}.agg([{}])", base, exprs.join(", "))
            }
            "sum" | "mean" | "min" | "max" => {
                format!("{}[{}].{}()", base, self.expr(&args[0]), m)
            }
            "writecsv" => format!("{}.write_csv({})", base, self.expr(&args[0])),
            "print" => format!("print({})", base),
            _ => {
                self.warn(format!("DataFrame method `{}` isn't lowered to Python yet.", m));
                let a: Vec<String> = args.iter().map(|x| self.expr(x)).collect();
                format!("{}.{}({})", base, m, a.join(", "))
            }
        }
    }

    /// Rewrite a Bust column formula (`price * qty`, `age >= 18`, `IIf(...)`) into a
    /// polars expression — the Python-side twin of the resolver's `lower_formula`.
    /// A bare name is a column (`col("x")`) unless it's a `Dim`'d value; polars
    /// overloads the operators (`>`, `&`, `~`), so no `.gt()`/`.and()` methods.
    fn lower_formula(&mut self, e: &Expr) -> String {
        match &e.kind {
            // A bare name is a column, unless it's a `Dim`'d value — then it's a
            // `lit(...)` (as is any literal), matching the Rust resolver and
            // sidestepping polars reading a bare string as a column name.
            ExprKind::Ident(name) => {
                let is_value = self.formula_vars.contains(&rust_name(name));
                if is_value {
                    self.df_builders.insert("lit");
                    format!("lit({})", rust_name(name))
                } else {
                    self.df_builders.insert("col");
                    format!("col(\"{}\")", name)
                }
            }
            ExprKind::Str(s) => {
                self.df_builders.insert("lit");
                format!("lit({})", py_str(s))
            }
            ExprKind::Int(n) => {
                self.df_builders.insert("lit");
                format!("lit({})", n)
            }
            ExprKind::Float(f) => {
                self.df_builders.insert("lit");
                format!("lit({})", py_float(*f))
            }
            ExprKind::Bool(b) => {
                self.df_builders.insert("lit");
                format!("lit({})", if *b { "True" } else { "False" })
            }
            ExprKind::Call { name, args } if name.eq_ignore_ascii_case("Is_Null") && args.len() == 1 => {
                let inner = self.lower_formula(&args[0]);
                format!("{}.is_null()", inner)
            }
            ExprKind::Call { name, args } if name == "Col" && args.len() == 1 => {
                self.df_builders.insert("col");
                format!("col({})", self.expr(&args[0]))
            }
            ExprKind::Call { name, args } if name.eq_ignore_ascii_case("IIf") && args.len() == 3 => {
                self.df_builders.insert("when");
                let c = self.lower_formula(&args[0]);
                let t = self.lower_formula(&args[1]);
                let el = self.lower_formula(&args[2]);
                format!("when({}).then({}).otherwise({})", c, t, el)
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let l = self.lower_formula(lhs);
                let r = self.lower_formula(rhs);
                let opstr = match op {
                    BinOp::Add => "+",
                    BinOp::Sub => "-",
                    BinOp::Mul => "*",
                    BinOp::Div => "/",
                    BinOp::Mod => "%",
                    BinOp::Gt => ">",
                    BinOp::Lt => "<",
                    BinOp::Ge => ">=",
                    BinOp::Le => "<=",
                    BinOp::Eq => "==",
                    BinOp::Ne => "!=",
                    BinOp::And => "&",
                    BinOp::Or => "|",
                    _ => {
                        self.warn("operator not supported in a DataFrame formula.");
                        "+"
                    }
                };
                format!("({} {} {})", l, opstr, r)
            }
            ExprKind::Not(inner) => {
                let i = self.lower_formula(inner);
                format!("(~{})", i)
            }
            _ => {
                self.warn("unsupported element in a DataFrame formula.");
                self.expr(e)
            }
        }
    }

    /// Lower one `Agg(...)` argument: `Sum(x)`/`Mean(x)`/`Count(x)`/… → the inner
    /// formula plus the polars aggregation method; a bare formula passes through.
    fn lower_agg(&mut self, e: &Expr) -> String {
        if let ExprKind::Call { name, args } = &e.kind {
            let low = name.to_ascii_lowercase();
            // `Count()` with no argument = rows per group (polars `len()`, exposed
            // as `count_rows` to dodge the Python builtin), aliased to "count".
            if low == "count" && args.is_empty() {
                self.df_builders.insert("count_rows");
                return "count_rows().alias(\"count\")".to_string();
            }
            if matches!(low.as_str(), "sum" | "mean" | "min" | "max" | "count") && args.len() == 1 {
                let inner = self.lower_formula(&args[0]);
                return format!("{}.{}()", inner, low);
            }
        }
        self.lower_formula(e)
    }

    /// A coarse numeric class for the `//` vs `/` division choice — read
    /// straight from the shared typing pass (which already widens arithmetic and
    /// knows function/collection return types).
    fn numeric(&self, e: &Expr) -> Num {
        match self.type_of(e) {
            Some(DeclType::Plain(Type::Integer | Type::Long | Type::LongLong | Type::Byte)) => Num::Int,
            Some(DeclType::Plain(Type::Single | Type::Double)) => Num::Float,
            _ => Num::Unknown,
        }
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
            ExprKind::Field(recv, field) => match &recv.kind {
                // `Enum.Variant`: a C-like variant is a value (`Suit.Spades`); a
                // data enum's unit variant constructs its class (`Empty()`).
                ExprKind::Ident(name) if self.enum_names.contains(name) => {
                    if self.data_enums.contains(name) {
                        format!("{}()", field)
                    } else {
                        format!("{}.{}", name, field)
                    }
                }
                // An attribute on a `Use`-d module (`numpy.pi`) keeps its exact
                // casing — Python names aren't lowercased.
                ExprKind::Ident(name) if self.use_modules.contains(name) => {
                    format!("{}.{}", name, field)
                }
                _ => {
                    let r = self.expr(recv);
                    format!("{}.{}", r, rust_name(field))
                }
            },
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
                // `Enum.Variant(args)` constructs a data-enum variant class.
                let ctor = match &recv.kind {
                    ExprKind::Ident(name) if self.enum_names.contains(name) => Some(name.clone()),
                    _ => None,
                };
                if let Some(name) = ctor {
                    let a: Vec<String> = args.iter().map(|x| self.expr(x)).collect();
                    if !self.data_enums.contains(&name) {
                        self.warn(format!(
                            "`{}.{}(…)` — a C-like enum variant carries no data.",
                            name, method
                        ));
                    }
                    format!("{}({})", method, a.join(", "))
                } else {
                    let s = self.method_call(recv, method, args);
                    if self.should_auto_try_method(recv, method, e) {
                        self.hoist_result(s)
                    } else {
                        s
                    }
                }
            }
            ExprKind::List(items) => {
                let parts: Vec<String> = items.iter().map(|i| self.expr(i)).collect();
                format!("[{}]", parts.join(", "))
            }
            ExprKind::Index(recv, idx) => {
                let r = self.expr(recv);
                let i = self.expr(idx);
                format!("{}[{}]", r, i)
            }
            ExprKind::Not(inner) => {
                let i = self.expr(inner);
                format!("not ({})", i)
            }
            ExprKind::Try(inner) => self.hoist_try(inner),
            ExprKind::Raw(inner) => {
                let saved = self.skip_auto_try;
                self.skip_auto_try = true;
                let s = self.expr(inner);
                self.skip_auto_try = saved;
                s
            }
            ExprKind::Binary { op: BinOp::Concat, .. } => self.concat_fstring(e),
            ExprKind::Binary { op: BinOp::Div, lhs, rhs } => {
                // Rust's `/` truncates for integer operands but divides for floats;
                // Python's `/` is always float, so integer operands need `//`. When
                // the operand types can't be proven, keep `/` and warn.
                let l = self.operand(lhs);
                let r = self.operand(rhs);
                match (self.numeric(lhs), self.numeric(rhs)) {
                    (Num::Int, Num::Int) => format!("{} // {}", l, r),
                    (Num::Float, _) | (_, Num::Float) => format!("{} / {}", l, r),
                    _ => {
                        self.warn(
                            "`/` on values of unknown type — kept as Python float division; \
                             if these are integers you may want `//`.",
                        );
                        format!("{} / {}", l, r)
                    }
                }
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let l = self.operand(lhs);
                let r = self.operand(rhs);
                if *op == BinOp::Pow {
                    format!("{} ** {}", l, r)
                } else {
                    format!("{} {} {}", l, self.bin_op(*op), r)
                }
            }
            ExprKind::Call { name, args } => {
                let s = self.call(name, args);
                if self.should_auto_try_call(name) {
                    self.hoist_result(s)
                } else {
                    s
                }
            }
            other => {
                self.warn(format!("`{}` doesn't lower to Python yet.", expr_name(other)));
                format!("None  # [Bust→Python] unsupported: {}", expr_name(other))
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
        // Option/Result constructors keep their capitalised names (they map to
        // the prelude `Some`/`Ok`/`Err` classes; `None` is Python's own).
        match name {
            "Some" => {
                self.needs_option = true;
                let a: Vec<String> = args.iter().map(|x| self.expr(x)).collect();
                return format!("Some({})", a.join(", "));
            }
            "Ok" => {
                self.needs_result = true;
                // `Ok(())` is the unit success → `Ok(None)`.
                let is_unit = args.is_empty()
                    || (args.len() == 1 && matches!(&args[0].kind, ExprKind::Tuple(t) if t.is_empty()));
                if is_unit {
                    return "Ok(None)".into();
                }
                let a: Vec<String> = args.iter().map(|x| self.expr(x)).collect();
                return format!("Ok({})", a.join(", "));
            }
            "Err" => {
                self.needs_result = true;
                let a: Vec<String> = args.iter().map(|x| self.expr(x)).collect();
                return format!("Err({})", a.join(", "));
            }
            // `CStr`/`Str` — VB's infallible string conversion → Python `str(x)`.
            "CStr" | "Str" if args.len() == 1 => {
                let a = self.expr(&args[0]);
                return format!("str({})", a);
            }
            // `Sleep ms` — VB6's kernel32 Sleep (milliseconds) → `time.sleep(s)`.
            "Sleep" if args.len() == 1 => {
                self.needs_time = true;
                let a = self.expr(&args[0]);
                return format!("time.sleep({} / 1000)", a);
            }
            _ => {}
        }
        if name.eq_ignore_ascii_case("iif") && args.len() == 3 {
            // Lazy, like the Rust `if`/`else` (not VB's eager IIf).
            let c = self.expr(&args[0]);
            let t = self.expr(&args[1]);
            let e = self.expr(&args[2]);
            return format!("({}) if ({}) else ({})", t, c, e);
        }
        if name.eq_ignore_ascii_case("rnd") && args.is_empty() {
            self.needs_random = true;
            return "random.random()".into();
        }
        if let Some(s) = self.lower_py_builtin(name, args) {
            return s;
        }
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
                "atn" => return math(self, "atan"),
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

    /// String / conversion builtins that aren't maths. Characters, not bytes,
    /// matching Rust's `chars()` (Python 3 `str` is already code points).
    fn lower_py_builtin(&mut self, name: &str, args: &[Expr]) -> Option<String> {
        let n = name.to_ascii_lowercase();
        let a = |i: usize, e: &mut Emitter| e.expr(&args[i]);
        Some(match (n.as_str(), args.len()) {
            ("len", 1) => format!("len({})", a(0, self)),
            ("ucase", 1) => format!("({}).upper()", a(0, self)),
            ("lcase", 1) => format!("({}).lower()", a(0, self)),
            ("trim", 1) => format!("({}).strip()", a(0, self)),
            ("cstr" | "str", 1) => format!("str({})", a(0, self)),
            ("chr", 1) => format!("chr(int({}) & 255)", a(0, self)),
            ("asc", 1) => {
                let s = a(0, self);
                format!("(ord(({s})[0]) if ({s}) else 0)")
            }
            ("val", 1) => {
                self.needs_val = true;
                format!("_vb_val({})", a(0, self))
            }
            ("cdbl", 1) => {
                self.needs_cdbl = true;
                self.needs_result = true;
                format!("_vb_cdbl({})", a(0, self))
            }
            ("clng", 1) => {
                self.needs_clng = true;
                self.needs_result = true;
                format!("_vb_clng({})", a(0, self))
            }
            ("cint", 1) => {
                self.needs_cint = true;
                self.needs_result = true;
                format!("_vb_cint({})", a(0, self))
            }
            ("inputbox", 1) => {
                self.needs_input = true;
                self.needs_result = true;
                format!("_vb_input_box({})", a(0, self))
            }
            ("left", 2) => format!("({})[:max(int({}), 0)]", a(0, self), a(1, self)),
            ("right", 2) => {
                let s = a(0, self);
                let n = a(1, self);
                format!("(({s})[-int({n}):] if int({n}) else '')")
            }
            ("mid", 2) => format!("({})[max(int({}) - 1, 0):]", a(0, self), a(1, self)),
            ("mid", 3) => {
                let s = a(0, self);
                let start = a(1, self);
                let count = a(2, self);
                format!(
                    "({s})[max(int({start}) - 1, 0):max(int({start}) - 1, 0) + int({count})]"
                )
            }
            ("replace", 3) => format!("({}).replace({}, {})", a(0, self), a(1, self), a(2, self)),
            ("instr", 2) => {
                self.needs_option = true;
                let hay = a(0, self);
                let needle = a(1, self);
                format!(
                    "(lambda __i: Some(__i + 1) if __i >= 0 else None)(({hay}).find({needle}))"
                )
            }
            ("round", 2) => {
                self.needs_round = true;
                let x = a(0, self);
                let p = a(1, self);
                format!("_vb_round(({x}) * (10 ** int({p}))) / (10 ** int({p}))")
            }
            ("split", 1) => format!("({}).split(' ')", a(0, self)),
            ("split", 2) => format!("({}).split({})", a(0, self), a(1, self)),
            ("join", 1) => format!("' '.join({})", a(0, self)),
            ("join", 2) => format!("({}).join({})", a(1, self), a(0, self)),
            ("space", 1) => format!("(' ' * max(int({}), 0))", a(0, self)),
            ("format", 2) => {
                let ExprKind::Str(pat) = &args[1].kind else {
                    return None;
                };
                let parsed = crate::fmtpat::FormatPat::parse(pat)?;
                if parsed.is_bare() {
                    self.needs_vb = true;
                    format!("{}.format(_vb({}))", py_str(pat), a(0, self))
                } else {
                    format!(
                        "{}.format({})",
                        py_str(&parsed.python_pattern()),
                        a(0, self)
                    )
                }
            }
            _ => return None,
        })
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
            // Option/Result are modelled as the prelude `Some`/`None`/`Ok`/`Err`
            // wrappers, not a bare union — so `object` is the honest annotation.
            DeclType::Option(_) => "object".into(),
            DeclType::Result(_, _) => "object".into(),
            // A DataFrame is a polars frame (no imported class name); annotate it
            // `object` so a param/return hint can't NameError. Local-var hints
            // aren't evaluated at runtime, but params/returns are.
            DeclType::Named(n) if n == "DataFrame" => {
                self.stdlib_used.insert("DataFrame".to_string());
                "object".into()
            }
            DeclType::Named(n) => {
                // A stdlib value type (`As Json`) needs the `vbrpy` import too.
                if STDLIB_SUPPORTED.contains(&n.as_str()) {
                    self.stdlib_used.insert(n.clone());
                }
                n.clone()
            }
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
        let project = !self.stdlib_used.is_empty();

        if self.needs_math {
            code.push_str("import math\n");
        }
        if self.needs_time {
            code.push_str("import time\n");
        }
        if self.needs_sys {
            code.push_str("import sys\n");
        }
        if self.needs_random {
            code.push_str("import random\n");
        }
        // `Use <package> <version> [As <module>]` → a top-level `import <module>`
        // (the alias when a pip package imports under a different name — `pillow`
        // installs, `PIL` imports), in source order. The module is then in scope
        // for both direct calls and inline `Python` blocks (same module globals).
        // The dependency (keyed by the *package* name) is recorded below.
        for u in &program.uses {
            let module = u.alias.as_deref().unwrap_or(&u.crate_name);
            code.push_str(&format!("import {}\n", module));
        }
        // In a project the `Some`/`Ok`/`Err` wrappers come from `vbrpy`, so only
        // user `Type`/`Enum` need the dataclass import here; single-file inlines
        // the wrappers, so they need it too.
        let needs_dataclass =
            self.needs_dataclass || (!project && (self.needs_option || self.needs_result));
        if needs_dataclass {
            code.push_str("from dataclasses import dataclass\n");
        }
        if self.needs_enum {
            code.push_str("from enum import Enum\n");
        }

        if project {
            // A stdlib program imports the shared prelude + namespaces from the
            // bundled `vbrpy` package — one definition of every class, so their
            // `isinstance` checks line up across `main.py` and `vbrpy`.
            let mut names: Vec<&str> = Vec::new();
            if self.needs_option {
                names.push("Some");
            }
            if self.needs_result {
                names.push("Ok");
                names.push("Err");
            }
            if self.needs_vb {
                names.push("_vb");
            }
            if self.needs_unwrap {
                names.push("_unwrap");
            }
            if self.needs_round {
                names.push("_vb_round");
            }
            if self.needs_val {
                names.push("_vb_val");
            }
            if self.needs_cdbl {
                names.push("_vb_cdbl");
            }
            if self.needs_clng {
                names.push("_vb_clng");
            }
            if self.needs_cint {
                names.push("_vb_cint");
            }
            if self.needs_input {
                names.push("_vb_input_box");
            }
            for ns in &self.stdlib_used {
                // DataFrame isn't a `vbrpy` class — it re-exports polars builders.
                if ns != "DataFrame" {
                    names.push(ns);
                }
            }
            for b in &self.df_builders {
                names.push(b);
            }
            code.push_str(&format!("from vbrpy import {}\n", names.join(", ")));
            code.push('\n');
        } else {
            let any_import = self.needs_math
                || self.needs_time
                || self.needs_sys
                || self.needs_random
                || needs_dataclass
                || self.needs_enum
                || !program.uses.is_empty();
            if any_import {
                code.push('\n');
            }
            // Single-file: inline the wrappers/helpers (`None` is Python's own).
            if self.needs_option {
                code.push_str(OPTION_CLASS);
                code.push('\n');
            }
            if self.needs_result {
                code.push_str(RESULT_CLASSES);
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
            if self.needs_val {
                code.push_str(VB_VAL_HELPER);
                code.push('\n');
            }
            if self.needs_cdbl {
                code.push_str(VB_CDBL_HELPER);
                code.push('\n');
            }
            if self.needs_clng {
                code.push_str(VB_CLNG_HELPER);
                code.push('\n');
            }
            if self.needs_cint {
                code.push_str(VB_CINT_HELPER);
                code.push('\n');
            }
            if self.needs_input {
                code.push_str(VB_INPUT_HELPER);
                code.push('\n');
            }
            if self.needs_unwrap {
                code.push_str(UNWRAP_HELPER);
                code.push('\n');
            }
        }
        code.push_str(&self.body);

        // Call `main` on run, mirroring the Rust entry point.
        if program.functions.iter().any(|f| f.receiver.is_none() && rust_name(&f.name) == "main") {
            code.push_str("\n\nif __name__ == \"__main__\":\n    main()\n");
        }

        // pip requirements: each `Use` pins its version (mirroring the Rust
        // side's reproducible Cargo pin), plus our own polars for a DataFrame
        // program (the parallel of `vbr_stdlib`'s `dataframe` Cargo feature) —
        // Python-polars versions independently of the Rust crate, so it takes a
        // floor rather than the crate's pin.
        let mut requirements: Vec<String> =
            program.uses.iter().map(|u| format!("{}=={}", u.crate_name, u.version)).collect();
        if self.stdlib_used.contains("DataFrame") {
            requirements.push("polars>=1.0".to_string());
        }

        PyProgram {
            code,
            warnings: self.warnings,
            stdlib_used: self.stdlib_used.into_iter().collect(),
            requirements,
        }
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

/// `Option`'s `Some` wrapper — `None` is Python's own singleton, so a match
/// reads `case Some(v):` / `case None:`.
const OPTION_CLASS: &str = "\
@dataclass
class Some:
    value: object
";

/// `Result`'s `Ok`/`Err` wrappers.
const RESULT_CLASSES: &str = "\
@dataclass
class Ok:
    value: object

@dataclass
class Err:
    error: object
";

/// `.Unwrap()` — returns the payload of a `Some`/`Ok`, raises on `None`/`Err`
/// (like Rust's `.unwrap()` panicking); a bare value passes through, so a
/// `dict.get(k).Unwrap()` works too.
const UNWRAP_HELPER: &str = "\
def _unwrap(x):
    if isinstance(x, (Some, Ok)):
        return x.value
    if isinstance(x, Err):
        raise Exception(f'unwrapped an Err: {x.error}')
    if x is None:
        raise Exception('unwrapped a None')
    return x
";

/// VB `Round` rounds half away from zero (as Rust's `f64::round` does), unlike
/// Python's banker's rounding — so `Round(2.5)` is `3`, matching `vbr run`.
const VB_ROUND_HELPER: &str = "\
import math as _math
def _vb_round(x):
    return _math.floor(x + 0.5) if x >= 0 else _math.ceil(x - 0.5)
";

/// `Val` — lenient parse, `0.0` on junk, matching Rust's `parse.unwrap_or(0.0)`.
const VB_VAL_HELPER: &str = "\
def _vb_val(s):
    try:
        return float(str(s).strip())
    except ValueError:
        return 0.0
";

/// `CDbl`/`CLng`/`CInt` — strict parse, `Result` like the Rust builtins.
const VB_CDBL_HELPER: &str = "\
def _vb_cdbl(s):
    try:
        return Ok(float(str(s).strip()))
    except ValueError as e:
        return Err(str(e))
";

const VB_CLNG_HELPER: &str = "\
def _vb_clng(s):
    try:
        return Ok(int(str(s).strip()))
    except ValueError as e:
        return Err(str(e))
";

const VB_CINT_HELPER: &str = "\
def _vb_cint(s):
    try:
        n = int(str(s).strip())
        if n < -2147483648 or n > 2147483647:
            return Err('out of range')
        return Ok(n)
    except ValueError as e:
        return Err(str(e))
";

/// `InputBox` — prompt + one line; closed input fails, a blank Enter is `\"\"`.
const VB_INPUT_HELPER: &str = "\
def _vb_input_box(prompt):
    try:
        return Ok(input(prompt))
    except EOFError:
        return Err('end of input')
";

/// A coarse numeric class used only to choose integer (`//`) vs float (`/`)
/// division — the one place Python and Rust arithmetic diverge on operand type.
#[derive(Clone, Copy, PartialEq)]
enum Num {
    Int,
    Float,
    Unknown,
}

/// Strip the common leading whitespace an editor added to a `Python` block
/// (Python is whitespace-sensitive) and drop blank edge lines, keeping each
/// line's own relative indentation.
fn dedent_lines(raw: &str) -> Vec<String> {
    let min = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    let mut lines: Vec<String> = raw
        .lines()
        .map(|l| if l.len() >= min { l[min..].to_string() } else { l.to_string() })
        .collect();
    while lines.first().is_some_and(|l| l.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    lines
}

/// A Python string literal. **Single-quoted on purpose**: f-strings are
/// double-quoted, so a string literal interpolated inside one (`f"{d['k']}"`)
/// never clashes quotes — which keeps the output valid on Python < 3.12 too
/// (nested same-quotes in an f-string are only legal from 3.12).
fn py_str(s: &str) -> String {
    format!("'{}'", py_escape_sq(s))
}

/// The literal-text portion of a (double-quoted) f-string: the usual escapes,
/// plus `{`/`}` doubled so they aren't read as interpolations.
fn fstring_text(s: &str) -> String {
    py_escape_dq(s).replace('{', "{{").replace('}', "}}")
}

/// Escape for a single-quoted literal.
fn py_escape_sq(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

/// Escape for a double-quoted context (f-string text).
fn py_escape_dq(s: &str) -> String {
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

fn py_body_diverges(stmts: &[Stmt]) -> bool {
    match stmts.iter().rev().find(|s| !matches!(s, Stmt::Comment(_) | Stmt::LineMark(_))) {
        Some(Stmt::Return(_) | Stmt::RaiseError(_) | Stmt::Break | Stmt::Continue) => true,
        Some(Stmt::If { branches, else_body }) => {
            else_body.as_ref().is_some_and(|e| py_body_diverges(e))
                && branches.iter().all(|(_, b)| py_body_diverges(b))
        }
        Some(Stmt::Match { arms, .. }) => !arms.is_empty() && arms.iter().all(|a| py_body_diverges(&a.body)),
        Some(Stmt::HandleErr { body, .. }) => py_body_diverges(body),
        _ => false,
    }
}

fn stmt_name(s: &Stmt) -> &'static str {
    match s {
        Stmt::Dim { .. } => "Dim",
        Stmt::Set { .. } => "Set",
        Stmt::Destroy { .. } => "= Nothing",
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
        Stmt::GpuInto { .. } => "Into",
        Stmt::Assert(_) => "Assert",
        Stmt::RaiseError(_) => "RaiseError",
        Stmt::HandleErr { .. } => "Handle",
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
        ExprKind::Raw(_) => "Raw",
        ExprKind::Try(_) => "error propagation (?)",
        _ => "expression",
    }
}
