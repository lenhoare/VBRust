//! AST in, idiomatic Rust source out.
//!
//! Two small but important touches even at this slice:
//!  * a mutability pre-scan, so a variable that is reassigned becomes `let mut`
//!    (Rust requires it; VB never made you think about it);
//!  * identifier renaming to snake_case, consistently at declaration and use.

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diagnostics::Diagnostics;
use crate::resolver::{self, FnTable};

/// CLI stand-in for VB's InputBox: print the prompt, read a line, return it.
const INPUT_BOX_HELPER: &str = "fn input_box(prompt: &str) -> String {
    use std::io::Write;
    print!(\"{}\", prompt);
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    line.trim_end().to_string()
}
";

pub fn transpile(program: &Program, diags: &mut Diagnostics) -> String {
    transpile_module(program, &[], true, false, diags)
}

/// Transpile one file of a project. `modules` are the other project module names
/// (snake-cased), used to qualify cross-module calls; when `is_entry`, the file
/// is the crate root and gets `mod <name>;` declarations and `fn main`. `web`
/// targets the browser (`vbr runweb`): a `Screen` renders through Ratzilla
/// instead of crossterm (a `Page` is web by nature; a `Window` ignores it).
pub fn transpile_module(
    program: &Program,
    modules: &[String],
    is_entry: bool,
    web: bool,
    diags: &mut Diagnostics,
) -> String {
    // A `Css` block is a Page's stylesheet — it has no meaning anywhere else.
    if !program.css.is_empty() && program.pages.is_empty() {
        diags.error_once(
            "css-no-page",
            "`Css … End Css` styles a `Page` (a browser app) — this program has none. \
             A `Window` or `Screen` is themed with `Theme` instead.",
        );
    }
    // A web program (one with a `Page`) compiles to a Yew (WebAssembly) app.
    if !program.pages.is_empty() {
        if !program.windows.is_empty() || !program.screens.is_empty() {
            diags.error_once(
                "mixed-surfaces",
                "A program can't mix `Page` with `Window`/`Screen` — each program is one \
                 kind of app. Split them into separate programs.",
            );
        }
        let rust = crate::web::emit_web_program(program, diags);
        diags.clear_line_map();
        return rust;
    }
    // A GUI program (one with a `Window`) compiles to an Iced application: the
    // window definitions plus a `fn main` that launches the one `Function Main()`
    // names with `<Window>.Run`.
    if !program.windows.is_empty() {
        let rust = crate::gui::emit_gui_program(program, diags);
        // The GUI emitter assembles sections out of order, so its line
        // checkpoints would mislead — drop them rather than lie.
        diags.clear_line_map();
        return rust;
    }
    // A TUI program (one with a `Screen`) compiles to a ratatui application —
    // in the terminal (crossterm) by default, in the browser (Ratzilla) for
    // `vbr runweb`. Same state, same view; only the shell differs.
    if !program.screens.is_empty() {
        let rust = crate::tui::emit_tui_program(program, web, diags);
        diags.clear_line_map();
        return rust;
    }
    // Fire the one-time teaching notes for builtins before generating code,
    // keeping the rendering functions pure.
    for func in &program.functions {
        note_builtins(&func.body, diags);
    }
    mark_stdlib_types(program, diags);
    let module_set: HashSet<String> = modules.iter().cloned().collect();
    // Enum names — a reference like `Color.Red` resolves to the path `Color::Red`.
    let enum_set: HashSet<String> = program.enums.iter().map(|e| e.name.clone()).collect();

    let fns = resolver::build_fn_table(program);
    let methods = resolver::build_method_table(program);
    let consts = resolver::build_const_map(program);
    let structs = resolver::build_struct_table(program);

    let mut out = String::new();
    for comment in &program.leading_comments {
        out.push_str(&format!("// {}\n", comment));
    }
    if !program.leading_comments.is_empty() {
        out.push('\n');
    }
    // The crate root declares each sibling module (alphabetical, for stable output).
    if is_entry && !modules.is_empty() {
        let mut mods: Vec<&String> = modules.iter().collect();
        mods.sort();
        for m in mods {
            out.push_str(&format!("mod {};\n", m));
        }
        out.push('\n');
    }
    // Pull in HashMap automatically when it's used.
    if program.functions.iter().any(|f| body_uses_hashmap(&f.body)) {
        out.push_str("use std::collections::HashMap;\n\n");
    }
    // Pull in the stdlib namespaces that were used (marked during note_builtins).
    let std_used: Vec<&str> = STDLIB_TYPES
        .iter()
        .filter(|t| diags.has_mark(&format!("stdlib:{}", t)))
        .copied()
        .collect();
    if !std_used.is_empty() {
        out.push_str(&format!("use vbr_stdlib::{{{}}};\n\n", std_used.join(", ")));
    }
    // A dataframe program also needs the polars expression builders that column
    // formulas lower to (`col("x")`, `lit(3)`, `when(...)`), re-exported by the
    // wrapper so the generated code has a single dependency.
    if std_used.contains(&"DataFrame") {
        // The polars expression builders column formulas lower to. A given
        // program may not use all three, so silence the unused-import lint.
        out.push_str("#[allow(unused_imports)]\n");
        out.push_str("use vbr_stdlib::dataframe::{col, lit, when};\n\n");
    }

    // Top-level items, separated by a single blank line: structs, then impl
    // blocks (methods grouped by receiver), then free functions.
    let mut first_item = true;
    let mut sep = |out: &mut String| {
        if !first_item {
            out.push('\n');
        }
        first_item = false;
    };

    // Runtime helper for InputBox, emitted only when it's used.
    if diags.has_mark("input_box") {
        sep(&mut out);
        out.push_str(INPUT_BOX_HELPER);
    }

    if !program.constants.is_empty() {
        sep(&mut out);
        for c in &program.constants {
            emit_const(c, &mut out, diags);
        }
    }

    for s in &program.structs {
        sep(&mut out);
        emit_struct(s, diags, &mut out);
    }

    for e in &program.enums {
        sep(&mut out);
        emit_enum(e, &mut out);
    }

    // Receivers in first-seen order.
    let mut receivers: Vec<&String> = Vec::new();
    for f in &program.functions {
        if let Some(r) = &f.receiver {
            if !receivers.contains(&r) {
                receivers.push(r);
            }
        }
    }
    for recv in receivers {
        sep(&mut out);
        emit_impl(
            recv, program, &fns, &methods, &consts, &module_set, &enum_set, &structs, diags,
            &mut out,
        );
    }

    for func in program.functions.iter().filter(|f| f.receiver.is_none()) {
        sep(&mut out);
        emit_fn(
            func, &fns, &methods, &consts, &module_set, &enum_set, &structs, diags, &mut out, 0,
            None,
        );
    }
    out
}

pub(crate) fn emit_const(c: &ConstDef, out: &mut String, diags: &mut Diagnostics) {
    let name = to_screaming(&c.name);
    if name != c.name {
        diags.warn(
            c.line,
            format!(
                "Constant '{}' is '{}' on the Rust side — constants are uppercased.",
                c.name, name
            ),
        );
    }
    if c.name.eq_ignore_ascii_case("pi") {
        diags.warn(
            c.line,
            "Rust already provides PI — prefer `std::f64::consts::PI` over your own constant.",
        );
    }
    let vis = if c.public { "pub " } else { "" };
    // A const String must be a &str (no owned String in const position).
    let ty = if c.ty == Type::Text {
        "&str".to_string()
    } else {
        c.ty.rust().to_string()
    };
    out.push_str(&format!(
        "{}const {}: {} = {};\n",
        vis,
        name,
        ty,
        render_expr(&c.value, Some(c.ty))
    ));
}

pub(crate) fn emit_impl(
    recv: &str,
    program: &Program,
    fns: &FnTable,
    methods: &resolver::MethodTable,
    consts: &HashMap<String, String>,
    modules: &HashSet<String>,
    enums: &HashSet<String>,
    structs: &resolver::StructTable,
    diags: &mut Diagnostics,
    out: &mut String,
) {
    out.push_str(&format!("impl {} {{\n", recv));
    let mut first = true;
    for f in program
        .functions
        .iter()
        .filter(|f| f.receiver.as_deref() == Some(recv))
    {
        if !first {
            out.push('\n');
        }
        first = false;
        let mutates = methods
            .get(&(recv.to_string(), rust_name(&f.name)))
            .copied()
            .unwrap_or(false);
        let self_param = if mutates { "&mut self" } else { "&self" };
        emit_fn(f, fns, methods, consts, modules, enums, structs, diags, out, 1, Some(self_param));
    }
    out.push_str("}\n");
}

/// Emit a simple enum: `#[derive(…)] enum Name { A, B, C }`. `Copy` so values can
/// be matched/compared freely and used where Iced wants a `Copy` value.
pub(crate) fn emit_enum(e: &EnumDef, out: &mut String) {
    let kw = if e.public { "pub enum" } else { "enum" };
    out.push_str(&format!("#[derive({})]\n", enum_derives(e)));
    // A variant matched-on but never constructed (defensive `Match` arms) is a
    // normal, legitimate pattern — don't warn about it.
    out.push_str("#[allow(dead_code)]\n");
    out.push_str(&format!("{} {} {{\n", kw, e.name));
    for v in &e.variants {
        if v.payload.is_empty() {
            out.push_str(&format!("    {},\n", v.name));
        } else {
            let types: Vec<String> = v.payload.iter().map(decltype_rust).collect();
            out.push_str(&format!("    {}({}),\n", v.name, types.join(", ")));
        }
    }
    out.push_str("}\n");
}

/// The derive set for an enum, computed from its variant payloads (all primitives
/// or `String` in V1): `Debug`/`Clone`/`PartialEq` always; `Copy` unless a
/// `String` payload; `Eq` unless a float payload. An all-unit enum gets the full
/// set (same as a simple enum).
fn enum_derives(e: &EnumDef) -> String {
    let payloads: Vec<&DeclType> = e.variants.iter().flat_map(|v| &v.payload).collect();
    // Debug/Clone hold for every payload type. Copy only for Copy primitives (not
    // String/Vec/struct/…). PartialEq/Eq only when every payload is a primitive or
    // String — a struct payload derives neither (structs are Debug+Clone only), so
    // we drop them rather than emit a derive that won't compile.
    let copy = payloads.iter().all(|t| matches!(t, DeclType::Plain(p) if !matches!(p, Type::Text)));
    let partial_eq = payloads.iter().all(|t| matches!(t, DeclType::Plain(_)));
    let eq = payloads
        .iter()
        .all(|t| matches!(t, DeclType::Plain(p) if !matches!(p, Type::Single | Type::Double)));
    let mut d = vec!["Debug", "Clone"];
    if copy {
        d.push("Copy");
    }
    if partial_eq {
        d.push("PartialEq");
    }
    if eq {
        d.push("Eq");
    }
    d.join(", ")
}

