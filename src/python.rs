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
    // Every enum type name, and the subset that carry data (a sum type). A
    // C-like enum lowers to `enum.Enum`; a data one to a base class + a
    // `@dataclass` per variant, so construction and patterns differ.
    enum_names: HashSet<String>,
    data_enums: HashSet<String>,
    // Per-`Match` counter, so the scrutinee temp (`_m0`) is unique and matches
    // can nest.
    match_counter: usize,
    // A light, function-local type map (reset per function): enough to pick `//`
    // vs `/` for division, and to tell a dict `.insert` from a list `.insert`.
    var_types: std::collections::HashMap<String, DeclType>,
    // The current function's return type — tells a `?` whether to propagate an
    // `Err` (Result) or a `None` (Option).
    current_ret: Option<DeclType>,
    // Temp counter for `?` hoisting (`_t0`).
    tmp_counter: usize,
    // Prelude features, switched on as the body needs them.
    needs_vb: bool,
    needs_round: bool,
    needs_math: bool,
    needs_dataclass: bool,
    needs_enum: bool,
    needs_option: bool,
    needs_result: bool,
    needs_unwrap: bool,
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
        self.enum_names = program.enums.iter().map(|e| e.name.clone()).collect();
        self.data_enums = program
            .enums
            .iter()
            .filter(|e| e.variants.iter().any(|v| !v.payload.is_empty()))
            .map(|e| e.name.clone())
            .collect();

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
        self.var_types.clear();
        self.current_ret = func.ret.clone();
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
            self.var_types.insert(rust_name(&p.name), p.ty.clone());
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
                    Some(e) => self.hoist_value(e, indent),
                    None => self.default_value(ty),
                };
                let hint = self.type_hint(ty);
                self.var_types.insert(rust_name(name), ty.clone());
                self.line(indent, &format!("{}: {} = {}", rust_name(name), hint, value));
            }
            Stmt::Assign { target, value, op } => {
                let t = self.expr(target);
                let v = self.hoist_value(value, indent);
                match op {
                    Some(o) => self.line(indent, &format!("{} {}= {}", t, self.bin_op(*o), v)),
                    None => self.line(indent, &format!("{} = {}", t, v)),
                }
            }
            Stmt::Return(Some(e)) => {
                let v = self.hoist_value(e, indent);
                self.line(indent, &format!("return {}", v));
            }
            Stmt::Return(None) => self.line(indent, "return"),
            Stmt::Expr(e) => {
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
            Stmt::For { var, from, to, step, body } => {
                // A `For` counter over an integer range is an int (used by `//`).
                self.var_types.insert(rust_name(var), DeclType::Plain(Type::Long));
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
            Stmt::Match { scrutinee, arms, .. } => self.match_stmt(scrutinee, arms, indent),
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
    /// plus an optional guard fragment (ranges become a guard since Python has no
    /// range pattern). `subj` is the scrutinee temp the range guard reads.
    fn translate_pattern(&mut self, pattern: &str, subj: &str) -> (String, Option<String>) {
        let toks: Vec<&str> = pattern.split_whitespace().collect();
        if toks == ["_"] {
            return ("_".into(), None);
        }
        // A range (`90 ..= 99` / `1 .. 5`) → a guarded wildcard.
        if let Some(pos) = toks.iter().position(|t| *t == "..=" || *t == "..") {
            let lo = self.pattern_literal(&toks[..pos].join(" "));
            let hi = self.pattern_literal(&toks[pos + 1..].join(" "));
            let op = if toks[pos] == "..=" { "<=" } else { "<" };
            return ("_".into(), Some(format!("{} <= {} {} {}", lo, subj, op, hi)));
        }
        // Everything else — constructors, enum paths, alternation, captures,
        // literals, and their nestings (`Err(MathError::Custom(msg))`).
        (self.pat_to_py(&toks), None)
    }

    /// Translate one pattern (recursively). Alternation is split first, then each
    /// alternative is a `primary`.
    fn pat_to_py(&mut self, toks: &[&str]) -> String {
        let alts = split_top_level(toks, "|");
        if alts.len() > 1 {
            return alts
                .iter()
                .map(|a| self.pat_primary(a))
                .collect::<Vec<_>>()
                .join(" | ");
        }
        self.pat_primary(toks)
    }

    fn pat_primary(&mut self, toks: &[&str]) -> String {
        match toks {
            [] | ["_"] => "_".into(),
            ["true"] => "True".into(),
            ["false"] => "False".into(),
            ["None"] => "None".into(),
            _ => {
                // `Head( args )` — a constructor (Some/Ok/Err or enum variant).
                if let Some(lp) = toks.iter().position(|t| *t == "(") {
                    let head = &toks[..lp];
                    let inner = &toks[lp + 1..toks.len().saturating_sub(1)];
                    let args: Vec<String> =
                        split_top_level(inner, ",").iter().map(|g| self.pat_to_py(g)).collect();
                    return self.ctor_pattern(head, &args);
                }
                // A bare enum path with no payload (`Enum::Variant`).
                if let Some(pos) = toks.iter().position(|t| *t == "::") {
                    let qualifier = toks[..pos].join("");
                    let variant = toks[pos + 1..].join("");
                    return if self.data_enums.contains(&qualifier) {
                        format!("{}()", variant)
                    } else {
                        format!("{}.{}", qualifier, variant)
                    };
                }
                // A single token: capture, literal, or bool.
                self.pattern_literal(&toks.join(" "))
            }
        }
    }

    /// A constructor pattern `Head(args)` → Python. `Head` is `Some`/`Ok`/`Err`
    /// (a prelude wrapper) or a qualified data-enum variant (`Enum::Variant`,
    /// whose class is just `Variant`).
    fn ctor_pattern(&mut self, head: &[&str], args: &[String]) -> String {
        if let Some(pos) = head.iter().position(|t| *t == "::") {
            let variant = head[pos + 1..].join("");
            return format!("{}({})", variant, args.join(", "));
        }
        let name = head.join("");
        match name.as_str() {
            "Some" => self.needs_option = true,
            "Ok" | "Err" => self.needs_result = true,
            _ => {}
        }
        format!("{}({})", name, args.join(", "))
    }

    /// One literal/capture token → its Python spelling (`true`→`True`, `- 5`→`-5`,
    /// a bare name stays a capture).
    fn pattern_literal(&self, s: &str) -> String {
        match s {
            "true" => "True".into(),
            "false" => "False".into(),
            _ => s.replace(' ', ""),
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

    /// A method call → its Python form. The curated table turns Rust/VBR method
    /// names into Python idioms (`.push`→`.append`, `.len()`→`len()`, iterator
    /// chains → comprehensions); anything unrecognised passes straight through.
    fn method_call(&mut self, recv: &Expr, method: &str, args: &[Expr]) -> String {
        let m = method.to_ascii_lowercase();

        // Terminal consumers.
        match m.as_str() {
            "collect" => return self.render_iter(recv),
            "sum" => return format!("sum({})", self.expr(recv)),
            "count" | "len" => return format!("len({})", self.expr(recv)),
            "any" => return self.quantifier(recv, args, "any"),
            "all" => return self.quantifier(recv, args, "all"),
            _ => {}
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

        // Option-returning consumers → wrapped in `Some(...)` / `None`, so the
        // result matches the same way a Rust `Option` does.
        match m.as_str() {
            "find" if args.len() == 1 => {
                if let Some((v, cond)) = self.closure_parts(&args[0]) {
                    self.needs_option = true;
                    let src = self.expr(recv);
                    return format!("next((Some({v}) for {v} in {src} if {cond}), None)");
                }
            }
            "position" if args.len() == 1 => {
                if let Some((v, cond)) = self.closure_parts(&args[0]) {
                    self.needs_option = true;
                    let src = self.expr(recv);
                    return format!("next((Some(_i) for _i, {v} in enumerate({src}) if {cond}), None)");
                }
            }
            "max" | "min" if args.is_empty() => {
                self.needs_option = true;
                let src = self.expr(recv);
                return format!("(Some({m}({src})) if {src} else None)");
            }
            _ => {}
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

    /// A (`.collect()`'d) iterator chain → a Python list expression. `filter`/
    /// `map` become comprehensions; `take`/`skip`/`rev` become slices; the
    /// recursion bottoms out at the base receiver (a Vec/list). An unrecognised
    /// adapter is wrapped in `list(...)` with a warning.
    fn render_iter(&mut self, e: &Expr) -> String {
        if let ExprKind::MethodCall { recv, method, args } = &e.kind {
            let m = method.to_ascii_lowercase();
            match m.as_str() {
                "filter" if args.len() == 1 => {
                    if let Some((v, cond)) = self.closure_parts(&args[0]) {
                        let src = self.render_iter(recv);
                        return format!("[{v} for {v} in {src} if {cond}]");
                    }
                }
                "map" if args.len() == 1 => {
                    if let Some((v, body)) = self.closure_parts(&args[0]) {
                        let src = self.render_iter(recv);
                        return format!("[{body} for {v} in {src}]");
                    }
                }
                "take" if args.len() == 1 => {
                    let src = self.render_iter(recv);
                    let n = self.expr(&args[0]);
                    return format!("{src}[:{n}]");
                }
                "skip" if args.len() == 1 => {
                    let src = self.render_iter(recv);
                    let n = self.expr(&args[0]);
                    return format!("{src}[{n}:]");
                }
                "rev" if args.is_empty() => {
                    let src = self.render_iter(recv);
                    return format!("{src}[::-1]");
                }
                "collect" => return self.render_iter(recv),
                _ => {}
            }
        }
        self.expr(e)
    }

    /// Render a value expression, hoisting a top-level `?` (`Try`) into the
    /// preceding lines: bind the operand to a temp, early-return on failure
    /// (`Err` for a Result-returning function, `None` for an Option one), and
    /// yield the unwrapped `.value`. A `?` nested deeper than the whole value is
    /// warned (not in idiomatic use — VB writes `Dim x = f()?`).
    fn hoist_value(&mut self, e: &Expr, indent: usize) -> String {
        let ExprKind::Try(inner) = &e.kind else {
            return self.expr(e);
        };
        let tmp = format!("_t{}", self.tmp_counter);
        self.tmp_counter += 1;
        let val = self.expr(inner);
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

    /// A single-parameter closure `|v| body` → its `(var, body)` in Python.
    fn closure_parts(&mut self, e: &Expr) -> Option<(String, String)> {
        if let ExprKind::Closure { params, body, .. } = &e.kind {
            if params.len() == 1 {
                let var = rust_name(&params[0]);
                let b = self.expr(body);
                return Some((var, b));
            }
        }
        None
    }

    /// `.any(|v| cond)` / `.all(…)` → a generator quantifier.
    fn quantifier(&mut self, recv: &Expr, args: &[Expr], kind: &str) -> String {
        if args.len() == 1 {
            if let ExprKind::Closure { params, body, .. } = &args[0].kind {
                if params.len() == 1 {
                    let var = rust_name(&params[0]);
                    let src = self.expr(recv);
                    let cond = self.expr(body);
                    return format!("{}({} for {} in {})", kind, cond, var, src);
                }
            }
        }
        self.warn(format!("`.{}(…)` needs a single-parameter closure — wrapped as-is.", kind));
        format!("{}({})", kind, self.expr(recv))
    }

    /// Is `recv` known to be a `Map`/`HashMap` (so `.insert` is a subscript)?
    fn recv_is_map(&self, recv: &Expr) -> bool {
        matches!(
            &recv.kind,
            ExprKind::Ident(name) if matches!(self.var_types.get(&rust_name(name)), Some(DeclType::Map(_, _)))
        )
    }

    /// A coarse numeric class for the `//` vs `/` division choice.
    fn numeric(&self, e: &Expr) -> Num {
        match &e.kind {
            ExprKind::Int(_) => Num::Int,
            ExprKind::Float(_) => Num::Float,
            ExprKind::Ident(name) => match self.var_types.get(&rust_name(name)) {
                Some(DeclType::Plain(Type::Integer | Type::Long | Type::LongLong | Type::Byte)) => {
                    Num::Int
                }
                Some(DeclType::Plain(Type::Single | Type::Double)) => Num::Float,
                _ => Num::Unknown,
            },
            ExprKind::Binary { op, lhs, rhs }
                if matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Mod) =>
            {
                match (self.numeric(lhs), self.numeric(rhs)) {
                    (Num::Float, _) | (_, Num::Float) => Num::Float,
                    (Num::Int, Num::Int) => Num::Int,
                    _ => Num::Unknown,
                }
            }
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
                    self.method_call(recv, method, args)
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
            ExprKind::Try(inner) => {
                // A `?` reaching here is nested inside a larger expression; only a
                // top-level `?` (handled by `hoist_value`) can early-return.
                self.warn(
                    "`?` is only lowered as the whole right-hand side of a Dim/assignment/Return \
                     — nested use isn't supported yet.",
                );
                self.expr(inner)
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
            _ => {}
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
            // Option/Result are modelled as the prelude `Some`/`None`/`Ok`/`Err`
            // wrappers, not a bare union — so `object` is the honest annotation.
            DeclType::Option(_) => "object".into(),
            DeclType::Result(_, _) => "object".into(),
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
        // Option/Result wrappers are dataclasses too.
        let needs_dataclass = self.needs_dataclass || self.needs_option || self.needs_result;
        if self.needs_math {
            code.push_str("import math\n");
        }
        if needs_dataclass {
            code.push_str("from dataclasses import dataclass\n");
        }
        if self.needs_enum {
            code.push_str("from enum import Enum\n");
        }
        if self.needs_math || needs_dataclass || self.needs_enum {
            code.push('\n');
        }
        // The Option/Result wrappers (`None` is Python's own).
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
        if self.needs_unwrap {
            code.push_str(UNWRAP_HELPER);
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

/// A coarse numeric class used only to choose integer (`//`) vs float (`/`)
/// division — the one place Python and Rust arithmetic diverge on operand type.
#[derive(Clone, Copy, PartialEq)]
enum Num {
    Int,
    Float,
    Unknown,
}

/// Split a flat token slice on a separator token that sits at paren depth 0
/// (so `,`/`|` inside a nested `Custom(msg)` don't split it).
fn split_top_level<'a>(toks: &[&'a str], sep: &str) -> Vec<Vec<&'a str>> {
    let mut groups: Vec<Vec<&str>> = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    let mut depth = 0i32;
    for t in toks {
        match *t {
            "(" | "[" => {
                depth += 1;
                cur.push(*t);
            }
            ")" | "]" => {
                depth -= 1;
                cur.push(*t);
            }
            s if s == sep && depth == 0 => groups.push(std::mem::take(&mut cur)),
            _ => cur.push(*t),
        }
    }
    groups.push(cur);
    groups
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