pub(crate) fn emit_struct(s: &StructDef, diags: &mut Diagnostics, out: &mut String) {
    let kw = if s.public { "pub struct" } else { "struct" };
    // Debug + Clone are safe for every VBR field type (primitives, String,
    // collections, other structs) and let structs be printed and copied — e.g.
    // a `Vec<Struct>` state field snapshotted into a canvas.
    out.push_str("#[derive(Debug, Clone)]\n");
    out.push_str(&format!("{} {} {{\n", kw, s.name));
    for f in &s.fields {
        let fname = rust_name(&f.name);
        if fname != f.name {
            diags.warn_once_global(
                "struct-field-case",
                "Struct field names are lowercased for Rust (`FirstName` → `firstname`).",
            );
        }
        let vis = if f.public { "pub " } else { "" };
        out.push_str(&format!("    {}{}: {},\n", vis, fname, decltype_rust(&f.ty)));
    }
    out.push_str("}\n");
}

/// Render a `DeclType` as its Rust type, recursively. `Result<T>` injects the
/// `String` error type at any nesting depth (`Result<T, String>`).
pub(crate) fn decltype_rust(ty: &DeclType) -> String {
    match ty {
        DeclType::Plain(t) => t.rust().to_string(),
        // `PyObject` is the opaque inline-Python handle type — a GIL-independent
        // owned reference to a Python value VBR has no type for.
        DeclType::Named(n) if n == "PyObject" => "pyo3::Py<pyo3::PyAny>".to_string(),
        DeclType::Named(n) => n.clone(),
        DeclType::Tuple(ts) => {
            let parts: Vec<String> = ts.iter().map(decltype_rust).collect();
            format!("({})", parts.join(", "))
        }
        DeclType::Vec(t) => format!("Vec<{}>", decltype_rust(t)),
        DeclType::Map(k, v) => format!("HashMap<{}, {}>", decltype_rust(k), decltype_rust(v)),
        DeclType::Result(t, e) => format!("Result<{}, {}>", decltype_rust(t), decltype_rust(e)),
        DeclType::Option(t) => format!("Option<{}>", decltype_rust(t)),
        DeclType::Array(t, n) => format!("[{}; {}]", t.rust(), n),
        DeclType::Array2D(t, r, c) => format!("[[{}; {}]; {}]", t.rust(), c, r),
    }
}

/// Does any `Dim` in these statements declare a `HashMap`? (Recurses blocks.)
fn body_uses_hashmap(stmts: &[Stmt]) -> bool {
    stmts.iter().any(|s| match s {
        Stmt::Dim { ty: DeclType::Map(..), .. } => true,
        Stmt::If { branches, else_body } => {
            branches.iter().any(|(_, b)| body_uses_hashmap(b))
                || else_body.as_ref().map_or(false, |b| body_uses_hashmap(b))
        }
        Stmt::For { body, .. } | Stmt::ForEach { body, .. } => body_uses_hashmap(body),
        Stmt::Match { arms, .. } => arms.iter().any(|a| body_uses_hashmap(&a.body)),
        _ => false,
    })
}

pub(crate) fn emit_fn(
    func: &Function,
    fns: &FnTable,
    methods: &resolver::MethodTable,
    consts: &HashMap<String, String>,
    modules: &HashSet<String>,
    enums: &HashSet<String>,
    structs: &resolver::StructTable,
    diags: &mut Diagnostics,
    out: &mut String,
    base_indent: usize,
    self_param: Option<&str>,
) {
    let pad = "    ".repeat(base_indent);
    let name = rust_fn_name(&func.name, func.line, diags);

    let mut params: Vec<String> = Vec::new();
    if let Some(sp) = self_param {
        params.push(sp.to_string());
    }
    params.extend(func.params.iter().map(render_param));

    let ret = match &func.ret {
        Some(t) => format!(" -> {}", decltype_rust(t)),
        None => String::new(),
    };
    // Only a plain return type drives literal coercion of the tail expression;
    // an Ok/Some/tuple wrapper carries its own type.
    let tail_expected = match &func.ret {
        Some(DeclType::Plain(t)) => Some(*t),
        _ => None,
    };
    // `Public Function` → `pub fn`, so other modules can call it.
    let vis = if func.public { "pub " } else { "" };
    // Checkpoint the header too, so signature-level rustc errors map back.
    diags.map_line(out.matches('\n').count() + 1, func.line);
    out.push_str(&format!("{}{}fn {}({}){} {{\n", pad, vis, name, params.join(", "), ret));

    // `FunctionName = value` is really a return — rewrite it before emitting.
    let mut body = func.body.clone();
    convert_returns(&mut body, &name);

    // The ByRef parameters of *this* function — their uses get dereferenced.
    let byref: HashSet<String> = func
        .params
        .iter()
        .filter(|p| p.mode == ParamMode::ByRef)
        .map(|p| rust_name(&p.name))
        .collect();

    // Resolver rewrites the body (&mut at call sites, *deref of ByRef params,
    // `as` casts for numeric coercions) and tells us which locals were lent.
    // `?` is only valid when this function can itself fail (returns Result/Option).
    let can_propagate = matches!(
        func.ret,
        Some(DeclType::Result(..)) | Some(DeclType::Option(_))
    );
    let passed_by_ref = resolver::resolve_body(
        &mut body,
        &func.params,
        fns,
        methods,
        consts,
        modules,
        enums,
        structs,
        func.receiver.as_deref(),
        tail_expected,
        can_propagate,
        diags,
    );

    // Which locals need `let mut`: those reassigned, plus those lent mutably.
    let mut mutated = HashSet::new();
    collect_mutated(&body, &mut mutated);
    mutated.extend(passed_by_ref);

    emit_fn_body(&body, &mutated, &byref, tail_expected, diags, out, base_indent + 1);
    out.push_str(&format!("{}}}\n", pad));
}

fn render_param(p: &Param) -> String {
    let ty = match (&p.mode, &p.ty) {
        // ByVal String borrows as a read-only &str.
        (ParamMode::ByVal, DeclType::Plain(Type::Text)) => "&str".to_string(),
        // ByVal fixed-size primitive / tuple: pass by value.
        (ParamMode::ByVal, DeclType::Plain(t)) => t.rust().to_string(),
        (ParamMode::ByVal, DeclType::Tuple(_)) => decltype_rust(&p.ty),
        // ByVal Result/Option: taken by value (owned) — they carry an outcome to
        // consume, not a container to read through a borrow.
        (ParamMode::ByVal, dt @ (DeclType::Result(..) | DeclType::Option(_))) => decltype_rust(dt),
        // ByVal struct/collection (incl. Vec/HashMap): immutable borrow.
        (ParamMode::ByVal, dt) => format!("&{}", decltype_rust(dt)),
        // ByRef: a mutable borrow of whatever it is.
        (ParamMode::ByRef, dt) => format!("&mut {}", decltype_rust(dt)),
    };
    format!("{}: {}", rust_name(&p.name), ty)
}

/// Emit a function body, rendering a trailing `Return value` as an idiomatic
/// tail expression (no `return`, no semicolon) the way the spec shows.
fn emit_fn_body(
    stmts: &[Stmt],
    mutated: &HashSet<String>,
    byref: &HashSet<String>,
    ret: Option<Type>,
    diags: &mut Diagnostics,
    out: &mut String,
    indent: usize,
) {
    let pad = "    ".repeat(indent);
    // The tail expression is the last *non-comment* statement (line marks
    // don't count either), so a trailing inline comment doesn't rob a
    // `Return` of its idiomatic tail form.
    let last_real = stmts
        .iter()
        .rposition(|s| !matches!(s, Stmt::Comment(_) | Stmt::LineMark(_)));
    if let Some(l) = last_real {
        if let Stmt::Return(Some(e)) = &stmts[l] {
            for stmt in &stmts[..l] {
                emit_stmt(stmt, mutated, byref, indent, diags, out);
            }
            // Any trailing comments are emitted just above the returned value.
            for stmt in &stmts[l + 1..] {
                emit_stmt(stmt, mutated, byref, indent, diags, out);
            }
            out.push_str(&format!("{}{}\n", pad, render_expr(e, ret)));
            return;
        }
    }
    for stmt in stmts {
        emit_stmt(stmt, mutated, byref, indent, diags, out);
    }
}

/// Rewrite `FunctionName = value` (assignment to the function's own name) into
/// a `Return`, recursing through nested blocks.
fn convert_returns(stmts: &mut [Stmt], fn_name: &str) {
    for stmt in stmts.iter_mut() {
        match stmt {
            Stmt::Assign {
                target: Expr::Ident(name),
                value,
                op: None,
            } if rust_name(name) == fn_name => {
                *stmt = Stmt::Return(Some(value.clone()));
            }
            Stmt::If {
                branches,
                else_body,
            } => {
                for (_, body) in branches.iter_mut() {
                    convert_returns(body, fn_name);
                }
                if let Some(body) = else_body {
                    convert_returns(body, fn_name);
                }
            }
            Stmt::For { body, .. } | Stmt::ForEach { body, .. } | Stmt::DoLoop { body, .. } => {
                convert_returns(body, fn_name)
            }
            Stmt::Match { arms, .. } => {
                for arm in arms.iter_mut() {
                    convert_returns(&mut arm.body, fn_name);
                }
            }
            _ => {}
        }
    }
}

fn emit_block(
    stmts: &[Stmt],
    mutated: &HashSet<String>,
    byref: &HashSet<String>,
    indent: usize,
    diags: &mut Diagnostics,
    out: &mut String,
) {
    for stmt in stmts {
        emit_stmt(stmt, mutated, byref, indent, diags, out);
    }
}

pub(crate) fn emit_stmt(
    stmt: &Stmt,
    mutated: &HashSet<String>,
    byref: &HashSet<String>,
    indent: usize,
    diags: &mut Diagnostics,
    out: &mut String,
) {
    let pad = "    ".repeat(indent);
    match stmt {
        Stmt::LineMark(vbr_line) => {
            // Emits nothing: records that the next generated line came from
            // this VBR source line (for translating rustc errors back).
            diags.map_line(out.matches('\n').count() + 1, *vbr_line);
        }
        Stmt::Comment(text) => {
            out.push_str(&format!("{}// {}\n", pad, text));
        }
        Stmt::Dim {
            name,
            ty,
            init,
            line,
        } => {
            let var = rust_name(name);
            // `Dim x As T = Rust … End Rust` — the block's value, typed by `As T`.
            if let Some(Expr::InlineRust(raw)) = init {
                let kw = let_kw(mutated.contains(&var));
                out.push_str(&format!(
                    "{}{} {}: {} = {};\n",
                    pad,
                    kw,
                    var,
                    decltype_rust(ty),
                    render_inline_block(raw, indent)
                ));
                return;
            }
            // `Dim x As T = Python … End Python` — run the block via pyo3 and
            // extract its last-line value into `T` (or hold a `PyObject` handle).
            if let Some(Expr::InlinePython { inputs, body }) = init {
                let kw = let_kw(mutated.contains(&var));
                out.push_str(&format!(
                    "{}{} {}: {} = {};\n",
                    pad,
                    kw,
                    var,
                    decltype_rust(ty),
                    render_python_block(inputs, body, Some(ty), indent)
                ));
                return;
            }
            match ty {
                // Collections are `mut` only if mutated; empty unless given an
                // initialiser (e.g. an iterator `.collect()`).
                // Collections start empty unless given an initialiser; `Result`/
                // `Option` always carry one (there's no meaningful empty value).
                DeclType::Vec(_) | DeclType::Map(..) | DeclType::Result(..) | DeclType::Option(_) => {
                    let kw = let_kw(mutated.contains(&var));
                    let empty = match ty {
                        DeclType::Vec(_) => "Vec::new()",
                        DeclType::Map(..) => "HashMap::new()",
                        DeclType::Option(_) => "None",
                        _ => "",
                    };
                    let value = init
                        .as_ref()
                        .map(|e| render_expr(e, None))
                        .unwrap_or_else(|| empty.to_string());
                    out.push_str(&format!(
                        "{}{} {}: {} = {};\n",
                        pad,
                        kw,
                        var,
                        decltype_rust(ty),
                        value
                    ));
                }
                // Fixed arrays are auto-zeroed; the size is the element count.
                DeclType::Array(t, n) => {
                    array_size_note(diags);
                    let kw = let_kw(mutated.contains(&var));
                    let d = array_default(*t);
                    out.push_str(&format!(
                        "{}{} {}: [{}; {}] = [{}; {}];\n",
                        pad, kw, var, t.rust(), n, d, n
                    ));
                }
                DeclType::Array2D(t, r, c) => {
                    array_size_note(diags);
                    let kw = let_kw(mutated.contains(&var));
                    let d = array_default(*t);
                    out.push_str(&format!(
                        "{}{} {}: [[{}; {}]; {}] = [[{}; {}]; {}];\n",
                        pad, kw, var, t.rust(), c, r, d, c, r
                    ));
                }
                // A struct value (always fully initialised at the Dim).
                DeclType::Named(n) => {
                    let kw = let_kw(mutated.contains(&var));
                    let value = init
                        .as_ref()
                        .map(|e| render_expr(e, None))
                        .unwrap_or_default();
                    out.push_str(&format!("{}{} {}: {} = {};\n", pad, kw, var, n, value));
                }
                DeclType::Tuple(_) => {
                    let kw = let_kw(mutated.contains(&var));
                    let value = init
                        .as_ref()
                        .map(|e| render_expr(e, None))
                        .unwrap_or_default();
                    out.push_str(&format!(
                        "{}{} {}: {} = {};\n",
                        pad,
                        kw,
                        var,
                        decltype_rust(ty),
                        value
                    ));
                }
                DeclType::Plain(t) => {
                    let is_mut = mutated.contains(&var);
                    if !t.is_fixed_size() {
                        emit_dim_string(&var, name, is_mut, init.as_ref(), *line, diags, &pad, out);
                    } else {
                        let kw = let_kw(is_mut);
                        match init {
                            Some(e) => {
                                let value = render_expr(e, Some(*t));
                                out.push_str(&format!(
                                    "{}{} {}: {} = {};\n",
                                    pad, kw, var, t.rust(), value
                                ));
                            }
                            None => {
                                out.push_str(&format!("{}{} {}: {};\n", pad, kw, var, t.rust()));
                            }
                        }
                    }
                }
            }
        }
        Stmt::Set {
            name,
            mutable,
            value,
        } => {
            diags.note(
                "set-borrow",
                "`Set` borrows instead of copying — the new name points at the same value, \
                 so no copy is made. `Set Mut` borrows mutably, letting you change the original.",
            );
            let var = rust_name(name);
            let borrow = if *mutable { "&mut " } else { "&" };
            out.push_str(&format!(
                "{}let {} = {}{};\n",
                pad,
                var,
                borrow,
                render_expr(value, None)
            ));
        }
        Stmt::Assign { target, value, op } => {
            let lhs = match target {
                // Assigning through a ByRef parameter writes to the pointee: `*p = …`.
                Expr::Ident(name) => {
                    let var = rust_name(name);
                    if byref.contains(&var) {
                        format!("*{}", var)
                    } else {
                        var
                    }
                }
                other => render_expr(other, None),
            };
            // `+=` / `-=` / `*=` / `/=` for a compound assignment, else plain `=`.
            let assign = match op {
                Some(o) => format!("{}=", op_str(*o)),
                None => "=".to_string(),
            };
            out.push_str(&format!("{}{} {} {};\n", pad, lhs, assign, render_expr(value, None)));
        }
        Stmt::Expr(e) => {
            let rendered = match e {
                Expr::InlineRust(raw) => render_inline_block(raw, indent),
                _ => render_expr(e, None),
            };
            out.push_str(&format!("{}{};\n", pad, rendered));
        }
        Stmt::DestructureDim { names, ty, value } => {
            // `let (a, b) = value;` — each binding is `mut` only if reassigned.
            let pat: Vec<String> = names
                .iter()
                .map(|n| {
                    let v = rust_name(n);
                    if mutated.contains(&v) {
                        format!("mut {}", v)
                    } else {
                        v
                    }
                })
                .collect();
            // A written `As (T, U)` annotates the tuple; it also tells a `Python`
            // block which Rust tuple to extract its several results into.
            let anno = match ty {
                Some(t) => format!(": {}", decltype_rust(t)),
                None => String::new(),
            };
            let val = match value {
                Expr::InlineRust(raw) => render_inline_block(raw, indent),
                Expr::InlinePython { inputs, body } => {
                    render_python_block(inputs, body, ty.as_ref(), indent)
                }
                _ => render_expr(value, None),
            };
            out.push_str(&format!("{}let ({}){} = {};\n", pad, pat.join(", "), anno, val));
        }
        Stmt::HandleDim { name, raw, .. } => {
            // An opaque handle: Rust infers the type (no annotation). We can't see
            // whether later blocks mutate it (`.next()` vs a `&self` call), so we
            // bind it `mut` and allow the case where that mut goes unused.
            let var = rust_name(name);
            out.push_str(&format!(
                "{}#[allow(unused_mut)]\n{}let mut {} = {};\n",
                pad,
                pad,
                var,
                render_inline_block(raw, indent)
            ));
        }
        Stmt::Return(Some(e)) => {
            out.push_str(&format!("{}return {};\n", pad, render_expr(e, None)));
        }
        Stmt::Return(None) => {
            out.push_str(&format!("{}return;\n", pad));
        }
        Stmt::Print(e) => {
            // Print a concatenation as one flat println! (string literals fold
            // into the format string); a lone literal prints directly.
            match e {
                Expr::Binary { op: BinOp::Concat, .. } => {
                    let (fmt, args) = flatten_concat(e);
                    if args.is_empty() {
                        out.push_str(&format!("{}println!(\"{}\");\n", pad, fmt));
                    } else {
                        out.push_str(&format!(
                            "{}println!(\"{}\", {});\n",
                            pad,
                            fmt,
                            args.join(", ")
                        ));
                    }
                }
                Expr::Str(s) => {
                    out.push_str(&format!("{}println!(\"{}\");\n", pad, escape_fmt(s)));
                }
                _ => {
                    out.push_str(&format!(
                        "{}println!(\"{{}}\", {});\n",
                        pad,
                        render_expr(e, None)
                    ));
                }
            }
        }
        Stmt::If {
            branches,
            else_body,
        } => {
            for (i, (cond, body)) in branches.iter().enumerate() {
                let head = if i == 0 { "if" } else { "} else if" };
                out.push_str(&format!("{}{} {} {{\n", pad, head, render_expr(cond, None)));
                emit_block(body, mutated, byref, indent + 1, diags, out);
            }
            if let Some(body) = else_body {
                out.push_str(&format!("{}}} else {{\n", pad));
                emit_block(body, mutated, byref, indent + 1, diags, out);
            }
            out.push_str(&format!("{}}}\n", pad));
        }
        Stmt::For {
            var,
            from,
            to,
            step,
            body,
        } => {
            let loop_var = rust_name(var);
            let range = render_range(from, to, step.as_ref(), diags);
            out.push_str(&format!("{}for {} in {} {{\n", pad, loop_var, range));
            emit_block(body, mutated, byref, indent + 1, diags, out);
            out.push_str(&format!("{}}}\n", pad));
        }
        Stmt::DoLoop { cond, body } => {
            let inner = "    ".repeat(indent + 1);
            match cond {
                None => {
                    out.push_str(&format!("{}loop {{\n", pad));
                    emit_block(body, mutated, byref, indent + 1, diags, out);
                }
                Some(DoCond::PreWhile(c)) => {
                    out.push_str(&format!("{}while {} {{\n", pad, render_expr(c, None)));
                    emit_block(body, mutated, byref, indent + 1, diags, out);
                }
                Some(DoCond::PreUntil(c)) => {
                    out.push_str(&format!("{}while !({}) {{\n", pad, render_expr(c, None)));
                    emit_block(body, mutated, byref, indent + 1, diags, out);
                }
                // Post-test loops run the body once before checking.
                Some(DoCond::PostWhile(c)) => {
                    out.push_str(&format!("{}loop {{\n", pad));
                    emit_block(body, mutated, byref, indent + 1, diags, out);
                    out.push_str(&format!("{}if !({}) {{\n", inner, render_expr(c, None)));
                    out.push_str(&format!("{}    break;\n{}}}\n", inner, inner));
                }
                Some(DoCond::PostUntil(c)) => {
                    out.push_str(&format!("{}loop {{\n", pad));
                    emit_block(body, mutated, byref, indent + 1, diags, out);
                    out.push_str(&format!("{}if {} {{\n", inner, render_expr(c, None)));
                    out.push_str(&format!("{}    break;\n{}}}\n", inner, inner));
                }
            }
            out.push_str(&format!("{}}}\n", pad));
        }
        Stmt::Break => out.push_str(&format!("{}break;\n", pad)),
        Stmt::Continue => out.push_str(&format!("{}continue;\n", pad)),
        Stmt::ForEach {
            var1,
            var2,
            iter,
            body,
        } => {
            // Iterate by shared reference — no copy, the collection stays usable.
            let pattern = match var2 {
                Some(v2) => format!("({}, {})", rust_name(var1), rust_name(v2)),
                None => rust_name(var1),
            };
            out.push_str(&format!(
                "{}for {} in &{} {{\n",
                pad,
                pattern,
                render_expr(iter, None)
            ));
            emit_block(body, mutated, byref, indent + 1, diags, out);
            out.push_str(&format!("{}}}\n", pad));
        }
        Stmt::Match {
            scrutinee,
            arms,
            line: _,
        } => {
            let arm_pad = "    ".repeat(indent + 1);
            out.push_str(&format!("{}match {} {{\n", pad, render_expr(scrutinee, None)));
            for arm in arms {
                let guard = match &arm.guard {
                    Some(g) => format!(" if {}", render_expr(g, None)),
                    None => String::new(),
                };
                out.push_str(&format!("{}{}{} => {{\n", arm_pad, arm.pattern, guard));
                emit_block(&arm.body, mutated, byref, indent + 2, diags, out);
                out.push_str(&format!("{}}}\n", arm_pad));
            }
            out.push_str(&format!("{}}}\n", pad));
        }
        Stmt::Draw(cmd) => {
            out.push_str(&format!("{}{}\n", pad, render_draw_cmd(cmd, diags)));
        }
    }
}

/// A named canvas colour → its 8-bit RGB. A small, memorable palette; anything
/// beyond it uses the explicit `Color(r, g, b)` form.
fn named_color(name: &str) -> Option<(u8, u8, u8)> {
    let c = match name.to_ascii_lowercase().as_str() {
        "black" => (0, 0, 0),
        "white" => (255, 255, 255),
        "red" => (255, 0, 0),
        "green" => (0, 128, 0),
        "blue" => (0, 0, 255),
        "gray" | "grey" => (128, 128, 128),
        "yellow" => (255, 255, 0),
        "orange" => (255, 165, 0),
        "purple" => (128, 0, 128),
        "navy" => (0, 0, 128),
        "cyan" => (0, 255, 255),
        "magenta" => (255, 0, 255),
        _ => return None,
    };
    Some(c)
}

/// A canvas coordinate/dimension expression, cast to the `f32` Iced draws with.
fn coord(e: &Expr) -> String {
    format!("({}) as f32", render_expr(e, None))
}

/// A colour argument → an `iced::Color`: `Color.Red` (palette) or `Color(r,g,b)`.
fn render_color(e: &Expr, diags: &mut Diagnostics) -> String {
    match e {
        Expr::Field(recv, name) if matches!(&**recv, Expr::Ident(n) if n.eq_ignore_ascii_case("Color")) => {
            match named_color(name) {
                Some((r, g, b)) => format!("iced::Color::from_rgb8({}, {}, {})", r, g, b),
                None => {
                    diags.error_once(
                        "unknown-color",
                        format!(
                            "Unknown colour `Color.{}`. Named colours: Black, White, Red, Green, \
                             Blue, Gray, Yellow, Orange, Purple, Navy, Cyan, Magenta — or use \
                             `Color(r, g, b)`.",
                            name
                        ),
                    );
                    "iced::Color::BLACK".to_string()
                }
            }
        }
        Expr::Call { name, args } if name.eq_ignore_ascii_case("Color") && args.len() == 3 => format!(
            "iced::Color::from_rgb8(({}) as u8, ({}) as u8, ({}) as u8)",
            render_expr(&args[0], None),
            render_expr(&args[1], None),
            render_expr(&args[2], None)
        ),
        _ => {
            diags.error_once(
                "bad-color",
                "Expected a colour: a named `Color.Red` or an explicit `Color(r, g, b)`.",
            );
            "iced::Color::BLACK".to_string()
        }
    }
}

/// A drawable shape → the Iced `Path` that draws it.
fn render_path(shape: &Shape) -> String {
    match shape {
        Shape::Circle(cx, cy, r) => format!(
            "iced::widget::canvas::Path::circle(iced::Point::new({}, {}), {})",
            coord(cx), coord(cy), coord(r)
        ),
        Shape::Rect(x, y, w, h) => format!(
            "iced::widget::canvas::Path::rectangle(iced::Point::new({}, {}), iced::Size::new({}, {}))",
            coord(x), coord(y), coord(w), coord(h)
        ),
        Shape::Line(x1, y1, x2, y2) => format!(
            "iced::widget::canvas::Path::line(iced::Point::new({}, {}), iced::Point::new({}, {}))",
            coord(x1), coord(y1), coord(x2), coord(y2)
        ),
    }
}

/// A drawing verb → the Rust statement that applies it to the ambient `frame`
/// (a `&mut Frame`, so fills/strokes and nested paint-function calls both work).
pub(crate) fn render_draw_cmd(cmd: &DrawCmd, diags: &mut Diagnostics) -> String {
    match cmd {
        DrawCmd::Fill { shape, color } => {
            if matches!(shape, Shape::Line(..)) {
                diags.error_once(
                    "fill-line",
                    "A Line has no area to fill — draw it with `Stroke Line(...)` instead.",
                );
            }
            format!("frame.fill(&{}, {});", render_path(shape), render_color(color, diags))
        }
        DrawCmd::Stroke { shape, color, width } => {
            let w = width.as_ref().map(coord).unwrap_or_else(|| "1.0".to_string());
            format!(
                "frame.stroke(&{}, iced::widget::canvas::Stroke::default().with_color({}).with_width({}));",
                render_path(shape),
                render_color(color, diags),
                w
            )
        }
        DrawCmd::Text { text, x, y, color } => {
            let col = match color {
                Some(c) => render_color(c, diags),
                None => "iced::Color::BLACK".to_string(),
            };
            format!(
                "frame.fill_text(iced::widget::canvas::Text {{ content: format!(\"{{}}\", {}), \
                 position: iced::Point::new({}, {}), color: {}, ..Default::default() }});",
                render_expr(text, None),
                coord(x),
                coord(y),
                col
            )
        }
        DrawCmd::Paint { name, args } => {
            let mut a = vec!["frame".to_string()];
            a.extend(args.iter().map(|e| render_expr(e, None)));
            format!("{}({});", rust_name(name), a.join(", "))
        }
    }
}

/// Emit a `Dim` of an unknown-size `String`, where ownership rules bite.
fn emit_dim_string(
    var: &str,
    orig_name: &str,
    is_mut: bool,
    init: Option<&Expr>,
    line: usize,
    diags: &mut Diagnostics,
    pad: &str,
    out: &mut String,
) {
    match init {
        None => {
            out.push_str(&format!("{}{} {}: String;\n", pad, let_kw(is_mut), var));
        }
        // Every String variable is an owned String — uniform and predictable.
        Some(Expr::Str(s)) => {
            out.push_str(&format!(
                "{}{} {}: String = \"{}\".to_string();\n",
                pad,
                let_kw(is_mut),
                var,
                escape(s)
            ));
        }
        // Assigning one String variable to another would move/copy something of
        // unknown size. Rust won't do that silently — explain the explicit forms.
        Some(Expr::Ident(rhs)) => {
            diags.error(line, unknown_size_message(orig_name, rhs));
        }
        // Anything else (concat → format!, `.clone()`, …) is a freshly owned String.
        Some(other) => {
            if is_clone(other) {
                diags.note(
                    "clone-cost",
                    "`.clone()` makes a full copy. That's fine when you genuinely need a \
                     second copy, but it has a real cost — reach for `Set` to borrow when \
                     you only need to look at the value.",
                );
            }
            out.push_str(&format!(
                "{}{} {}: String = {};\n",
                pad,
                let_kw(is_mut),
                var,
                render_expr(other, None)
            ));
        }
    }
}

/// Walk the body firing the one-time `⚠`/`ℹ` notes for builtins that behave
/// differently than VB programmers expect (indexing, Option/Result returns).
pub(crate) fn note_builtins(stmts: &[Stmt], diags: &mut Diagnostics) {
    for stmt in stmts {
        match stmt {
            Stmt::Dim { init: Some(e), .. } => note_builtins_expr(e, diags),
            Stmt::Set { value, .. } | Stmt::Assign { value, .. } => note_builtins_expr(value, diags),
            Stmt::Return(Some(e)) | Stmt::Print(e) | Stmt::Expr(e) => {
                note_builtins_expr(e, diags)
            }
            Stmt::If {
                branches,
                else_body,
            } => {
                for (cond, body) in branches {
                    note_builtins_expr(cond, diags);
                    note_builtins(body, diags);
                }
                if let Some(body) = else_body {
                    note_builtins(body, diags);
                }
            }
            Stmt::For {
                from, to, step, body, ..
            } => {
                note_builtins_expr(from, diags);
                note_builtins_expr(to, diags);
                if let Some(s) = step {
                    note_builtins_expr(s, diags);
                }
                note_builtins(body, diags);
            }
            Stmt::ForEach { iter, body, .. } => {
                note_builtins_expr(iter, diags);
                note_builtins(body, diags);
            }
            Stmt::DoLoop { cond, body } => {
                match cond {
                    Some(
                        DoCond::PreWhile(c)
                        | DoCond::PreUntil(c)
                        | DoCond::PostWhile(c)
                        | DoCond::PostUntil(c),
                    ) => note_builtins_expr(c, diags),
                    None => {}
                }
                note_builtins(body, diags);
            }
            Stmt::Match { scrutinee, arms, .. } => {
                note_builtins_expr(scrutinee, diags);
                for arm in arms {
                    if let Some(g) = &arm.guard {
                        note_builtins_expr(g, diags);
                    }
                    note_builtins(&arm.body, diags);
                }
            }
            _ => {}
        }
    }
}

fn note_builtins_expr(e: &Expr, diags: &mut Diagnostics) {
    match e {
        Expr::Binary { lhs, rhs, .. } => {
            note_builtins_expr(lhs, diags);
            note_builtins_expr(rhs, diags);
        }
        Expr::MethodCall { recv, method, args } => {
            if let Expr::Ident(name) = &**recv {
                if let Some(canon) = stdlib_type(name) {
                    diags.mark(&format!("stdlib:{}", canon));
                    diags.note(
                        "stdlib-call",
                        "The standard library is a Rust crate — `FileSystem.Read(...)` becomes \
                         `FileSystem::read(...)` and `use vbr_stdlib::…;` is added for you.",
                    );
                }
            }
            if method.eq_ignore_ascii_case("unwrap") {
                diags.warn_once_global(
                    "unwrap-training-wheels",
                    ".unwrap() works, but it's training wheels — it crashes the program if the \
                     value is an error or None. Prefer the `?` operator to propagate, or \
                     `Match` over Ok/Err (Some/None) to handle both outcomes.",
                );
            }
            if method.eq_ignore_ascii_case("insert") {
                diags.note(
                    "hashmap-insert-tostring",
                    "HashMap keys are owned Strings — VBR adds `.to_string()` to a string-literal \
                     key for you, so `dict.insert(\"key\", v)` becomes `dict.insert(\"key\".to_string(), v)`.",
                );
            }
            note_builtins_expr(recv, diags);
            for a in args {
                note_builtins_expr(a, diags);
            }
        }
        Expr::Try(inner) | Expr::Field(inner, _) | Expr::Closure { body: inner, .. } => {
            note_builtins_expr(inner, diags)
        }
        Expr::Index(inner, idx) => {
            diags.note(
                "index-bounds",
                "Indexing with `x[i]` panics if `i` is out of bounds. When you're not sure \
                 the index is valid, use `x.get(i)` — it returns an Option you can handle.",
            );
            note_builtins_expr(inner, diags);
            note_builtins_expr(idx, diags);
        }
        Expr::StructLit { fields, .. } => {
            for (_, v) in fields {
                note_builtins_expr(v, diags);
            }
        }
        Expr::Call { name, args } => {
            match name.to_ascii_lowercase().as_str() {
                "mid" => diags.note(
                    "builtin-mid",
                    "Mid is 1-indexed in VB; VBR shifts the position for you and counts by \
                     characters (not bytes), so it stays correct on any text — Mid(s, 2, 3) \
                     is s.chars().skip(1).take(3).",
                ),
                "instr" => diags.note(
                    "builtin-instr",
                    "InStr becomes Rust's .find(), which returns an Option: Some(pos) when \
                     found, None when not. You handle both instead of checking for 0.",
                ),
                "val" => diags.note(
                    "builtin-val",
                    "Val becomes Rust's .parse(), which returns a Result: parsing can fail, \
                     so you handle the error rather than getting a silent 0.",
                ),
                "inputbox" => {
                    diags.mark("input_box");
                    diags.note(
                        "builtin-inputbox",
                        "InputBox has no window in a terminal app — VBR prints the prompt and \
                         reads a line from the keyboard, returning it as a String.",
                    );
                }
                "rnd" => diags.error_once(
                    "builtin-rnd",
                    "Rnd() is not built in — Rust keeps randomness in the `rand` crate so it \
                     stays explicit. Add it with `Use rand 0.8`, then:\n\n    \
                     use rand::Rng;\n    \
                     let x: f64 = rand::thread_rng().gen_range(0.0..1.0);",
                ),
                "format" => diags.error_once(
                    "builtin-format",
                    "Format(value, \"pattern\") is not supported. For a fixed number of \
                     decimals use a format specifier — `Debug.Print Str(x)` or directly \
                     `format!(\"{:.2}\", x)`. For grouped thousands (\"#,###\"), add the \
                     num-format crate (`Use num-format`).",
                ),
                _ => {}
            }
            for a in args {
                note_builtins_expr(a, diags);
            }
        }
        _ => {}
    }
}

pub(crate) fn is_mutating_method(m: &str) -> bool {
    matches!(
        m,
        "push" | "insert" | "remove" | "pop" | "clear" | "sort" | "reverse" | "extend"
            | "retain" | "resize" | "truncate" | "append" | "dedup"
            // String mutators.
            | "push_str" | "insert_str" | "make_ascii_uppercase" | "make_ascii_lowercase"
    )
}

/// Mark the receiver variable of any mutating method call (`v.push(…)`).
fn mark_mutating_calls(e: &Expr, set: &mut HashSet<String>) {
    match e {
        Expr::MethodCall { recv, method, args } => {
            if is_mutating_method(&rust_name(method)) {
                if let Expr::Ident(v) = &**recv {
                    set.insert(rust_name(v));
                }
            }
            mark_mutating_calls(recv, set);
            for a in args {
                mark_mutating_calls(a, set);
            }
        }
        Expr::Call { args, .. } => {
            for a in args {
                mark_mutating_calls(a, set);
            }
        }
        Expr::Binary { lhs, rhs, .. } | Expr::Index(lhs, rhs) => {
            mark_mutating_calls(lhs, set);
            mark_mutating_calls(rhs, set);
        }
        Expr::Field(inner, _)
        | Expr::Try(inner)
        | Expr::Cast(inner, _)
        | Expr::Deref(inner)
        | Expr::MutRef(inner)
        | Expr::Ref(inner)
        | Expr::Closure { body: inner, .. } => mark_mutating_calls(inner, set),
        Expr::StructLit { fields, .. } => {
            for (_, v) in fields {
                mark_mutating_calls(v, set);
            }
        }
        _ => {}
    }
}

/// The root variable of an assignable place, e.g. `alice` in `alice.age`,
/// or `grid` in `grid[r][c]`. `None` when there's no plain local at the root.
fn lvalue_root(target: &Expr) -> Option<String> {
    match target {
        Expr::Ident(name) => Some(rust_name(name)),
        Expr::Field(inner, _) | Expr::Index(inner, _) => lvalue_root(inner),
        _ => None,
    }
}

/// The default element for a fixed array of `t` (it must be a Copy type).
fn array_default(t: Type) -> &'static str {
    match t {
        Type::Single | Type::Double => "0.0",
        Type::Boolean => "false",
        _ => "0",
    }
}

fn array_size_note(diags: &mut Diagnostics) {
    diags.warn_once_global(
        "array-size",
        "A fixed array's size is the element COUNT, not an upper bound — `Dim x(10)` is \
         10 elements with indexes 0..9 (VB6 gave you 11). For a growable list use a Vec.",
    );
}

/// Render a `Rust … End Rust` body as a Rust block expression `{ … }`, dedented
/// and re-indented under `indent`. A one-line body stays inline (`{ a + b }`).
fn render_inline_block(raw: &str, indent: usize) -> String {
    let body = dedent(raw);
    let lines: Vec<&str> = body
        .lines()
        .skip_while(|l| l.trim().is_empty())
        .collect();
    // Drop trailing blank lines.
    let end = lines.iter().rposition(|l| !l.trim().is_empty()).map_or(0, |p| p + 1);
    let lines = &lines[..end];

    if lines.len() <= 1 {
        return format!("{{ {} }}", lines.first().map_or("", |l| l.trim()));
    }
    let inner = "    ".repeat(indent + 1);
    let close = "    ".repeat(indent);
    let mut s = String::from("{\n");
    for l in lines {
        if l.trim().is_empty() {
            s.push('\n');
        } else {
            s.push_str(&format!("{}{}\n", inner, l));
        }
    }
    s.push_str(&format!("{}}}", close));
    s
}

/// Render a `Python … End Python` block as a Rust block expression that runs the
/// body through pyo3. Unlike inline Rust (spliced tokens), this executes real
/// CPython: the body runs in a fresh namespace, the last line's value is captured
/// in `_vbr_result`, and it is either `.extract()`ed into the annotated Rust type
/// or `.unbind()`ed into an opaque `PyObject` handle. `inputs` are VBR variables
/// injected into the namespace first (scalars convert; a handle is re-borrowed).
/// `ty` is the target type (`None` → context inference, non-`Dim` positions only).
fn render_python_block(inputs: &[String], raw: &str, ty: Option<&DeclType>, indent: usize) -> String {
    let body = prepare_python(raw);
    let is_handle = matches!(ty, Some(DeclType::Named(n)) if n == "PyObject");
    let ret = match ty {
        Some(t) => format!(" -> pyo3::PyResult<{}>", decltype_rust(t)),
        None => String::new(),
    };
    let pad = "    ".repeat(indent + 1);
    let close = "    ".repeat(indent);
    // Inject each VBR input under the name it was written as; `&var` works for
    // scalars (converted) and `&Py<PyAny>` handles (re-borrowed) alike.
    let mut sets = String::new();
    for name in inputs {
        sets.push_str(&format!(
            "{p}    ns.set_item(\"{key}\", &{var})?;\n",
            p = pad,
            key = name,
            var = rust_name(name),
        ));
    }
    // A handle unbinds the value (GIL-independent); a scalar extracts it.
    let tail = if is_handle {
        format!(
            "{p}    Ok(ns.get_item(\"_vbr_result\")?\n\
             {p}        .expect(\"the Python block produced no value on its last line\")\n\
             {p}        .unbind())\n",
            p = pad,
        )
    } else {
        format!(
            "{p}    ns.get_item(\"_vbr_result\")?\n\
             {p}        .expect(\"the Python block produced no value on its last line\")\n\
             {p}        .extract()\n",
            p = pad,
        )
    };
    // The Python source is embedded as a raw string; its lines stay at column 0
    // (Python is whitespace-sensitive) regardless of the surrounding Rust indent.
    format!(
        "{{\n\
         {p}use pyo3::prelude::*;\n\
         {p}pyo3::Python::with_gil(|py|{ret} {{\n\
         {p}    let ns = pyo3::types::PyDict::new(py);\n\
         {sets}\
         {p}    py.run(&std::ffi::CString::new(r#\"\n{body}\n\"#).unwrap(), Some(&ns), Some(&ns))?;\n\
         {tail}\
         {p}}})\n\
         {p}.expect(\"the Python block raised an exception\")\n\
         {close}}}",
        p = pad,
        ret = ret,
        sets = sets,
        body = body,
        tail = tail,
        close = close,
    )
}

/// Prepare a raw Python body for `exec`: dedent it (Python cares about the leading
/// whitespace the VBR editor added), trim blank edges, and bind the last non-blank
/// line to `_vbr_result` so its value can be read back out. The last line must be
/// an expression (the `Rust`-block "last line is the value" rule carries over).
fn prepare_python(raw: &str) -> String {
    let body = dedent(raw);
    let mut lines: Vec<String> = body.lines().map(|l| l.to_string()).collect();
    while lines.first().is_some_and(|l| l.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    if let Some(idx) = lines.iter().rposition(|l| !l.trim().is_empty()) {
        lines[idx] = format!("_vbr_result = {}", lines[idx].trim_start());
    }
    lines.join("\n")
}

/// Strip the common leading whitespace from every non-blank line.
fn dedent(s: &str) -> String {
    let min = s
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    s.lines()
        .map(|l| if l.len() >= min { &l[min..] } else { l })
        .collect::<Vec<_>>()
        .join("\n")
}

fn let_kw(is_mut: bool) -> &'static str {
    if is_mut {
        "let mut"
    } else {
        "let"
    }
}

fn is_clone(e: &Expr) -> bool {
    matches!(e, Expr::MethodCall { method, .. } if method == "clone")
}

/// The teaching block from spec_01, with the user's own variable names filled in.
fn unknown_size_message(target: &str, source: &str) -> String {
    format!(
        "Cannot assign '{src}' to '{dst}' directly.\n\n  \
         Unlike integers or doubles, String is not a fixed size —\n  \
         it can grow to any length. Rust won't silently copy something\n  \
         of unknown size. You need to be explicit:\n\n  \
         Set {dst} = {src}                    ' borrow — {dst} looks at {src}, no copy made\n  \
         Set Mut {dst} = {src}                ' mutable borrow — {dst} can modify {src}\n  \
         Dim {dst} As String = {src}.clone()  ' explicit copy — you are asking\n                                ' \
         for a copy knowing it has a cost\n\n  \
         The same rule applies to any type that can grow to an unknown\n  \
         size. Fixed size types like Long, Double and Boolean copy\n  \
         freely because Rust knows exactly how big they are.",
        src = source,
        dst = target
    )
}

/// Build the Rust range for a `For` loop, including `Step`.
fn render_range(from: &Expr, to: &Expr, step: Option<&Expr>, diags: &mut Diagnostics) -> String {
    let lo = render_expr(from, None);
    let hi = render_expr(to, None);
    match step {
        None => format!("{}..={}", lo, hi),
        Some(Expr::Int(n)) if *n < 0 => {
            // Counting down: Rust ranges only go up, so reverse.
            diags.note(
                "for-step-negative",
                "ℹ A counting-down `For ... Step -n` becomes `(lo..=hi).rev()` in Rust — \
                 Rust ranges always run low-to-high, so we reverse a normal one.",
            );
            let abs = (-n) as i64;
            if abs == 1 {
                format!("({}..={}).rev()", hi, lo)
            } else {
                format!("({}..={}).rev().step_by({})", hi, lo, abs)
            }
        }
        Some(Expr::Int(n)) => format!("({}..={}).step_by({})", lo, hi, n),
        Some(other) => {
            // Non-literal step: fall back to a literal-rendered step_by.
            format!("({}..={}).step_by({})", lo, hi, render_expr(other, None))
        }
    }
}

/// Render an expression. `expected` lets a `Double` context coerce integer
/// literals to floats (`5` -> `5.0`), which Rust requires.
pub(crate) fn render_expr(e: &Expr, expected: Option<Type>) -> String {
    render_prec(e, expected, 0, false)
}

/// Precedence-aware rendering: parens are emitted only where Rust needs them,
/// so the output reads like hand-written Rust (and rustc stays quiet).
/// `parent_prec` is the binding power of the enclosing operator; `is_right`
/// marks the right operand of a left-associative parent (so `a - (b - c)`
/// keeps its parens).
fn render_prec(e: &Expr, expected: Option<Type>, parent_prec: u8, is_right: bool) -> String {
    match e {
        Expr::Int(n) => {
            // An integer literal assigned into a float context needs a `.0`.
            if expected.map_or(false, |t| t.is_float()) {
                format!("{}.0", n)
            } else {
                n.to_string()
            }
        }
        Expr::Float(f) => fmt_float(*f),
        Expr::Bool(b) => b.to_string(),
        Expr::Str(s) => format!("\"{}\"", escape(s)),
        // `None` is the Option constructor, not a variable — keep it as-is.
        Expr::Ident(name) if name == "None" => "None".to_string(),
        // `Me` is the method receiver.
        Expr::Ident(name) if name == "Me" => "self".to_string(),
        Expr::Ident(name) => rust_name(name),
        Expr::Binary { op, .. } if *op == BinOp::Concat => {
            // `&` concatenation becomes one flat format!, sidestepping ownership:
            // the whole chain is collected in order and string literals fold into
            // the format string itself, so `"a: " & x & "!"` reads as
            // `format!("a: {}!", x)`. The call is atomic — never needs parens.
            let (fmt, args) = flatten_concat(e);
            if args.is_empty() {
                format!("format!(\"{}\")", fmt)
            } else {
                format!("format!(\"{}\", {})", fmt, args.join(", "))
            }
        }
        Expr::Binary { op, lhs, rhs } if *op == BinOp::Xor => {
            // VBR treats Xor as a loose logical op, but Rust's `^` binds *tighter*
            // than comparison/`&&`/`||`. So parenthesise any binary operand to keep
            // our grouping, and wrap the whole node when it sits under a tighter op.
            let operand = |e: &Expr| {
                let s = render_prec(e, None, 0, false);
                if matches!(e, Expr::Binary { .. }) {
                    format!("({})", s)
                } else {
                    s
                }
            };
            let inner = format!("{} ^ {}", operand(lhs), operand(rhs));
            let p = prec(BinOp::Xor);
            if p < parent_prec || (p == parent_prec && is_right) {
                format!("({})", inner)
            } else {
                inner
            }
        }
        Expr::Binary { op, lhs, rhs } if *op == BinOp::Pow => {
            // `^` lowers to powi (integer exponent) or powf (float exponent),
            // assuming a floating-point base as the spec shows.
            let base = render_math_recv(lhs);
            match rhs.as_ref() {
                Expr::Int(n) => format!("{}.powi({})", base, n),
                Expr::Float(f) => format!("{}.powf({})", base, fmt_float(*f)),
                other => format!("{}.powf({})", base, render_expr(other, None)),
            }
        }
        Expr::Binary { op, lhs, rhs } => {
            let p = prec(*op);
            // Arithmetic propagates the Double context; comparisons don't.
            let child = if is_arithmetic(*op) { expected } else { None };
            let inner = format!(
                "{} {} {}",
                render_prec(lhs, child, p, false),
                op_str(*op),
                render_prec(rhs, child, p, true)
            );
            if p < parent_prec || (p == parent_prec && is_right) {
                format!("({})", inner)
            } else {
                inner
            }
        }
        Expr::MethodCall { recv, method, args } => {
            // DataFrame `Select` (tagged by the resolver) renders its column names
            // as a slice: `df.Select("a", "b")` → `df.select(&["a", "b"])`.
            if method == "__df_select" {
                let cols: Vec<String> = args.iter().map(|a| render_expr(a, None)).collect();
                return format!("{}.select(&[{}])", render_recv(recv), cols.join(", "));
            }
            // DataFrame `GroupBy` renders its key names as a slice, and `Agg`
            // its (already-lowered) aggregation expressions likewise:
            // `df.group_by(&["band"]).agg(&[col("age").mean()])`.
            if method == "__df_group_by" {
                let keys: Vec<String> = args.iter().map(|a| render_expr(a, None)).collect();
                return format!("{}.group_by(&[{}])", render_recv(recv), keys.join(", "));
            }
            if method == "__df_agg" {
                let aggs: Vec<String> = args.iter().map(|a| render_expr(a, None)).collect();
                return format!("{}.agg(&[{}])", render_recv(recv), aggs.join(", "));
            }
            // Joins: the other frame first (already borrowed), then the key
            // names as a slice: `people.join(&orders, &["name"])`.
            if let Some(real) = method
                .strip_prefix("__df_")
                .filter(|r| matches!(*r, "join" | "left_join" | "outer_join"))
            {
                let other = args.first().map(|a| render_expr(a, None)).unwrap_or_default();
                let keys: Vec<String> =
                    args.iter().skip(1).map(|a| render_expr(a, None)).collect();
                return format!(
                    "{}.{}({}, &[{}])",
                    render_recv(recv),
                    real,
                    other,
                    keys.join(", ")
                );
            }
            let m = rust_name(method);
            // Stdlib namespace call: `FileSystem.Read(x)` → `FileSystem::read(x)`.
            if let Expr::Ident(name) = &**recv {
                if let Some(canon) = stdlib_type(name) {
                    let rendered: Vec<String> = args.iter().map(|a| render_expr(a, None)).collect();
                    return format!("{}::{}({})", canon, m, rendered.join(", "));
                }
            }
            // Integer `^` lowered to `.pow(...)` (the resolver rewrites it here):
            // Rust's integer `pow` takes a `u32` exponent.
            if m == "pow" {
                let arg = args.first().map_or_else(String::new, |a| render_expr(a, None));
                return format!("{}.pow(({}) as u32)", render_recv(recv), arg);
            }
            let rendered: Vec<String> = args
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    // A string literal into a `Vec<String>`/`HashMap` becomes owned:
                    // `push("x")` / `insert("k", …)`.
                    if m == "push" || (m == "insert" && i == 0) {
                        if let Expr::Str(s) = a {
                            return format!("\"{}\".to_string()", escape(s));
                        }
                    }
                    render_expr(a, None)
                })
                .collect();
            // render_recv parenthesises a leading `*`/`-` receiver, e.g.
            // `(*tag).as_string()`. Method names follow Rust convention.
            format!("{}.{}({})", render_recv(recv), m, rendered.join(", "))
        }
        Expr::Closure { params, body, by_ref_params } => {
            render_closure(params, body, *by_ref_params)
        }
        Expr::Tuple(elems) => {
            let parts: Vec<String> = elems.iter().map(|e| render_expr(e, None)).collect();
            format!("({})", parts.join(", "))
        }
        Expr::TupleIndex(inner, n) => format!("{}.{}", render_recv(inner), n),
        Expr::Index(inner, idx) => {
            // A Rust index must be `usize`; a literal is fine, anything else is cast.
            let i = match idx.as_ref() {
                Expr::Int(n) => n.to_string(),
                other => format!("({}) as usize", render_expr(other, None)),
            };
            format!("{}[{}]", render_prec(inner, None, 9, false), i)
        }
        // Fallback for inline Rust in an embedded position (statement positions
        // are rendered with proper indentation by the emitter).
        Expr::InlineRust(raw) => render_inline_block(raw, 0),
        // Inline Python is supported as a typed/handle `Dim` initialiser, which the
        // emitter handles with the target type in hand. In any other position we
        // have no type to extract into, so fall back to context inference.
        Expr::InlinePython { inputs, body } => render_python_block(inputs, body, None, 0),
        // `Not e` → `!e`. Unary `!` binds tighter than any binary op, so it never
        // needs outer parens; the operand is parenthesised if it's itself binary.
        Expr::Not(inner) => format!("!{}", render_prec(inner, None, 9, false)),
        // `Await` is consumed by the GUI codegen (event splitting); if one reaches
        // here it's a misuse — render the inner call so output is still valid Rust.
        Expr::Await(inner) => render_prec(inner, expected, parent_prec, is_right),
        Expr::Call { name, args } => {
            if name.contains("::") {
                // Already a qualified path (a cross-module call) — render verbatim.
                let rendered: Vec<String> = args.iter().map(|a| render_expr(a, None)).collect();
                format!("{}({})", name, rendered.join(", "))
            } else if let Some(s) = lower_constructor(name, args) {
                // Ok/Err/Some result/option constructors.
                s
            } else if let Some(s) = lower_builtin(name, args) {
                // Known string/maths builtins lower to idiomatic Rust.
                s
            } else {
                // An ordinary call to a user function.
                let rendered: Vec<String> = args.iter().map(|a| render_expr(a, None)).collect();
                format!("{}({})", rust_name(name), rendered.join(", "))
            }
        }
        Expr::Try(inner) => format!("{}?", render_prec(inner, None, 9, false)),
        Expr::Field(inner, field) => format!("{}.{}", render_recv(inner), rust_name(field)),
        // Already the verbatim SCREAMING_SNAKE name from the resolver.
        Expr::ConstRef(name) => name.clone(),
        Expr::StructLit { name, fields } => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(fname, fval)| {
                    // A string-literal field becomes an owned String (fields own).
                    let value = match fval {
                        Expr::Str(s) => format!("\"{}\".to_string()", escape(s)),
                        other => render_expr(other, None),
                    };
                    format!("{}: {}", rust_name(fname), value)
                })
                .collect();
            if parts.is_empty() {
                format!("{} {{}}", name)
            } else {
                format!("{} {{ {} }}", name, parts.join(", "))
            }
        }
        Expr::Deref(inner) => format!("*{}", render_prec(inner, expected, 9, false)),
        Expr::MutRef(inner) => format!("&mut {}", render_prec(inner, None, 9, false)),
        Expr::Ref(inner) => format!("&{}", render_prec(inner, None, 9, false)),
        Expr::Cast(inner, ty) => {
            // `x as f64`. Parenthesise the cast if it sits under a tighter op.
            let inner = render_prec(inner, None, 9, false);
            let cast = format!("{} as {}", inner, ty.rust());
            if parent_prec > 0 {
                format!("({})", cast)
            } else {
                cast
            }
        }
    }
}

/// Render a closure. Iterator chains are built by the resolver (which knows
/// element types); `by_ref` emits the `|&x|` destructuring pattern the
/// resolver chose for `filter`/`find` over Copy elements.
fn render_closure(params: &[String], body: &Expr, by_ref: bool) -> String {
    let prefix = if by_ref { "&" } else { "" };
    let ps: Vec<String> = params
        .iter()
        .map(|p| format!("{}{}", prefix, rust_name(p)))
        .collect();
    format!("|{}| {}", ps.join(", "), render_expr(body, None))
}

/// Lower the Result/Option constructors. The `&str`→`String` coercion for a
/// `String` error type is done by the resolver (E-aware), so `Err` renders its
/// payload as-is here.
fn lower_constructor(name: &str, args: &[Expr]) -> Option<String> {
    match (name, args.len()) {
        ("Ok", 1) => Some(format!("Ok({})", render_expr(&args[0], None))),
        ("Some", 1) => Some(format!("Some({})", render_expr(&args[0], None))),
        ("Err", 1) => Some(format!("Err({})", render_expr(&args[0], None))),
        _ => None,
    }
}

/// Lower a VB string or maths builtin to Rust, or return `None` if it isn't one
/// (or the argument count doesn't match — then it's treated as a normal call).
fn lower_builtin(name: &str, args: &[Expr]) -> Option<String> {
    let r = |i: usize| render_expr(&args[i], None);
    match (name.to_ascii_lowercase().as_str(), args.len()) {
        // --- strings ---
        ("len", 1) => Some(method0(&args[0], "len")),
        ("ucase", 1) => Some(method0(&args[0], "to_uppercase")),
        ("lcase", 1) => Some(method0(&args[0], "to_lowercase")),
        ("trim", 1) => Some(method0(&args[0], "trim")),
        // Left/Right count **characters**, as VB does — never bytes. A byte slice
        // (`&s[..n]`) panics the moment a string holds a multi-byte char ("café"),
        // so iterate over `chars()` instead: correct for any Unicode, and safe.
        ("left", 2) => Some(format!(
            "{}.chars().take({}).collect::<String>()",
            render_recv(&args[0]),
            as_usize_arg(&args[1])
        )),
        ("right", 2) => {
            let s = render_recv(&args[0]);
            Some(format!(
                "{0}.chars().skip({0}.chars().count().saturating_sub({1})).collect::<String>()",
                s,
                as_usize_arg(&args[1])
            ))
        }
        ("replace", 3) => Some(format!("{}.replace({}, {})", r(0), r(1), r(2))),
        ("str", 1) => Some(method0(&args[0], "to_string")),
        // InStr → .find() (returns Option); Val → .parse() (returns Result).
        ("instr", 2) => Some(format!("{}.find({})", r(0), r(1))),
        ("val", 1) => Some(format!("{}.parse::<f64>()", r(0))),
        // InputBox → a generated helper that prompts and reads a line.
        ("inputbox", 1) => Some(format!("input_box({})", r(0))),
        // Mid is 1-indexed in VB; Rust slices are 0-indexed, so shift by one.
        ("mid", 3) => Some(render_mid(&args[0], &args[1], Some(&args[2]))),
        ("mid", 2) => Some(render_mid(&args[0], &args[1], None)),
        // --- maths (assume a floating-point argument) ---
        ("sqr", 1) => Some(math0(&args[0], "sqrt")),
        ("abs", 1) => Some(math0(&args[0], "abs")),
        ("int", 1) => Some(math0(&args[0], "floor")),
        ("round", 1) => Some(math0(&args[0], "round")),
        ("sin", 1) => Some(math0(&args[0], "sin")),
        ("cos", 1) => Some(math0(&args[0], "cos")),
        ("tan", 1) => Some(math0(&args[0], "tan")),
        ("log", 1) => Some(math0(&args[0], "ln")),
        ("exp", 1) => Some(math0(&args[0], "exp")),
        _ => None,
    }
}

/// `recv.method()` for a string builtin: parenthesise the receiver if needed.
fn method0(recv: &Expr, method: &str) -> String {
    format!("{}.{}()", render_recv(recv), method)
}

/// `recv.method()` for a maths builtin: a bare numeric literal receiver is
/// ambiguous between f32/f64, so tag it `f64` (`3.7.floor()` won't compile,
/// `3.7f64.floor()` will). Variables keep their declared type.
fn math0(recv: &Expr, method: &str) -> String {
    format!("{}.{}()", render_math_recv(recv), method)
}

fn render_recv(e: &Expr) -> String {
    let s = render_prec(e, None, 9, false);
    // Parenthesise a leading unary so `(-5.0).abs()` / `(*p).foo()` / a borrowed
    // slice `(&s[..]).to_string()` parse right.
    if s.starts_with('-') || s.starts_with('*') || s.starts_with('&') {
        format!("({})", s)
    } else {
        s
    }
}

fn render_math_recv(e: &Expr) -> String {
    match e {
        Expr::Int(n) => suffix_f64(n.to_string()),
        Expr::Float(f) => suffix_f64(fmt_float(*f)),
        _ => render_recv(e),
    }
}

fn suffix_f64(literal: String) -> String {
    let typed = format!("{}f64", literal);
    if typed.starts_with('-') {
        format!("({})", typed)
    } else {
        typed
    }
}

/// `Mid(s, start)` / `Mid(s, start, len)` → a substring counted in **characters**.
/// `start` is 1-indexed (VB), so we skip `start - 1`. Iterating over `chars()`
/// (rather than a byte slice) keeps VB's character semantics and never splits a
/// multi-byte char. Literal positions are folded so the output stays clean.
fn render_mid(s: &Expr, start: &Expr, len: Option<&Expr>) -> String {
    let sr = render_recv(s);
    let skip = match start {
        Expr::Int(n) => (n - 1).max(0).to_string(),
        other => format!("(({}) - 1) as usize", render_expr(other, None)),
    };
    match len {
        Some(len) => format!(
            "{}.chars().skip({}).take({}).collect::<String>()",
            sr,
            skip,
            as_usize_arg(len)
        ),
        None => format!("{}.chars().skip({}).collect::<String>()", sr, skip),
    }
}

/// Render an expression that must be a `usize` (a slice/iterator count). An
/// integer literal is emitted bare (it infers `usize`); anything else is cast.
fn as_usize_arg(e: &Expr) -> String {
    match e {
        Expr::Int(n) => n.to_string(),
        other => format!("({}) as usize", render_expr(other, None)),
    }
}

fn is_arithmetic(op: BinOp) -> bool {
    matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod)
}

/// Binding power — higher binds tighter.
fn prec(op: BinOp) -> u8 {
    match op {
        // Logical operators are loosest (looser than comparison), as in Rust.
        BinOp::Or => 1,
        BinOp::Xor => 2,
        BinOp::And => 3,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => 4,
        BinOp::Concat => 5,
        BinOp::Add | BinOp::Sub => 6,
        BinOp::Mul | BinOp::Div | BinOp::Mod => 7,
        BinOp::Pow => 8,
    }
}

fn op_str(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::Le => "<=",
        BinOp::Ge => ">=",
        BinOp::Pow => "^",    // handled separately (lowers to powi/powf)
        BinOp::Concat => "&", // handled separately
        BinOp::And => "&&",
        BinOp::Or => "||",
        BinOp::Xor => "^",
    }
}

fn fmt_float(f: f64) -> String {
    let s = f.to_string();
    if s.contains('.') || s.contains('e') || s.contains('E') {
        s
    } else {
        format!("{}.0", s)
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Escape a string literal for use *inside* a format string: the usual
/// escapes plus `{`/`}`, which format! treats as placeholders.
fn escape_fmt(s: &str) -> String {
    escape(s).replace('{', "{{").replace('}', "}}")
}

/// Collect an `&` concatenation chain, in order, into one format string and
/// its arguments. String literals fold into the format string; every other
/// operand becomes a `{}` argument.
fn flatten_concat(e: &Expr) -> (String, Vec<String>) {
    fn walk(e: &Expr, fmt: &mut String, args: &mut Vec<String>) {
        match e {
            Expr::Binary { op: BinOp::Concat, lhs, rhs } => {
                walk(lhs, fmt, args);
                walk(rhs, fmt, args);
            }
            Expr::Str(s) => fmt.push_str(&escape_fmt(s)),
            other => {
                fmt.push_str("{}");
                args.push(render_prec(other, None, 0, false));
            }
        }
    }
    let mut fmt = String::new();
    let mut args = Vec::new();
    walk(e, &mut fmt, &mut args);
    (fmt, args)
}

/// Names that are reassigned somewhere in the body need `let mut`.
fn collect_mutated(stmts: &[Stmt], set: &mut HashSet<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::Assign { target, .. } => {
                if let Some(root) = lvalue_root(target) {
                    set.insert(root);
                }
            }
            // `nums.push(...)` etc. mutate the receiver collection.
            Stmt::Expr(e) => mark_mutating_calls(e, set),
            // `Set Mut a = b` borrows b mutably, so b's own binding must be `mut`.
            Stmt::Set {
                mutable: true,
                value: Expr::Ident(n),
                ..
            } => {
                set.insert(rust_name(n));
            }
            Stmt::If {
                branches,
                else_body,
            } => {
                for (_, body) in branches {
                    collect_mutated(body, set);
                }
                if let Some(body) = else_body {
                    collect_mutated(body, set);
                }
            }
            Stmt::For { body, .. } | Stmt::ForEach { body, .. } | Stmt::DoLoop { body, .. } => {
                collect_mutated(body, set)
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    collect_mutated(&arm.body, set);
                }
            }
            _ => {}
        }
    }
}

fn rust_fn_name(name: &str, line: usize, diags: &mut Diagnostics) -> String {
    if name == "Main" {
        return "main".to_string();
    }
    let lowered = rust_name(name);
    if lowered != name {
        diags.warn(
            line,
            format!(
                "Function '{}' is '{}' on the Rust side — one rule: a VBR name is \
                 its lowercase self in Rust.",
                name, lowered
            ),
        );
    }
    lowered
}

/// The Rust spelling of a constant: simply uppercased — the same one-rule
/// mapping as `rust_name`, in the constants' case convention.
pub(crate) fn to_screaming(name: &str) -> String {
    name.to_uppercase()
}

/// The real (snake_case) name of a stdlib wrapper method, from its lowercased
/// VBR spelling — `GetString` → `get_string`, `ReadCsv` → `read_csv`. The
/// stdlib is real Rust with real snake_case methods; *user* names are plainly
/// lowercased instead, so this fixed dictionary bridges VBR's surface API to
/// the crate we ship. (Writing the snake_case name directly works too — it
/// passes through `rust_name` unchanged.)
pub(crate) fn stdlib_method(squashed: &str) -> Option<&'static str> {
    Some(match squashed {
        // DataFrame
        "readcsv" => "read_csv",
        "withcolumn" => "with_column",
        "writecsv" => "write_csv",
        "groupby" => "group_by",
        "leftjoin" => "left_join",
        "outerjoin" => "outer_join",
        // FileSystem
        "readlines" => "read_lines",
        "movefile" => "move_file",
        "createfolder" => "create_folder",
        "createfolderall" => "create_folder_all",
        "folderexists" => "folder_exists",
        "deletefolder" => "delete_folder",
        "deletefolderall" => "delete_folder_all",
        // Regex
        "ismatch" => "is_match",
        "findall" => "find_all",
        "replaceall" => "replace_all",
        // DateTime
        "adddays" => "add_days",
        "addhours" => "add_hours",
        "addminutes" => "add_minutes",
        "diffdays" => "diff_days",
        "diffhours" => "diff_hours",
        // Json (`to_string` is also the universal Rust method — same mapping).
        "tostring" => "to_string",
        "topretty" => "to_pretty",
        "haskey" => "has_key",
        "getstring" => "get_string",
        "getint" => "get_int",
        "getfloat" => "get_float",
        "getbool" => "get_bool",
        "getarray" => "get_array",
        "setstring" => "set_string",
        "setint" => "set_int",
        "setbool" => "set_bool",
        "asstring" => "as_string",
        "asint" => "as_int",
        "asfloat" => "as_float",
        "asbool" => "as_bool",
        _ => return None,
    })
}

/// The canonical name of a vbr_stdlib namespace, if `name` is one. Stdlib calls
/// use `.` in VBR (`FileSystem.Read`) but `::` in Rust (`FileSystem::read`).
pub(crate) fn stdlib_type(name: &str) -> Option<&'static str> {
    match name.to_ascii_lowercase().as_str() {
        "filesystem" => Some("FileSystem"),
        "json" => Some("Json"),
        "datetime" => Some("DateTime"),
        "regex" => Some("Regex"),
        "http" => Some("Http"),
        "dataframe" => Some("DataFrame"),
        _ => None,
    }
}

/// All stdlib namespace names, for emitting `use vbr_stdlib::{…}`.
const STDLIB_TYPES: [&str; 6] = ["FileSystem", "Json", "DateTime", "Regex", "Http", "DataFrame"];

/// The stdlib namespaces a compiled program uses (for enabling Cargo features).
/// `FileSystem` is std-only and needs no feature; the rest map to a feature.
pub fn stdlib_used(diags: &Diagnostics) -> Vec<String> {
    STDLIB_TYPES
        .iter()
        .filter(|t| diags.has_mark(&format!("stdlib:{}", t)))
        .map(|s| s.to_string())
        .collect()
}

/// Mark stdlib types that appear as *type annotations* (params, returns, Dims,
/// fields, Vec/Map elements) so their `use` is emitted even without a `Type.X()`
/// call — e.g. `ByVal d As DateTime`.
fn mark_stdlib_types(program: &Program, diags: &mut Diagnostics) {
    fn mark_name(name: &str, diags: &mut Diagnostics) {
        if let Some(canon) = stdlib_type(name) {
            diags.mark(&format!("stdlib:{}", canon));
        }
    }
    fn mark_decltype(dt: &DeclType, diags: &mut Diagnostics) {
        match dt {
            DeclType::Named(n) => mark_name(n, diags),
            DeclType::Vec(t) | DeclType::Option(t) => mark_decltype(t, diags),
            DeclType::Result(t, e) => {
                mark_decltype(t, diags);
                mark_decltype(e, diags);
            }
            DeclType::Map(k, v) => {
                mark_decltype(k, diags);
                mark_decltype(v, diags);
            }
            DeclType::Tuple(ts) => {
                for t in ts {
                    mark_decltype(t, diags);
                }
            }
            _ => {}
        }
    }
    fn walk(stmts: &[Stmt], diags: &mut Diagnostics) {
        for s in stmts {
            match s {
                Stmt::Dim { ty, .. } => mark_decltype(ty, diags),
                Stmt::If { branches, else_body } => {
                    for (_, b) in branches {
                        walk(b, diags);
                    }
                    if let Some(b) = else_body {
                        walk(b, diags);
                    }
                }
                Stmt::For { body, .. }
                | Stmt::ForEach { body, .. }
                | Stmt::DoLoop { body, .. } => walk(body, diags),
                Stmt::Match { arms, .. } => {
                    for a in arms {
                        walk(&a.body, diags);
                    }
                }
                _ => {}
            }
        }
    }

    for s in &program.structs {
        for f in &s.fields {
            mark_decltype(&f.ty, diags);
        }
    }
    for func in &program.functions {
        for p in &func.params {
            mark_decltype(&p.ty, diags);
        }
        if let Some(rt) = &func.ret {
            mark_decltype(rt, diags);
        }
        walk(&func.body, diags);
    }
}

/// The Rust spelling of a VBR name: simply lowercased. VBR identifiers are
/// case-insensitive (VB style), so lowercase is the one canonical form — and
/// it makes the mapping trivially predictable: inside a `Rust … End Rust`
/// block or a `Match` pattern, `myTotal` is `mytotal`, never a guess about
/// where underscores landed. Underscores the user writes are kept, so
/// already-snake_case names (and Rust method names like `push_str`) pass
/// through unchanged.
pub(crate) fn rust_name(name: &str) -> String {
    name.to_lowercase()
}
