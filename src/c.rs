//! The **C backend** — Bust's third target, after Rust and Python. Where the
//! Python target could lean on dynamic typing and a garbage collector, C gives
//! us neither: every declaration needs a type (supplied by the neutral typing
//! pass, [`crate::types`]) and every heap value must be freed by hand. Bust's
//! answer to the latter is *leak-by-default* with an explicit `x = Nothing`
//! release hook (`Stmt::Destroy`) — which is exactly what makes a C target
//! worth having: it puts on the page the manual-memory cost that Rust's
//! ownership hides.
//!
//! Slice 1 is the core language over **scalars + strings**: functions, `Dim`,
//! arithmetic (with the widening rules), `If`/`For`/`Do`, comparison/logical
//! operators, the maths builtins, `Debug.Print` and `&` concatenation. The
//! output is a single self-contained `.c` with a small runtime inlined at the
//! top (the C analogue of the Python prelude / `vbr_stdlib`); it's split into a
//! real `vbr_runtime.{h,c}` in a later slice, once it grows.

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::iter;
use crate::pattern::{self, Pat};
use crate::transpiler::{body_never_returns, convert_returns};
use crate::types::{type_program, TypeTable};

/// The generated C, plus any constructs that couldn't cross cleanly.
pub struct CProgram {
    pub code: String,
    pub warnings: Vec<String>,
    /// Vendored C libraries to bundle beside `main.c` (base names under
    /// `csupport/`) — non-empty means a project folder, not a single file.
    pub vendored: Vec<String>,
    /// Extra `-l` linker flags the `Makefile` must pass.
    pub link_flags: Vec<String>,
    /// `(c_line, vbr_line)` 1-based checkpoints, same idea as the Rust
    /// transpiler's `line_map` — so an IDE can scroll the C pane with the
    /// cursor. Empty if nothing was emitted.
    pub line_map: Vec<(usize, usize)>,
}

/// Emit C for a whole parsed program.
pub fn emit_c(program: &Program) -> CProgram {
    let mut e = Emitter {
        types: type_program(program),
        out: String::new(),
        decls: String::new(),
        const_names: HashMap::new(),
        enums: HashMap::new(),
        aliases: HashMap::new(),
        match_counter: 0,
        tmp_counter: 0,
        current_ret: None,
        type_hint: None,
        indent: 0,
        warnings: Vec::new(),
        needs_math: false,
        needs_rnd: false,
        need_dup: false,
        need_from_ll: false,
        need_from_bool: false,
        need_from_double: false,
        need_from_float: false,
        need_concat: false,
        stdlib_fns: std::collections::BTreeSet::new(),
        needs_fs: false,
        needs_datetime: false,
        needs_shell: false,
        needs_regex: false,
        needs_unistd: false,
        needs_json: false,
        needs_database: false,
        needs_http: false,
        needs_strings: false,
        needs_instr: false,
        needs_val: false,
        needs_cdbl: false,
        needs_clng: false,
        needs_cint: false,
        needs_input: false,
        needs_round_places: false,
        needs_split: false,
        needs_join: false,
        needs_space: false,
        needs_fmt_double: false,
        needs_fmt_ll: false,
        skip_auto_try: false,
        emitting_main: false,
        wrap_ok: false,
        user_fns: HashSet::new(),
        user_methods: HashSet::new(),
        user_fn_ret: HashMap::new(),
        err_names: HashSet::new(),
        success_ret: None,
        line_map: Vec::new(),
    };
    e.program(program);
    e.finish(program)
}

/// What the C backend needs to know about an enum: its C name, whether any
/// variant carries data (a tagged union vs a plain C `enum`), and each variant's
/// payload types.
struct EnumInfo {
    name: String,
    is_data: bool,
    variants: Vec<(String, Vec<DeclType>)>,
}

/// How one `Match` arm lowers: a C condition (`None` = matches anything), the
/// payload locals to declare in its block, and a scrutinee alias for a bare `x`.
#[derive(Default)]
struct ArmPlan {
    cond: Option<String>,
    locals: Vec<(String, String, String)>,
    alias: Option<String>,
}

struct Emitter {
    types: TypeTable,
    out: String,
    /// Enum + struct `typedef`s + module constants, emitted before the functions.
    decls: String,
    /// Module constants keep their exact casing (`MAX_RETRIES`) — lowercased
    /// name → original spelling, so a reference isn't snake-cased like a local.
    const_names: HashMap<String, String>,
    /// Enum metadata by lowercased name.
    enums: HashMap<String, EnumInfo>,
    /// Active pattern bindings inside a `Match` arm: a bound name → the C
    /// expression it stands for (`x`→`_m0`), so guards/bodies can reference it
    /// without a separate declaration (needed since a C `if` condition can't
    /// introduce one).
    aliases: HashMap<String, String>,
    /// Per-`Match` counter, so scrutinee temps (`_m0`) are unique and nest.
    match_counter: usize,
    /// Counter for loop-index / iterator temps (`_i0`).
    tmp_counter: usize,
    /// The current function's return type — tells a `?` how to re-wrap a
    /// propagated failure.
    current_ret: Option<DeclType>,
    /// The Option/Result type a `Some`/`Ok`/`Err`/`None` should construct, taken
    /// from the surrounding `Return`/`Dim` context.
    type_hint: Option<DeclType>,
    indent: usize,
    warnings: Vec<String>,
    // Runtime helpers switched on as the body needs them.
    needs_math: bool,
    /// `Rnd()` — stdlib `rand` in `[0, 1)`, no libm.
    needs_rnd: bool,
    need_dup: bool,
    need_from_ll: bool,
    need_from_bool: bool,
    need_from_double: bool,
    need_from_float: bool,
    need_concat: bool,
    // Standard-library runtime functions used (`fs_read`, …) — emitted on demand.
    stdlib_fns: std::collections::BTreeSet<&'static str>,
    // Which stdlib namespaces' runtimes to emit + which system headers to pull in.
    needs_fs: bool,
    needs_datetime: bool,
    needs_shell: bool,
    needs_regex: bool,
    needs_unistd: bool,
    /// The `Json` namespace/value type — wraps vendored cJSON (a project folder).
    needs_json: bool,
    /// The `Database` value type — wraps SQLite (links `-lsqlite3`; rows are
    /// `Json`, so it implies `needs_json`).
    needs_database: bool,
    /// The `Http` namespace — one-shot requests over libcurl (links `-lcurl`).
    needs_http: bool,
    /// Free string builtins (`Len`/`Mid`/`UCase`/…) — UTF-8 character helpers.
    needs_strings: bool,
    /// `InStr` — needs `Option<Long>` plus `vbr_instr`.
    needs_instr: bool,
    /// `Val` — lenient parse, no Result.
    needs_val: bool,
    needs_cdbl: bool,
    needs_clng: bool,
    needs_cint: bool,
    /// `InputBox`.
    needs_input: bool,
    /// `Round(x, places)` — scale helper on top of `<math.h>` `round`.
    needs_round_places: bool,
    /// `Split` / `Join` / `Space` — Vec<String> helpers on top of `vbr_dup`.
    needs_split: bool,
    needs_join: bool,
    needs_space: bool,
    needs_fmt_double: bool,
    needs_fmt_ll: bool,
    skip_auto_try: bool,
    emitting_main: bool,
    wrap_ok: bool,
    user_fns: HashSet<String>,
    user_methods: HashSet<String>,
    user_fn_ret: HashMap<String, Option<DeclType>>,
    err_names: HashSet<String>,
    success_ret: Option<DeclType>,
    /// Checkpoints into `out` (function bodies). Offset by the header/runtime
    /// prefix in `finish` so they index the final `.c`.
    line_map: Vec<(usize, usize)>,
}

impl Emitter {
    fn warn(&mut self, msg: impl Into<String>) {
        self.warnings.push(format!("⚠ {}", msg.into()));
    }

    fn line(&mut self, text: &str) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    /// Record that the next C line in `out` came from this VBR source line.
    fn map_line(&mut self, vbr_line: usize) {
        let c_line = self.out.matches('\n').count() + 1;
        self.line_map.push((c_line, vbr_line));
    }

    fn program(&mut self, program: &Program) {
        if !program.windows.is_empty() || !program.screens.is_empty() || !program.pages.is_empty() || !program.sketches.is_empty() {
            self.warn(
                "GUI/TUI/Web surfaces (`Window`/`Screen`/`Page`/`Sketch`) are Rust-only — \
                 the C target is for the core language.",
            );
        }
        for c in &program.constants {
            self.const_names.insert(c.name.to_ascii_lowercase(), c.name.clone());
        }
        self.user_fns = program
            .functions
            .iter()
            .filter(|f| f.receiver.is_some() || !f.name.eq_ignore_ascii_case("main"))
            .map(|f| c_name(&f.name))
            .collect();
        self.user_methods = program
            .functions
            .iter()
            .filter(|f| f.receiver.is_some())
            .map(|f| c_name(&f.name))
            .collect();
        self.user_fn_ret = program
            .functions
            .iter()
            .filter(|f| f.receiver.is_some() || !f.name.eq_ignore_ascii_case("main"))
            .map(|f| (c_name(&f.name), f.ret.clone()))
            .collect();
        for e in &program.enums {
            let is_data = e.variants.iter().any(|v| !v.payload.is_empty());
            let variants = e.variants.iter().map(|v| (v.name.clone(), v.payload.clone())).collect();
            self.enums
                .insert(e.name.to_ascii_lowercase(), EnumInfo { name: e.name.clone(), is_data, variants });
        }
        self.enum_typedefs(program);
        self.struct_typedefs(program);
        // Stdlib value types (`DateTime`, `Process`) need their `typedef` before
        // any `Result<DateTime>` runtime — detect them from the inferred types.
        for ty in self.types.values() {
            if type_mentions(ty, "DateTime") {
                self.needs_datetime = true;
            }
            if type_mentions(ty, "Process") {
                self.needs_shell = true;
            }
            if type_mentions(ty, "Json") {
                self.needs_json = true;
            }
            if type_mentions(ty, "Database") {
                self.needs_database = true;
                self.needs_json = true; // query rows come back as `Json`
            }
        }
        self.stdlib_type_defs();
        self.constants(program);
        for (i, func) in program.functions.iter().enumerate() {
            if i > 0 {
                self.out.push('\n');
            }
            self.function(func);
        }
        // Emitted last (into `decls`, which `finish` places before the function
        // bodies): only now is every `needs_*` flag settled — a stateless
        // namespace like `Http` sets its flag while its call is rendered above,
        // and its whole-block runtime names types that must be instantiated here.
        self.collection_runtimes(program);
    }

    /// `Enum … End Enum` → a plain C `enum` (all variants payload-free) or a
    /// **tagged union** (a `tag` enum + a `union` of per-variant payload structs).
    fn enum_typedefs(&mut self, program: &Program) {
        for e in &program.enums {
            let name = &e.name;
            let is_data = e.variants.iter().any(|v| !v.payload.is_empty());
            let tags: Vec<String> = e.variants.iter().map(|v| format!("{}_{}", name, v.name)).collect();
            if !is_data {
                // A C-like enum: `typedef enum { Suit_Hearts, … } Suit;`
                self.decls.push_str(&format!("typedef enum {{ {} }} {};\n\n", tags.join(", "), name));
                continue;
            }
            // A sum type: a tag enum, then a struct wrapping tag + payload union.
            self.decls.push_str(&format!("typedef enum {{ {} }} {}Tag;\n", tags.join(", "), name));
            self.decls.push_str("typedef struct {\n");
            self.decls.push_str(&format!("    {}Tag tag;\n", name));
            self.decls.push_str("    union {\n");
            for v in &e.variants {
                if v.payload.is_empty() {
                    continue;
                }
                let fields: Vec<String> = v
                    .payload
                    .iter()
                    .enumerate()
                    .map(|(i, t)| format!("{} f{};", c_type(t), i))
                    .collect();
                self.decls.push_str(&format!("        struct {{ {} }} {};\n", fields.join(" "), v.name));
            }
            self.decls.push_str("    } data;\n");
            self.decls.push_str(&format!("}} {};\n\n", name));
        }
    }

    /// `Type Person … End Type` → a `typedef`'d C struct.
    fn struct_typedefs(&mut self, program: &Program) {
        for s in &program.structs {
            self.decls.push_str("typedef struct {\n");
            for f in &s.fields {
                self.decls
                    .push_str(&format!("    {} {};\n", c_type(&f.ty), c_name(&f.name)));
            }
            self.decls.push_str(&format!("}} {};\n\n", s.name));
        }
    }

    /// `typedef`s for stdlib value types, before any container/`Result` runtime
    /// that embeds them.
    fn stdlib_type_defs(&mut self) {
        if self.needs_datetime {
            self.decls.push_str("typedef struct tm DateTime;\n\n");
        }
        if self.needs_shell {
            // A launched child: its pid, whether it's been reaped, and its code.
            self.decls
                .push_str("typedef struct { int pid; int reaped; long long code; } Process;\n\n");
        }
        if self.needs_json {
            // A thin handle over a vendored cJSON node. The node is owned by the
            // parsed document (children borrow it); copying the wrapper is a cheap
            // pointer copy — the leak-by-default memory model keeps the doc alive.
            self.decls.push_str("typedef struct { cJSON *node; } Json;\n\n");
        }
        if self.needs_database {
            // A live SQLite connection handle (linked `-lsqlite3`).
            self.decls.push_str("typedef struct { sqlite3 *conn; } Database;\n\n");
        }
    }

    /// Emit a monomorphised runtime (`typedef` + functions) for every `Vec<T>`
    /// and `HashMap<K, V>` instantiation the program uses — inner types first,
    /// so a nested container is declared before the one that holds it.
    fn collection_runtimes(&mut self, program: &Program) {
        let mut c = Collected::default();
        gather_types(program, &mut c);
        // Every user function is fallible — instantiate Result<T, String> for its
        // success type even when the source never wrote `As Result`.
        for f in &program.functions {
            if f.receiver.is_none() && f.name.eq_ignore_ascii_case("main") {
                continue;
            }
            visit_ty(&result_of(f.ret.as_ref()), &mut c);
        }
        // Also from every *inferred* expression type — this is how a stdlib
        // call's `Result<…>` return (never written as a `Dim`) gets a runtime.
        // Visit in source order (the table is a HashMap) so emission is stable.
        let mut inferred: Vec<(&crate::span::Span, &DeclType)> = self.types.iter().collect();
        inferred.sort_by_key(|(s, _)| (s.start, s.end));
        for (_, ty) in inferred {
            visit_ty(ty, &mut c);
        }
        // The `Json` runtime is emitted as one whole block, so every wrapper it
        // names must exist even when the program uses only a few accessors —
        // force-instantiate the full set of `Result<T>` returns (+`Vec<Json>`).
        if self.needs_json {
            let res = |t: DeclType| DeclType::Result(Box::new(t), Box::new(DeclType::Plain(Type::Text)));
            let json = || DeclType::Named("Json".to_string());
            for ty in [
                res(DeclType::Plain(Type::Text)),
                res(DeclType::Plain(Type::Long)),
                res(DeclType::Plain(Type::Double)),
                res(DeclType::Plain(Type::Boolean)),
                res(json()),
                DeclType::Vec(Box::new(json())),
                res(DeclType::Vec(Box::new(json()))),
            ] {
                visit_ty(&ty, &mut c);
            }
        }
        // The `Database` runtime is one block too — force its `Result<Database>`
        // return and the `Vec<String>` params type (an all-empty-`[]` program
        // would otherwise never instantiate `Vec_str`).
        if self.needs_database {
            let res = |t: DeclType| DeclType::Result(Box::new(t), Box::new(DeclType::Plain(Type::Text)));
            for ty in [
                res(DeclType::Named("Database".to_string())),
                DeclType::Vec(Box::new(DeclType::Plain(Type::Text))),
            ] {
                visit_ty(&ty, &mut c);
            }
        }
        // `Http` (one block) returns `Result<String>` and `Post` takes a
        // `Map<String,String>` of headers — force both so a `Get`-only program
        // still declares the `Map_str_str` the runtime names.
        if self.needs_http {
            let text = || DeclType::Plain(Type::Text);
            for ty in [
                DeclType::Result(Box::new(text()), Box::new(text())),
                DeclType::Map(Box::new(text()), Box::new(text())),
            ] {
                visit_ty(&ty, &mut c);
            }
        }
        // String/conversion builtins name these wrappers even when the source
        // never wrote `As Option` / `As Result` (Handle/`?` still need the typedef).
        if self.needs_instr {
            visit_ty(&DeclType::Option(Box::new(DeclType::Plain(Type::Long))), &mut c);
        }
        if self.needs_cdbl {
            visit_ty(
                &DeclType::Result(
                    Box::new(DeclType::Plain(Type::Double)),
                    Box::new(DeclType::Plain(Type::Text)),
                ),
                &mut c,
            );
        }
        if self.needs_clng {
            visit_ty(
                &DeclType::Result(
                    Box::new(DeclType::Plain(Type::Long)),
                    Box::new(DeclType::Plain(Type::Text)),
                ),
                &mut c,
            );
        }
        if self.needs_cint {
            visit_ty(
                &DeclType::Result(
                    Box::new(DeclType::Plain(Type::Integer)),
                    Box::new(DeclType::Plain(Type::Text)),
                ),
                &mut c,
            );
        }
        if self.needs_input {
            let text = DeclType::Plain(Type::Text);
            visit_ty(
                &DeclType::Result(Box::new(text.clone()), Box::new(text)),
                &mut c,
            );
        }
        if self.needs_split || self.needs_join {
            visit_ty(
                &DeclType::Vec(Box::new(DeclType::Plain(Type::Text))),
                &mut c,
            );
        }
        for ty in &c.order {
            match ty {
                DeclType::Option(_) => self.emit_option_runtime(ty),
                DeclType::Result(..) => self.emit_result_runtime(ty),
                DeclType::Vec(_) => self.emit_vec_runtime(ty),
                DeclType::Map(..) => self.emit_map_runtime(ty),
                _ => {}
            }
        }
    }

    /// `Option<T>` → a `{ is_some, value }` struct + an `_unwrap`.
    fn emit_option_runtime(&mut self, ty: &DeclType) {
        let DeclType::Option(t) = ty else { return };
        let et = c_type(t);
        let n = opt_name(ty);
        self.decls.push_str(&format!("typedef struct {{ bool is_some; {et} value; }} {n};\n"));
        self.decls.push_str(&format!(
            "static {et} {n}_unwrap({n} o) {{ \
             if (!o.is_some) {{ fprintf(stderr, \"unwrapped a None\\n\"); exit(1); }} return o.value; }}\n\n"
        ));
    }

    /// `Result<T, E>` → a `{ is_ok, ok, err }` struct (+`_unwrap`); a unit
    /// success (`Result<()>`) drops the `ok` field.
    fn emit_result_runtime(&mut self, ty: &DeclType) {
        let DeclType::Result(t, e) = ty else { return };
        let n = res_name(ty);
        let et = c_type(e);
        if is_unit(t) {
            self.decls.push_str(&format!("typedef struct {{ bool is_ok; {et} err; }} {n};\n"));
            // A unit `.Unwrap()` returns nothing — it just aborts on an `Err`.
            self.decls.push_str(&format!(
                "static void {n}_unwrap({n} r) {{ \
                 if (!r.is_ok) {{ fprintf(stderr, \"unwrapped an Err\\n\"); exit(1); }} }}\n\n"
            ));
            return;
        }
        let ot = c_type(t);
        self.decls.push_str(&format!("typedef struct {{ bool is_ok; {ot} ok; {et} err; }} {n};\n"));
        self.decls.push_str(&format!(
            "static {ot} {n}_unwrap({n} r) {{ \
             if (!r.is_ok) {{ fprintf(stderr, \"unwrapped an Err\\n\"); exit(1); }} return r.ok; }}\n\n"
        ));
    }

    fn emit_vec_runtime(&mut self, ty: &DeclType) {
        let DeclType::Vec(elem) = ty else { return };
        let et = c_type(elem);
        let n = vec_name(ty);
        self.decls.push_str(&format!("typedef struct {{ {et}* data; size_t len, cap; }} {n};\n"));
        self.decls.push_str(&format!(
            "static void {n}_push({n}* v, {et} x) {{\n    \
             if (v->len == v->cap) {{ v->cap = v->cap ? v->cap * 2 : 4; \
             v->data = realloc(v->data, v->cap * sizeof({et})); }}\n    \
             v->data[v->len++] = x;\n}}\n"
        ));
        self.decls.push_str(&format!(
            "static {n} {n}_of(size_t count, {et}* items) {{\n    \
             {n} v = {{0}};\n    \
             for (size_t i = 0; i < count; i++) {n}_push(&v, items[i]);\n    \
             return v;\n}}\n\n"
        ));
    }

    fn emit_map_runtime(&mut self, ty: &DeclType) {
        let DeclType::Map(k, v) = ty else { return };
        let kt = c_type(k);
        let vt = c_type(v);
        let n = map_name(ty);
        // A linear-probe-free assoc list — simple and readable; iteration is in
        // insertion order (so a HashMap program is snapshot-only, not diffed
        // against Rust's randomised order).
        let key_eq = if is_text(k) {
            "strcmp(m->entries[i].key, k) == 0"
        } else {
            "m->entries[i].key == k"
        };
        self.decls.push_str(&format!("typedef struct {{ {kt} key; {vt} val; }} {n}Entry;\n"));
        self.decls
            .push_str(&format!("typedef struct {{ {n}Entry* entries; size_t len, cap; }} {n};\n"));
        self.decls.push_str(&format!(
            "static void {n}_insert({n}* m, {kt} k, {vt} val) {{\n    \
             for (size_t i = 0; i < m->len; i++) if ({key_eq}) {{ m->entries[i].val = val; return; }}\n    \
             if (m->len == m->cap) {{ m->cap = m->cap ? m->cap * 2 : 4; \
             m->entries = realloc(m->entries, m->cap * sizeof({n}Entry)); }}\n    \
             m->entries[m->len].key = k; m->entries[m->len].val = val; m->len++;\n}}\n"
        ));
        self.decls.push_str(&format!(
            "static {vt}* {n}_get({n}* m, {kt} k) {{\n    \
             for (size_t i = 0; i < m->len; i++) if ({key_eq}) return &m->entries[i].val;\n    \
             return NULL;\n}}\n"
        ));
        self.decls.push_str(&format!(
            "static bool {n}_contains({n}* m, {kt} k) {{ return {n}_get(m, k) != NULL; }}\n\n"
        ));
    }

    /// `Const X As T = v` → a file-scope `static const` (case preserved).
    fn constants(&mut self, program: &Program) {
        for c in &program.constants {
            let cty = c_type(&DeclType::Plain(c.ty));
            let val = self.expr(&c.value);
            self.decls.push_str(&format!("static const {} {} = {};\n", cty, c.name, val));
        }
        if !program.constants.is_empty() {
            self.decls.push('\n');
        }
    }

    fn function(&mut self, func: &Function) {
        let is_main = func.receiver.is_none() && func.name.eq_ignore_ascii_case("main");
        self.emitting_main = is_main;
        self.wrap_ok = !is_main;
        self.success_ret = func.ret.clone();
        self.current_ret = if is_main {
            func.ret.clone()
        } else {
            Some(result_of(func.ret.as_ref()))
        };
        self.map_line(func.line);
        let sig = self.signature(func, is_main);
        self.line(&format!("{} {{", sig));
        self.indent += 1;

        let mut body = func.body.clone();
        convert_returns(&mut body, &func.name);
        self.block(&body);

        if !body_never_returns(&body) {
            if is_main {
                self.line("return 0;");
            } else {
                let ty = result_of(func.ret.as_ref());
                self.line(&format!("return {};", self.ok_literal(&ty, None)));
            }
        }
        self.indent -= 1;
        self.line("}");
        self.emitting_main = false;
        self.wrap_ok = false;
    }

    /// The C signature (no trailing `;` — the caller adds `{` or `;`).
    fn signature(&self, func: &Function, is_main: bool) -> String {
        if is_main {
            return "int main(void)".to_string();
        }
        let ret = c_type(&result_of(func.ret.as_ref()));
        let mut params: Vec<String> = Vec::new();
        // A method `Function Person.M()` takes the receiver first, by pointer
        // (so it can mutate, and reads go through `self->field`).
        if let Some(recv) = &func.receiver {
            params.push(format!("{}* self", recv));
        }
        for p in &func.params {
            params.push(format!("{} {}", c_type(&p.ty), c_name(&p.name)));
        }
        let params = if params.is_empty() { "void".to_string() } else { params.join(", ") };
        let name = match &func.receiver {
            Some(recv) => format!("{}_{}", recv, c_name(&func.name)),
            None => c_name(&func.name),
        };
        format!("{} {}({})", ret, name, params)
    }

    fn block(&mut self, stmts: &[Stmt]) {
        for s in stmts {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::LineMark(vbr_line) => self.map_line(*vbr_line),
            Stmt::Comment(c) => {
                let text = c.trim_start_matches(['\'', ' ']).to_string();
                self.line(&format!("// {}", text));
            }
            Stmt::Dim { name, ty, init, .. } => self.dim(name, ty, init.as_ref()),
            Stmt::Assign { target, value, op } => {
                let t = self.expr(target);
                let v = self.expr(value);
                match op {
                    Some(o) => self.line(&format!("{} {}= {};", t, bin_op(*o), v)),
                    None => self.line(&format!("{} = {};", t, v)),
                }
            }
            // `x = Nothing` → release the heap value (the whole reason the C
            // target exists): free it and null the pointer.
            Stmt::Destroy { name, .. } => {
                let n = c_name(name);
                self.line(&format!("free({});", n));
                self.line(&format!("{} = NULL;", n));
            }
            Stmt::Print(e) => {
                let s = self.as_str(e);
                self.line(&format!("printf(\"%s\\n\", {});", s));
            }
            Stmt::Return(Some(e)) => {
                self.type_hint = self.success_ret.clone();
                let v = self.expr(e);
                self.type_hint = None;
                if self.wrap_ok {
                    if let Some(ty) = self.current_ret.clone() {
                        self.line(&format!("return {};", self.ok_literal(&ty, Some(v))));
                    } else {
                        self.line(&format!("return {};", v));
                    }
                } else {
                    self.line(&format!("return {};", v));
                }
            }
            Stmt::Return(None) => {
                if self.wrap_ok {
                    if let Some(ty) = self.current_ret.clone() {
                        self.line(&format!("return {};", self.ok_literal(&ty, None)));
                    } else {
                        self.line("return;");
                    }
                } else if self.emitting_main {
                    self.line("return 0;");
                } else {
                    self.line("return;");
                }
            }
            Stmt::RaiseError(e) => {
                let msg = match &e.kind {
                    ExprKind::Str(s) => {
                        self.need_dup = true;
                        format!("vbr_dup({})", c_string(s))
                    }
                    _ => {
                        self.need_dup = true;
                        format!("vbr_dup({})", self.as_str(e))
                    }
                };
                if let Some(ty) = self.current_ret.clone() {
                    self.line(&format!("return {};", self.err_literal(&ty, &msg)));
                } else {
                    self.line(&format!("fprintf(stderr, \"Error: %s\\n\", {}); return 1;", msg));
                }
            }
            Stmt::HandleErr { target, call, err_name, body, .. } => {
                self.emit_handle(target.as_ref(), call, err_name, body);
            }
            // A bare expression used for its effect (`alice.HaveBirthday()`).
            Stmt::Expr(e) => {
                let v = self.expr(e);
                self.line(&format!("{};", v));
            }
            Stmt::If { branches, else_body } => self.if_stmt(branches, else_body.as_deref()),
            Stmt::For { var, from, to, step, body, .. } => self.for_stmt(var, from, to, step.as_ref(), body),
            Stmt::DoLoop { cond, body } => self.do_loop(cond, body),
            Stmt::ForEach { var1, var2, iter, body } => {
                self.for_each(var1, var2.as_deref(), iter, body)
            }
            Stmt::Match { scrutinee, arms, .. } => self.match_stmt(scrutinee, arms),
            Stmt::Break => self.line("break;"),
            Stmt::Continue => self.line("continue;"),
            other => {
                self.warn(format!("`{}` doesn't lower to C yet.", stmt_name(other)));
                self.line(&format!("/* [Bust→C] unsupported: {} */", stmt_name(other)));
            }
        }
    }

    fn dim(&mut self, name: &str, ty: &DeclType, init: Option<&Expr>) {
        let cty = c_type(ty);
        let var = c_name(name);
        let is_collection = matches!(ty, DeclType::Vec(_) | DeclType::Map(..));
        match init {
            None => {
                let zero = if is_collection {
                    "{0}"
                } else if is_text(ty) {
                    "NULL"
                } else {
                    "0"
                };
                self.line(&format!("{} {} = {};", cty, var, zero));
            }
            // An empty list literal → an empty container of the declared type.
            Some(Expr { kind: ExprKind::List(items), .. }) if items.is_empty() => {
                self.line(&format!("{} {} = {{0}};", cty, var));
            }
            // An iterator terminal (`.collect()/.sum()/.any()`) has no expression
            // form in C — it becomes a preceding loop that fills `var`.
            Some(e) if is_iter_terminal(e) => self.iter_dim(&var, ty, e),
            // A string literal is copied onto the heap so it's uniform with
            // concat results (both `char*` you may later `= Nothing`).
            Some(Expr { kind: ExprKind::Str(s), .. }) if is_text(ty) => {
                self.need_dup = true;
                let lit = c_string(s);
                self.line(&format!("{} {} = vbr_dup({});", cty, var, lit));
            }
            Some(e) => {
                self.type_hint = Some(ty.clone());
                let v = self.expr(e);
                self.type_hint = None;
                self.line(&format!("{} {} = {};", cty, var, v));
            }
        }
    }

    /// Lower a `Dim v = <chain>.collect()/.sum()/.any()` to the empty/zero
    /// initialiser plus the loop that fills it (C has no comprehension form). The
    /// chain shape comes from the shared [`iter`] analysis.
    fn iter_dim(&mut self, var: &str, ty: &DeclType, e: &Expr) {
        let Some(chain) = iter::parse(e) else { return };
        let cty = c_type(ty);
        match &chain.terminal {
            iter::Terminal::Collect => {
                self.line(&format!("{} {} = {{0}};", cty, var));
                self.collect_loop(var, ty, chain.base, &chain.steps);
            }
            iter::Terminal::Sum => {
                let base = self.expr(chain.base);
                self.line(&format!("{} {} = 0;", cty, var));
                let idx = self.next_tmp();
                self.line(&format!("for (size_t {idx} = 0; {idx} < {base}.len; {idx}++) {{"));
                self.indent += 1;
                self.line(&format!("{} += {}.data[{}];", var, base, idx));
                self.indent -= 1;
                self.line("}");
            }
            iter::Terminal::Any { var: cvar, cond } => self.quantify_dim(var, ty, chain.base, cvar, cond, true),
            iter::Terminal::All { var: cvar, cond } => self.quantify_dim(var, ty, chain.base, cvar, cond, false),
            _ => self.warn("this iterator terminal doesn't lower to C yet."),
        }
    }

    fn quantify_dim(&mut self, var: &str, ty: &DeclType, base: &Expr, cvar: &str, cond: &Expr, any: bool) {
        let cty = c_type(ty);
        let elem = self.elem_ctype(base);
        let base_c = self.expr(base);
        self.line(&format!("{} {} = {};", cty, var, if any { "false" } else { "true" }));
        let idx = self.next_tmp();
        self.line(&format!("for (size_t {idx} = 0; {idx} < {base_c}.len; {idx}++) {{"));
        self.indent += 1;
        self.line(&format!("{} {} = {}.data[{}];", elem, c_name(cvar), base_c, idx));
        let c = self.expr(cond);
        if any {
            self.line(&format!("if ({}) {{ {} = true; break; }}", c, var));
        } else {
            self.line(&format!("if (!({})) {{ {} = false; break; }}", c, var));
        }
        self.indent -= 1;
        self.line("}");
    }

    /// The loop body of a `.collect()` over a base Vec plus up to one adapter.
    fn collect_loop(&mut self, var: &str, target_ty: &DeclType, base: &Expr, steps: &[iter::Step]) {
        let vecn = vec_name(target_ty);
        let base_c = self.expr(base);
        let elem = self.elem_ctype(base);
        let idx = self.next_tmp();
        self.line(&format!("for (size_t {idx} = 0; {idx} < {base_c}.len; {idx}++) {{"));
        self.indent += 1;
        match steps {
            [] => {
                self.line(&format!("{}_push(&{}, {}.data[{}]);", vecn, var, base_c, idx));
            }
            [iter::Step::Filter { var: p, cond }] => {
                self.line(&format!("{} {} = {}.data[{}];", elem, c_name(p), base_c, idx));
                let c = self.expr(cond);
                self.line(&format!("if ({}) {}_push(&{}, {});", c, vecn, var, c_name(p)));
            }
            [iter::Step::Map { var: p, body }] => {
                self.line(&format!("{} {} = {}.data[{}];", elem, c_name(p), base_c, idx));
                let b = self.expr(body);
                self.line(&format!("{}_push(&{}, {});", vecn, var, b));
            }
            _ => self.warn("C supports at most one filter/map before `.collect()` so far."),
        }
        self.indent -= 1;
        self.line("}");
    }

    fn elem_ctype(&self, e: &Expr) -> String {
        match self.type_of(e) {
            DeclType::Vec(elem) => c_type(&elem),
            _ => "long long".to_string(),
        }
    }

    fn next_tmp(&mut self) -> String {
        let t = format!("_i{}", self.tmp_counter);
        self.tmp_counter += 1;
        t
    }

    fn if_stmt(&mut self, branches: &[(Expr, Vec<Stmt>)], else_body: Option<&[Stmt]>) {
        for (i, (cond, body)) in branches.iter().enumerate() {
            let c = self.expr(cond);
            let kw = if i == 0 { "if" } else { "} else if" };
            self.line(&format!("{} ({}) {{", kw, c));
            self.indent += 1;
            self.block(body);
            self.indent -= 1;
        }
        if let Some(body) = else_body {
            self.line("} else {");
            self.indent += 1;
            self.block(body);
            self.indent -= 1;
        }
        self.line("}");
    }

    fn for_stmt(&mut self, var: &str, from: &Expr, to: &Expr, step: Option<&Expr>, body: &[Stmt]) {
        let v = c_name(var);
        let lo = self.expr(from);
        let hi = self.expr(to);
        // A negative literal step counts down (inclusive `>=`); anything else
        // counts up. `For` is inclusive of its bound either way.
        let descending = matches!(step, Some(Expr { kind: ExprKind::Int(n), .. }) if *n < 0)
            || matches!(step, Some(Expr { kind: ExprKind::Float(f), .. }) if *f < 0.0);
        let cmp = if descending { ">=" } else { "<=" };
        let incr = match step {
            Some(s) => {
                let st = self.expr(s);
                format!("{} += {}", v, st)
            }
            None => format!("{}++", v),
        };
        self.line(&format!("for (long long {v} = {lo}; {v} {cmp} {hi}; {incr}) {{"));
        self.indent += 1;
        self.block(body);
        self.indent -= 1;
        self.line("}");
    }

    fn do_loop(&mut self, cond: &Option<DoCond>, body: &[Stmt]) {
        match cond {
            None => {
                self.line("while (1) {");
                self.indent += 1;
                self.block(body);
                self.indent -= 1;
                self.line("}");
            }
            Some(DoCond::PreWhile(c)) | Some(DoCond::PreUntil(c)) => {
                let raw = self.expr(c);
                let test = if matches!(cond, Some(DoCond::PreUntil(_))) {
                    format!("!({})", raw)
                } else {
                    raw
                };
                self.line(&format!("while ({}) {{", test));
                self.indent += 1;
                self.block(body);
                self.indent -= 1;
                self.line("}");
            }
            Some(DoCond::PostWhile(c)) | Some(DoCond::PostUntil(c)) => {
                self.line("do {");
                self.indent += 1;
                self.block(body);
                self.indent -= 1;
                let raw = self.expr(c);
                let test = if matches!(cond, Some(DoCond::PostUntil(_))) {
                    format!("!({})", raw)
                } else {
                    raw
                };
                self.line(&format!("}} while ({});", test));
            }
        }
    }

    /// `For Each x In v … Next` → an index loop over the container's storage.
    fn for_each(&mut self, var1: &str, var2: Option<&str>, iter: &Expr, body: &[Stmt]) {
        let ity = self.type_of(iter);
        let base = self.expr(iter);
        let idx = self.next_tmp();
        match &ity {
            DeclType::Vec(elem) => {
                let et = c_type(elem);
                self.line(&format!("for (size_t {idx} = 0; {idx} < {base}.len; {idx}++) {{"));
                self.indent += 1;
                self.line(&format!("{} {} = {}.data[{}];", et, c_name(var1), base, idx));
                self.block(body);
                self.indent -= 1;
                self.line("}");
            }
            DeclType::Map(k, v) => {
                let kt = c_type(k);
                self.line(&format!("for (size_t {idx} = 0; {idx} < {base}.len; {idx}++) {{"));
                self.indent += 1;
                self.line(&format!("{} {} = {}.entries[{}].key;", kt, c_name(var1), base, idx));
                if let Some(v2) = var2 {
                    let vt = c_type(v);
                    self.line(&format!("{} {} = {}.entries[{}].val;", vt, c_name(v2), base, idx));
                }
                self.block(body);
                self.indent -= 1;
                self.line("}");
            }
            _ => {
                self.warn("`For Each` needs a Vec or HashMap.");
                self.line("/* [Bust→C] For Each over a non-collection */");
            }
        }
    }

    /// A C expression yielding a `char*` for `value`, converting per its type —
    /// the counterpart of Rust's `Display` / Python's `_vb`.
    fn as_str(&mut self, e: &Expr) -> String {
        let ty = self.value_ty(e);
        if is_text(&ty) {
            // A literal is a `const char*` already; anything else is our `char*`.
            return self.expr(e);
        }
        let inner = self.expr(e);
        if is_bool(&ty) {
            self.need_from_bool = true;
            format!("vbr_from_bool({})", inner)
        } else if is_single(&ty) {
            // A `float` needs f32-precision round-tripping, or promotion to
            // `double` prints the noise digits (`3.14` → `3.140000104904175`).
            self.need_from_float = true;
            format!("vbr_from_float({})", inner)
        } else if is_float(&ty) {
            self.need_from_double = true;
            format!("vbr_from_double({})", inner)
        } else {
            self.need_from_ll = true;
            format!("vbr_from_ll({})", inner)
        }
    }

    /// A C expression for `e`'s value.
    fn expr(&mut self, e: &Expr) -> String {
        match &e.kind {
            ExprKind::Int(n) => n.to_string(),
            ExprKind::Float(f) => c_float(*f),
            ExprKind::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            ExprKind::Str(s) => c_string(s),
            // A `Match`-arm binding stands for a C expression (`x`→`_m0`).
            ExprKind::Ident(name) if self.aliases.contains_key(&name.to_ascii_lowercase()) => {
                self.aliases[&name.to_ascii_lowercase()].clone()
            }
            // `None` constructs the absent Option for the surrounding context.
            ExprKind::Ident(name) if name.eq_ignore_ascii_case("None") => {
                match self.type_hint.take() {
                    Some(ty) => format!("({}){{ .is_some = false }}", opt_name(&ty)),
                    None => {
                        self.warn("`None` needs a known Option target type.");
                        "0 /* None */".to_string()
                    }
                }
            }
            ExprKind::Ident(name) if name.eq_ignore_ascii_case("Me") => "(*self)".to_string(),
            // A constant keeps its exact casing (`MAX_RETRIES`); a plain variable
            // lowercases like any C identifier.
            ExprKind::Ident(name) => self
                .const_names
                .get(&name.to_ascii_lowercase())
                .cloned()
                .unwrap_or_else(|| c_name(name)),
            ExprKind::ConstRef(name) => self
                .const_names
                .get(&name.to_ascii_lowercase())
                .cloned()
                .unwrap_or_else(|| name.clone()),
            // `Enum.Variant` — a C-like enum value (`Suit_Hearts`) or a unit data
            // variant (constructs `(Shape){ .tag = Shape_Empty }`).
            ExprKind::Field(recv, variant)
                if matches!(&recv.kind, ExprKind::Ident(n) if self.is_enum(n)) =>
            {
                self.enum_value(recv, variant)
            }
            // `Me.field` → `self->field`; any other `recv.field` → `recv.field`.
            ExprKind::Field(recv, field) => {
                if matches!(&recv.kind, ExprKind::Ident(n) if n.eq_ignore_ascii_case("Me")) {
                    format!("self->{}", c_name(field))
                } else {
                    let r = self.expr(recv);
                    format!("{}.{}", r, c_name(field))
                }
            }
            // `Person { name: …, age: … }` → a C compound literal.
            ExprKind::StructLit { name, fields } => {
                let parts: Vec<String> = fields
                    .iter()
                    .map(|(f, v)| {
                        let val = self.expr(v);
                        format!(".{} = {}", c_name(f), val)
                    })
                    .collect();
                format!("({}){{ {} }}", name, parts.join(", "))
            }
            // `Enum.Variant(args)` — construct a data variant (tagged union).
            ExprKind::MethodCall { recv, method, args }
                if matches!(&recv.kind, ExprKind::Ident(n) if self.is_enum(n)) =>
            {
                self.enum_construct(recv, method, args)
            }
            // `recv.Method(args)` → `Struct_method(&recv, args)` — the receiver
            // goes in first, by pointer (already a pointer when it's `Me`).
            ExprKind::MethodCall { recv, method, args } => {
                let s = self.method_call(recv, method, args);
                if self.should_auto_try_method(recv, method, e) {
                    self.hoist_result(s, &self.as_result_ty(e))
                } else {
                    s
                }
            }
            ExprKind::Not(inner) => {
                let i = self.expr(inner);
                format!("(!{})", i)
            }
            ExprKind::Binary { op: BinOp::Concat, lhs, rhs } => {
                self.need_concat = true;
                let l = self.as_str(lhs);
                let r = self.as_str(rhs);
                format!("vbr_concat({}, {})", l, r)
            }
            ExprKind::Binary { op: BinOp::Pow, lhs, rhs } => {
                self.needs_math = true;
                let l = self.expr(lhs);
                let r = self.expr(rhs);
                format!("pow({}, {})", l, r)
            }
            ExprKind::Binary { op: BinOp::Mod, lhs, rhs } => {
                let float = is_float(&self.type_of(lhs)) || is_float(&self.type_of(rhs));
                let l = self.expr(lhs);
                let r = self.expr(rhs);
                if float {
                    self.needs_math = true;
                    format!("fmod({}, {})", l, r)
                } else {
                    format!("({} % {})", l, r)
                }
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let l = self.expr(lhs);
                let r = self.expr(rhs);
                format!("({} {} {})", l, bin_op(*op), r)
            }
            ExprKind::Call { name, args } => {
                let s = self.call(name, args);
                if self.should_auto_try_call(name) {
                    self.hoist_result(s, &self.as_result_ty(e))
                } else {
                    s
                }
            }
            // `expr?` — propagate a failure, hoisting the temp + early return.
            ExprKind::Try(inner) => self.hoist_try(inner),
            ExprKind::Raw(inner) => {
                let saved = self.skip_auto_try;
                self.skip_auto_try = true;
                let s = self.expr(inner);
                self.skip_auto_try = saved;
                s
            }
            // `v[i]` — a Vec/Map stores its elements in `.data`.
            ExprKind::Index(recv, idx) => {
                let r = self.expr(recv);
                let i = self.expr(idx);
                format!("{}.data[{}]", r, i)
            }
            // `[a, b, …]` — build a Vec via its `_of` constructor.
            ExprKind::List(items) => {
                let ty = self.type_of(e);
                if items.is_empty() {
                    format!("({}){{0}}", c_type(&ty))
                } else {
                    let et = match &ty {
                        DeclType::Vec(el) => c_type(el),
                        _ => "long long".to_string(),
                    };
                    let parts: Vec<String> = items.iter().map(|i| self.expr(i)).collect();
                    format!("{}_of({}, ({}[]){{ {} }})", vec_name(&ty), items.len(), et, parts.join(", "))
                }
            }
            other => {
                self.warn(format!("`{}` doesn't lower to C yet.", expr_name(other)));
                "0 /* [Bust→C] unsupported */".to_string()
            }
        }
    }

    /// Dispatch a `recv.Method(args)` on the receiver's type: a stdlib namespace
    /// call, a collection op, a struct method, or `.get(k).Unwrap()` on a map.
    fn method_call(&mut self, recv: &Expr, method: &str, args: &[Expr]) -> String {
        // A standard-library namespace call (`FileSystem.Read(...)`) → a runtime fn.
        if let ExprKind::Ident(ns) = &recv.kind {
            if let Some(call) = self.stdlib_call(ns, method, args) {
                return call;
            }
        }
        let m = method.to_ascii_lowercase();
        // `.get(k).Unwrap()` on a map → deref the found value (Option is slice 5;
        // this is the one shape used so far).
        if m == "unwrap" {
            if let ExprKind::MethodCall { recv: gr, method: gm, args: ga } = &recv.kind {
                if gm.eq_ignore_ascii_case("get") {
                    if let DeclType::Map(..) = self.type_of(gr) {
                        let n = map_name(&self.type_of(gr));
                        let base = self.expr(gr);
                        let key = self.expr(&ga[0]);
                        return format!("(*{}_get(&{}, {}))", n, base, key);
                    }
                }
            }
            // `.Unwrap()` on an Option/Result → its `_unwrap` (aborts on the
            // absent/error case, like Rust's `.unwrap()` panicking).
            let rty = self.type_of(recv);
            let v = self.expr(recv);
            return match &rty {
                DeclType::Option(_) => format!("{}_unwrap({})", opt_name(&rty), v),
                DeclType::Result(..) => format!("{}_unwrap({})", res_name(&rty), v),
                _ => v,
            };
        }
        let rty = self.value_ty(recv);
        match &rty {
            // `s.Len()` on a String → `strlen` (VB's `Len`, method form).
            DeclType::Plain(Type::Text) if matches!(m.as_str(), "len") => {
                let base = self.expr(recv);
                format!("(long long)strlen({})", base)
            }
            DeclType::Vec(_) => {
                let n = vec_name(&rty);
                let base = self.expr(recv);
                match m.as_str() {
                    "push" => {
                        let a = self.expr(&args[0]);
                        format!("{}_push(&{}, {})", n, base, a)
                    }
                    "len" | "count" => format!("{}.len", base),
                    _ => {
                        self.warn(format!("`Vec.{}` doesn't lower to C yet.", m));
                        format!("0 /* Vec.{} */", m)
                    }
                }
            }
            DeclType::Map(..) => {
                let n = map_name(&rty);
                let base = self.expr(recv);
                match m.as_str() {
                    "insert" => {
                        let k = self.expr(&args[0]);
                        let v = self.expr(&args[1]);
                        format!("{}_insert(&{}, {}, {})", n, base, k, v)
                    }
                    "contains_key" => {
                        let k = self.expr(&args[0]);
                        format!("{}_contains(&{}, {})", n, base, k)
                    }
                    "len" | "count" => format!("{}.len", base),
                    _ => {
                        self.warn(format!("`HashMap.{}` doesn't lower to C yet.", m));
                        format!("0 /* Map.{} */", m)
                    }
                }
            }
            // A stdlib value type's instance method (`d.Year()`, `child.Kill()`).
            DeclType::Named(n) if is_stdlib_type(n) => {
                let n = n.clone();
                self.stdlib_instance_call(&n, recv, method, args)
            }
            _ => self.struct_method_call(recv, method, args),
        }
    }

    fn struct_method_call(&mut self, recv: &Expr, method: &str, args: &[Expr]) -> String {
        let struct_name = match self.value_ty(recv) {
            DeclType::Named(n) => n,
            _ => {
                self.warn(format!("couldn't resolve the type of `.{}(…)` — left as-is.", method));
                "void".to_string()
            }
        };
        // `Me` is already a `Struct*`; anything else is a value we take a
        // pointer to.
        let recv_arg = if matches!(&recv.kind, ExprKind::Ident(n) if n.eq_ignore_ascii_case("Me")) {
            "self".to_string()
        } else {
            format!("&{}", self.expr(recv))
        };
        let mut rendered = vec![recv_arg];
        for a in args {
            rendered.push(self.expr(a));
        }
        format!("{}_{}({})", struct_name, c_name(method), rendered.join(", "))
    }

    fn is_enum(&self, name: &str) -> bool {
        self.enums.contains_key(&name.to_ascii_lowercase())
    }

    /// The canonical `Enum_Variant` tag constant + the variant's payload types.
    fn variant_canon(&self, enum_key: &str, variant: &str) -> (String, String, Vec<DeclType>) {
        if let Some(info) = self.enums.get(enum_key) {
            if let Some((v, pl)) = info.variants.iter().find(|(v, _)| v.eq_ignore_ascii_case(variant)) {
                return (info.name.clone(), v.clone(), pl.clone());
            }
            return (info.name.clone(), variant.to_string(), Vec::new());
        }
        (enum_key.to_string(), variant.to_string(), Vec::new())
    }

    /// `Enum.Variant` in value position: a C-like enum constant, or a unit data
    /// variant constructed as a tagged union.
    fn enum_value(&mut self, recv: &Expr, variant: &str) -> String {
        let key = match &recv.kind {
            ExprKind::Ident(n) => n.to_ascii_lowercase(),
            _ => unreachable!(),
        };
        let is_data = self.enums.get(&key).map(|e| e.is_data).unwrap_or(false);
        let (ename, cvariant, _) = self.variant_canon(&key, variant);
        if is_data {
            format!("({}){{ .tag = {}_{} }}", ename, ename, cvariant)
        } else {
            format!("{}_{}", ename, cvariant)
        }
    }

    /// `Enum.Variant(args)` → a tagged-union compound literal.
    fn enum_construct(&mut self, recv: &Expr, variant: &str, args: &[Expr]) -> String {
        let key = match &recv.kind {
            ExprKind::Ident(n) => n.to_ascii_lowercase(),
            _ => unreachable!(),
        };
        let (ename, cvariant, _) = self.variant_canon(&key, variant);
        let a: Vec<String> = args.iter().map(|x| self.expr(x)).collect();
        if a.is_empty() {
            format!("({}){{ .tag = {}_{} }}", ename, ename, cvariant)
        } else {
            format!(
                "({}){{ .tag = {}_{}, .data.{} = {{ {} }} }}",
                ename,
                ename,
                cvariant,
                cvariant,
                a.join(", ")
            )
        }
    }

    /// `Match <scrutinee> … End Match` → a scrutinee temp + an if/else-if chain.
    /// (C `switch` can't do ranges/guards/bindings, so one uniform lowering.)
    fn match_stmt(&mut self, scrutinee: &Expr, arms: &[MatchArm]) {
        let temp = format!("_m{}", self.match_counter);
        self.match_counter += 1;
        let sty = self.type_of(scrutinee);
        let cty = c_type(&sty);
        let sval = self.expr(scrutinee);
        self.line(&format!("{} {} = {};", cty, temp, sval));

        // Resolve each arm to a condition + bindings up front (borrows `&self`).
        let plans: Vec<ArmPlan> = arms
            .iter()
            .map(|a| self.resolve_arm(&pattern::parse(&a.pattern), &temp, &sty))
            .collect();

        let last = arms.len().saturating_sub(1);
        // An arm with no condition (wildcard / bare binding) already catches all.
        let has_catch_all = arms.iter().zip(&plans).any(|(a, p)| a.guard.is_none() && p.cond.is_none());

        for (i, (arm, plan)) in arms.iter().zip(&plans).enumerate() {
            // A bare `x` binding aliases the scrutinee (so a guard can see it).
            let alias = plan.alias.clone();
            if let Some(x) = &alias {
                self.aliases.insert(x.clone(), temp.clone());
            }

            // The unconditional `else`: no condition, or the last guardless arm
            // of an exhaustive match (so C sees the chain as total).
            let is_closer = arm.guard.is_none() && (plan.cond.is_none() || (i == last && !has_catch_all));
            let cond = if is_closer { None } else { plan.cond.clone() };
            let guard = arm.guard.as_ref().map(|g| self.expr(g));
            let effective = match (cond, guard) {
                (Some(c), Some(g)) => Some(format!("{} && {}", c, g)),
                (Some(c), None) => Some(c),
                (None, Some(g)) => Some(g),
                (None, None) => None,
            };
            let header = match (i, &effective) {
                (0, Some(c)) => format!("if ({}) {{", c),
                (0, None) => "if (1) {".to_string(),
                (_, Some(c)) => format!("}} else if ({}) {{", c),
                (_, None) => "} else {".to_string(),
            };
            self.line(&header);
            self.indent += 1;
            for (lty, lname, lexpr) in &plan.locals {
                self.line(&format!("{} {} = {};", lty, lname, lexpr));
            }
            self.block(&arm.body);
            self.indent -= 1;
            if let Some(x) = &alias {
                self.aliases.remove(x);
            }
        }
        self.line("}");
    }

    /// Resolve one arm against the scrutinee type: its C condition, the locals to
    /// declare in its block, and (for a bare `x`) the scrutinee alias.
    fn resolve_arm(&self, pat: &Pat, temp: &str, scrut_ty: &DeclType) -> ArmPlan {
        let mut plan = ArmPlan::default();
        match (scrut_ty, pat) {
            (DeclType::Option(t), Pat::Some(inner)) => {
                plan.cond = Some(format!("{}.is_some", temp));
                self.bind_inner(inner, &format!("{}.value", temp), t, &mut plan);
            }
            (DeclType::Option(_), Pat::None) => plan.cond = Some(format!("!{}.is_some", temp)),
            (DeclType::Result(t, _), Pat::Ok(inner)) => {
                plan.cond = Some(format!("{}.is_ok", temp));
                self.bind_inner(inner, &format!("{}.ok", temp), t, &mut plan);
            }
            (DeclType::Result(_, e), Pat::Err(inner)) => {
                let mut cond = format!("!{}.is_ok", temp);
                let base = format!("{}.err", temp);
                match &**inner {
                    Pat::Binding(x) => plan.locals.push((c_type(e), c_name(x), base)),
                    Pat::EnumTag { enom, variant } => {
                        cond = format!("{} && {}.tag == {}", cond, base, self.tag_const(enom, variant));
                    }
                    Pat::Variant { enom, variant, binds } => {
                        cond = format!("{} && {}.tag == {}", cond, base, self.tag_const(enom, variant));
                        let (_, cv, payloads) = self.variant_canon(&enom.to_ascii_lowercase(), variant);
                        for (idx, b) in binds.iter().enumerate() {
                            let pty = payloads.get(idx).map(c_type).unwrap_or_else(|| "long long".to_string());
                            plan.locals.push((pty, c_name(b), format!("{}.data.{}.f{}", base, cv, idx)));
                        }
                    }
                    _ => {} // `Err(_)`
                }
                plan.cond = Some(cond);
            }
            _ => self.resolve_plain(pat, temp, scrut_ty, &mut plan),
        }
        plan
    }

    /// The slice-3 patterns (plain enum / integer scrutinee).
    fn resolve_plain(&self, pat: &Pat, temp: &str, scrut_ty: &DeclType, plan: &mut ArmPlan) {
        let is_data = match scrut_ty {
            DeclType::Named(n) => self.enums.get(&n.to_ascii_lowercase()).map(|e| e.is_data).unwrap_or(false),
            _ => false,
        };
        match pat {
            Pat::Wildcard => {}
            Pat::Binding(x) => plan.alias = Some(x.to_ascii_lowercase()),
            Pat::Variant { enom, variant, binds } => {
                plan.cond = Some(format!("{}.tag == {}", temp, self.tag_const(enom, variant)));
                let (_, cv, payloads) = self.variant_canon(&enom.to_ascii_lowercase(), variant);
                for (idx, b) in binds.iter().enumerate() {
                    let pty = payloads.get(idx).map(c_type).unwrap_or_else(|| "long long".to_string());
                    plan.locals.push((pty, c_name(b), format!("{}.data.{}.f{}", temp, cv, idx)));
                }
            }
            _ => plan.cond = Some(self.condition(pat, temp, is_data)),
        }
    }

    /// Bind the inner pattern of a `Some(_)`/`Ok(_)` — a name reads the payload,
    /// `_` reads nothing.
    fn bind_inner(&self, inner: &Pat, access: &str, ty: &DeclType, plan: &mut ArmPlan) {
        if let Pat::Binding(x) = inner {
            plan.locals.push((c_type(ty), c_name(x), access.to_string()));
        }
    }

    /// The C boolean condition testing whether `temp` matches `pat`.
    fn condition(&self, pat: &Pat, temp: &str, is_data: bool) -> String {
        match pat {
            Pat::Int(n) => format!("{} == {}", temp, n),
            Pat::Bool(b) => format!("{} == {}", temp, if *b { "true" } else { "false" }),
            Pat::Range { lo, hi, inclusive } => {
                let op = if *inclusive { "<=" } else { "<" };
                format!("{} >= {} && {} {} {}", temp, lo, temp, op, hi)
            }
            Pat::Alt(subs) => {
                let parts: Vec<String> =
                    subs.iter().map(|p| format!("({})", self.condition(p, temp, is_data))).collect();
                format!("({})", parts.join(" || "))
            }
            Pat::EnumTag { enom, variant } => {
                let tag = self.tag_const(enom, variant);
                if is_data {
                    format!("{}.tag == {}", temp, tag)
                } else {
                    format!("{} == {}", temp, tag)
                }
            }
            Pat::Variant { enom, variant, .. } => {
                format!("{}.tag == {}", temp, self.tag_const(enom, variant))
            }
            Pat::Wildcard | Pat::Binding(_) => "1".to_string(),
            // Option/Result patterns are resolved against the scrutinee type in
            // `resolve_arm`, never here.
            Pat::Some(_) | Pat::None | Pat::Ok(_) | Pat::Err(_) => "1".to_string(),
            Pat::Other(s) => format!("0 /* unsupported pattern: {} */", s),
        }
    }

    fn tag_const(&self, enom: &str, variant: &str) -> String {
        let (ename, cvariant, _) = self.variant_canon(&enom.to_ascii_lowercase(), variant);
        format!("{}_{}", ename, cvariant)
    }

    /// A standard-library namespace call → its C runtime function (registered for
    /// emission), or `None` if `ns.method` isn't a known stdlib call.
    fn stdlib_call(&mut self, ns: &str, method: &str, args: &[Expr]) -> Option<String> {
        let method = method.to_ascii_lowercase().replace('_', "");
        let fname = match (ns.to_ascii_lowercase().as_str(), method.as_str()) {
            ("filesystem", "read") => self.use_stdlib("fs_read", true),
            ("filesystem", "write") => self.use_stdlib("fs_write", true),
            ("filesystem", "delete") => self.use_stdlib("fs_delete", true),
            ("filesystem", "exists") => self.use_stdlib("fs_exists", false),
            ("datetime", "parse") => self.use_ns("vbr_datetime_parse", "datetime"),
            ("datetime", "now") => self.use_ns("vbr_datetime_now", "datetime"),
            ("regex", "replaceall") => self.use_ns("vbr_regex_replaceall", "regex"),
            ("regex", "replace") => self.use_ns("vbr_regex_replace", "regex"),
            ("shell", "run") => self.use_ns("vbr_shell_run", "shell"),
            ("shell", "start") => self.use_ns("vbr_shell_start", "shell"),
            ("json", "parse") => self.use_ns("vbr_json_parse", "json"),
            ("json", "object") => self.use_ns("vbr_json_object", "json"),
            ("json", "array") => self.use_ns("vbr_json_array", "json"),
            ("database", "open") => self.use_ns("vbr_db_open", "database"),
            ("http", "get") => self.use_ns("vbr_http_get", "http"),
            ("http", "post") => self.use_ns("vbr_http_post", "http"),
            _ => return None,
        };
        let a: Vec<String> = args.iter().map(|x| self.expr(x)).collect();
        Some(format!("{}({})", fname, a.join(", ")))
    }

    /// Mark a whole-namespace runtime (`datetime`/`shell`/`regex`, emitted as one
    /// block) as needed and return the C function name.
    fn use_ns(&mut self, fname: &str, ns: &str) -> String {
        match ns {
            "datetime" => self.needs_datetime = true,
            "shell" => self.needs_shell = true,
            "regex" => self.needs_regex = true,
            "json" => self.needs_json = true,
            "database" => {
                self.needs_database = true;
                self.needs_json = true;
            }
            "http" => self.needs_http = true,
            _ => {}
        }
        fname.to_string()
    }

    /// A method on a stdlib value type (`d.Year()`, `child.Kill()`) → its runtime
    /// function, receiver passed by pointer.
    fn stdlib_instance_call(&mut self, ty: &str, recv: &Expr, method: &str, args: &[Expr]) -> String {
        let t = ty.to_ascii_lowercase();
        match t.as_str() {
            "datetime" => self.needs_datetime = true,
            "process" => self.needs_shell = true,
            "json" => self.needs_json = true,
            "database" => {
                self.needs_database = true;
                self.needs_json = true;
                return self.database_call(recv, &method.to_ascii_lowercase().replace('_', ""), args);
            }
            _ => {}
        }
        let recv_c = self.expr(recv);
        let mut all = vec![format!("&{}", recv_c)];
        for a in args {
            all.push(self.expr(a));
        }
        let method = method.to_ascii_lowercase().replace('_', "");
        format!("vbr_{}_{}({})", t, method, all.join(", "))
    }

    /// A `Database` connection method. `Execute`/`Query` take an SQL string and a
    /// `Vec<String>` of params — an empty `[]` argument is forced to `Vec_str`
    /// (its default inference is `Vec<Long>`, which would mistype the runtime).
    fn database_call(&mut self, recv: &Expr, method: &str, args: &[Expr]) -> String {
        let recv_c = self.expr(recv);
        match method {
            "execute" | "query" => {
                let sql = self.expr(&args[0]);
                let params = self.db_params(&args[1]);
                format!("vbr_db_{}(&{}, {}, {})", method, recv_c, sql, params)
            }
            _ => format!("vbr_db_{}(&{})", method, recv_c),
        }
    }

    /// The params argument of a `Database` call as a `Vec_str` — an empty list
    /// literal becomes `(Vec_str){0}` (typed, unlike the default `Vec<Long>`).
    fn db_params(&mut self, arg: &Expr) -> String {
        if let ExprKind::List(items) = &arg.kind {
            if items.is_empty() {
                return "(Vec_str){0}".to_string();
            }
        }
        self.expr(arg)
    }

    /// Register a stdlib runtime function for emission and return its C name.
    fn use_stdlib(&mut self, fname: &'static str, needs_dup: bool) -> String {
        self.stdlib_fns.insert(fname);
        if fname.starts_with("fs_") {
            self.needs_fs = true;
        }
        if needs_dup {
            self.need_dup = true;
        }
        format!("vbr_{}", fname)
    }

    fn call(&mut self, name: &str, args: &[Expr]) -> String {
        // `Some`/`Ok`/`Err` construct an Option/Result compound literal, using
        // the surrounding context (`type_hint`) for the target type.
        if matches!(name, "Some" | "Ok" | "Err") {
            return self.construct(name, args);
        }
        // `CStr(x)`/`Str(x)` — a number → its string, the same conversion the
        // `&`-concat machinery uses (`vbr_from_ll`/`vbr_from_double`/…).
        if (name.eq_ignore_ascii_case("cstr") || name.eq_ignore_ascii_case("str")) && args.len() == 1 {
            return self.as_str(&args[0]);
        }
        // `Sleep <ms>` — VB6's kernel32 Sleep (milliseconds) → POSIX `usleep`.
        if name.eq_ignore_ascii_case("Sleep") {
            self.needs_unistd = true;
            let a: Vec<String> = args.iter().map(|x| self.expr(x)).collect();
            return format!("usleep(({}) * 1000)", a.join(", "));
        }
        if name.eq_ignore_ascii_case("rnd") && args.is_empty() {
            self.needs_rnd = true;
            return "rnd()".into();
        }
        // Lazy like Rust's `if`/`else` (C's ternary does not evaluate the unused arm).
        if name.eq_ignore_ascii_case("iif") && args.len() == 3 {
            let c = self.expr(&args[0]);
            let t = self.expr(&args[1]);
            let e = self.expr(&args[2]);
            return format!("(({}) ? ({}) : ({}))", c, t, e);
        }
        if name.eq_ignore_ascii_case("round") && args.len() == 2 {
            self.needs_math = true;
            self.needs_round_places = true;
            let x = self.expr(&args[0]);
            let p = self.expr(&args[1]);
            return format!("vbr_round_places({}, {})", x, p);
        }
        if let Some(s) = self.lower_c_builtin(name, args) {
            return s;
        }
        let rendered: Vec<String> = args.iter().map(|a| self.expr(a)).collect();
        // Maths builtins → the C standard library (all in `<math.h>`).
        if let Some(cfn) = c_math_builtin(name) {
            self.needs_math = true;
            return format!("{}({})", cfn, rendered.join(", "));
        }
        format!("{}({})", c_name(name), rendered.join(", "))
    }

    /// String / conversion builtins. Character counts (UTF-8), matching Rust's
    /// `chars()` — not `strlen` bytes.
    fn lower_c_builtin(&mut self, name: &str, args: &[Expr]) -> Option<String> {
        let n = name.to_ascii_lowercase();
        let a = |i: usize, e: &mut Emitter| e.expr(&args[i]);
        Some(match (n.as_str(), args.len()) {
            ("len", 1) => {
                self.needs_strings = true;
                format!("vbr_len({})", a(0, self))
            }
            ("ucase", 1) => {
                self.needs_strings = true;
                format!("vbr_ucase({})", a(0, self))
            }
            ("lcase", 1) => {
                self.needs_strings = true;
                format!("vbr_lcase({})", a(0, self))
            }
            ("trim", 1) => {
                self.needs_strings = true;
                format!("vbr_trim({})", a(0, self))
            }
            ("chr", 1) => {
                self.needs_strings = true;
                format!("vbr_chr({})", a(0, self))
            }
            ("asc", 1) => {
                self.needs_strings = true;
                format!("vbr_asc({})", a(0, self))
            }
            ("left", 2) => {
                self.needs_strings = true;
                format!("vbr_left({}, {})", a(0, self), a(1, self))
            }
            ("right", 2) => {
                self.needs_strings = true;
                format!("vbr_right({}, {})", a(0, self), a(1, self))
            }
            ("mid", 2) => {
                self.needs_strings = true;
                format!("vbr_mid({}, {})", a(0, self), a(1, self))
            }
            ("mid", 3) => {
                self.needs_strings = true;
                format!("vbr_mid_n({}, {}, {})", a(0, self), a(1, self), a(2, self))
            }
            ("replace", 3) => {
                self.needs_strings = true;
                format!("vbr_replace({}, {}, {})", a(0, self), a(1, self), a(2, self))
            }
            ("instr", 2) => {
                self.needs_strings = true;
                self.needs_instr = true;
                format!("vbr_instr({}, {})", a(0, self), a(1, self))
            }
            ("val", 1) => {
                self.needs_val = true;
                format!("vbr_val({})", a(0, self))
            }
            ("cdbl", 1) => {
                self.needs_cdbl = true;
                format!("vbr_cdbl({})", a(0, self))
            }
            ("clng", 1) => {
                self.needs_clng = true;
                format!("vbr_clng({})", a(0, self))
            }
            ("cint", 1) => {
                self.needs_cint = true;
                format!("vbr_cint({})", a(0, self))
            }
            ("inputbox", 1) => {
                self.needs_input = true;
                format!("vbr_input_box({})", a(0, self))
            }
            ("split", 1) => {
                self.needs_split = true;
                format!("vbr_split({}, \" \")", a(0, self))
            }
            ("split", 2) => {
                self.needs_split = true;
                format!("vbr_split({}, {})", a(0, self), a(1, self))
            }
            ("join", 1) => {
                self.needs_join = true;
                format!("vbr_join({}, \" \")", a(0, self))
            }
            ("join", 2) => {
                self.needs_join = true;
                format!("vbr_join({}, {})", a(0, self), a(1, self))
            }
            ("space", 1) => {
                self.needs_space = true;
                format!("vbr_space({})", a(0, self))
            }
            ("format", 2) => {
                let ExprKind::Str(pat) = &args[1].kind else {
                    return None;
                };
                let parsed = crate::fmtpat::FormatPat::parse(pat)?;
                let formatted = if parsed.is_bare() {
                    self.as_str(&args[0])
                } else if is_float(&self.type_of(&args[0])) {
                    let spec = parsed.printf_spec(true)?;
                    self.needs_fmt_double = true;
                    format!("vbr_fmt_double({}, {})", a(0, self), c_string(&spec))
                } else {
                    let spec = parsed.printf_spec(false)?;
                    self.needs_fmt_ll = true;
                    format!("vbr_fmt_ll({}, {})", a(0, self), c_string(&spec))
                };
                let mut s = formatted;
                if !parsed.prefix.is_empty() {
                    self.need_concat = true;
                    s = format!("vbr_concat({}, {})", c_string(&parsed.prefix), s);
                }
                if !parsed.suffix.is_empty() {
                    self.need_concat = true;
                    s = format!("vbr_concat({}, {})", s, c_string(&parsed.suffix));
                }
                s
            }
            _ => return None,
        })
    }

    /// `expr?` — bind the Option/Result to a temp on a preceding line, return
    /// early on failure (re-wrapped into the function's own return type), and
    /// yield the unwrapped success value.
    fn hoist_try(&mut self, inner: &Expr) -> String {
        let ity = self.type_of(inner);
        let val = self.expr(inner);
        let tmp = format!("_t{}", self.tmp_counter);
        self.tmp_counter += 1;
        let cty = c_type(&ity);
        self.line(&format!("{} {} = {};", cty, tmp, val));
        if let DeclType::Option(_) = ity {
            let ret = self.propagate_none();
            self.line(&format!("if (!{}.is_some) return {};", tmp, ret));
            format!("{}.value", tmp)
        } else {
            let ret = self.propagate_err(&tmp);
            self.line(&format!("if (!{}.is_ok) return {};", tmp, ret));
            // A unit `Result<()>?` has no `.ok` field — it yields nothing (used
            // only as a statement), so give a valid no-op expression.
            if let DeclType::Result(t, _) = &ity {
                if is_unit(t) {
                    return "(void)0".to_string();
                }
            }
            format!("{}.ok", tmp)
        }
    }

    fn as_result_ty(&self, e: &Expr) -> DeclType {
        match &e.kind {
            ExprKind::Call { name, .. } => {
                let err = || Box::new(DeclType::Plain(Type::Text));
                match name.to_ascii_lowercase().as_str() {
                    "cdbl" => {
                        return DeclType::Result(Box::new(DeclType::Plain(Type::Double)), err())
                    }
                    "clng" => {
                        return DeclType::Result(Box::new(DeclType::Plain(Type::Long)), err())
                    }
                    "cint" => {
                        return DeclType::Result(Box::new(DeclType::Plain(Type::Integer)), err())
                    }
                    "inputbox" => {
                        return DeclType::Result(Box::new(DeclType::Plain(Type::Text)), err())
                    }
                    _ => {}
                }
                if let Some(ret) = self.user_fn_ret.get(&c_name(name)) {
                    return result_of(ret.as_ref());
                }
            }
            ExprKind::MethodCall { method, .. } => {
                if let Some(ret) = self.user_fn_ret.get(&c_name(method)) {
                    return result_of(ret.as_ref());
                }
            }
            _ => {}
        }
        let ty = self.type_of(e);
        if matches!(ty, DeclType::Result(..)) {
            ty
        } else {
            result_of(Some(&ty))
        }
    }

    fn hoist_result(&mut self, val: String, ity: &DeclType) -> String {
        let tmp = format!("_t{}", self.tmp_counter);
        self.tmp_counter += 1;
        let cty = c_type(ity);
        self.line(&format!("{} {} = {};", cty, tmp, val));
        if self.emitting_main {
            self.line(&format!(
                "if (!{}.is_ok) {{ fprintf(stderr, \"Error: %s\\n\", {}.err); return 1; }}",
                tmp, tmp
            ));
        } else {
            let ret = self.propagate_err(&tmp);
            self.line(&format!("if (!{}.is_ok) return {};", tmp, ret));
        }
        if let DeclType::Result(t, _) = ity {
            if is_unit(t) {
                return "(void)0".to_string();
            }
        }
        format!("{}.ok", tmp)
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
        self.user_fns.contains(&c_name(name))
    }

    fn should_auto_try_method(&self, _recv: &Expr, method: &str, whole: &Expr) -> bool {
        if self.skip_auto_try {
            return false;
        }
        if matches!(self.type_of(whole), DeclType::Result(..)) {
            return true;
        }
        self.user_methods.contains(&c_name(method))
    }

    fn ok_literal(&self, ty: &DeclType, value: Option<String>) -> String {
        let n = res_name(ty);
        if let DeclType::Result(t, _) = ty {
            if is_unit(t) {
                return format!("({}){{ .is_ok = true }}", n);
            }
        }
        let v = value.unwrap_or_else(|| "0".to_string());
        format!("({}){{ .is_ok = true, .ok = {} }}", n, v)
    }

    fn err_literal(&self, ty: &DeclType, msg: &str) -> String {
        format!("({}){{ .is_ok = false, .err = {} }}", res_name(ty), msg)
    }

    fn emit_handle(
        &mut self,
        target: Option<&Expr>,
        call: &Expr,
        err_name: &str,
        body: &[Stmt],
    ) {
        let saved = self.skip_auto_try;
        self.skip_auto_try = true;
        let val = self.expr(call);
        self.skip_auto_try = saved;
        let ity = self.as_result_ty(call);
        let tmp = format!("_t{}", self.tmp_counter);
        self.tmp_counter += 1;
        let err = c_name(err_name);
        self.err_names.insert(err.clone());
        self.line(&format!("{} {} = {};", c_type(&ity), tmp, val));
        match target {
            None => {
                self.line(&format!("if (!{}.is_ok) {{", tmp));
                self.indent += 1;
                self.line(&format!("char* {} = {}.err;", err, tmp));
                self.block(body);
                self.indent -= 1;
                self.line("}");
            }
            Some(tgt) => {
                let lhs = self.expr(tgt);
                self.line(&format!("if (!{}.is_ok) {{", tmp));
                self.indent += 1;
                self.line(&format!("char* {} = {}.err;", err, tmp));
                if c_body_diverges(body) {
                    self.block(body);
                    self.indent -= 1;
                    self.line("} else {");
                    self.indent += 1;
                    self.line(&format!("{} = {}.ok;", lhs, tmp));
                    self.indent -= 1;
                    self.line("}");
                } else if let Some(Stmt::Expr(e)) = body
                    .iter()
                    .rev()
                    .find(|s| !matches!(s, Stmt::Comment(_) | Stmt::LineMark(_)))
                {
                    let last_i = body.iter().rposition(|s| matches!(s, Stmt::Expr(_))).unwrap();
                    self.block(&body[..last_i]);
                    let repl = self.expr(e);
                    self.line(&format!("{} = {};", lhs, repl));
                    self.indent -= 1;
                    self.line("} else {");
                    self.indent += 1;
                    self.line(&format!("{} = {}.ok;", lhs, tmp));
                    self.indent -= 1;
                    self.line("}");
                } else {
                    self.block(body);
                    self.indent -= 1;
                    self.line("} else {");
                    self.indent += 1;
                    self.line(&format!("{} = {}.ok;", lhs, tmp));
                    self.indent -= 1;
                    self.line("}");
                }
            }
        }
    }

    fn propagate_none(&mut self) -> String {
        match self.current_ret.clone() {
            Some(ty @ DeclType::Option(_)) => format!("({}){{ .is_some = false }}", opt_name(&ty)),
            _ => {
                self.warn("`?` used in a function that doesn't return an Option.");
                "0".to_string()
            }
        }
    }

    fn propagate_err(&mut self, tmp: &str) -> String {
        match self.current_ret.clone() {
            Some(ty @ DeclType::Result(..)) => {
                format!("({}){{ .is_ok = false, .err = {}.err }}", res_name(&ty), tmp)
            }
            _ => {
                self.warn("`?` used in a function that doesn't return a Result.");
                "0".to_string()
            }
        }
    }

    /// Build a `Some`/`Ok`/`Err` compound literal for the current `type_hint`.
    fn construct(&mut self, name: &str, args: &[Expr]) -> String {
        // Take the hint so the payload is evaluated context-free.
        let hint = self.type_hint.take();
        match (name, hint) {
            ("Some", Some(ty)) => {
                let n = opt_name(&ty);
                let v = self.expr(&args[0]);
                format!("({}){{ .is_some = true, .value = {} }}", n, v)
            }
            ("Ok", Some(ty)) => {
                let n = res_name(&ty);
                let unit = args.is_empty()
                    || matches!(args.first().map(|a| &a.kind), Some(ExprKind::Tuple(t)) if t.is_empty());
                if unit {
                    format!("({}){{ .is_ok = true }}", n)
                } else {
                    let v = self.expr(&args[0]);
                    format!("({}){{ .is_ok = true, .ok = {} }}", n, v)
                }
            }
            ("Err", Some(ty)) => {
                let n = res_name(&ty);
                let v = self.expr(&args[0]);
                format!("({}){{ .is_ok = false, .err = {} }}", n, v)
            }
            _ => {
                self.warn(format!("`{}(…)` needs a known Option/Result target type.", name));
                let a: Vec<String> = args.iter().map(|x| self.expr(x)).collect();
                format!("{}({})", name, a.join(", "))
            }
        }
    }

    fn type_of(&self, e: &Expr) -> DeclType {
        if let ExprKind::Ident(n) = &e.kind {
            if self.err_names.contains(&c_name(n)) {
                return DeclType::Plain(Type::Text);
            }
        }
        self.types.get(&e.span).cloned().unwrap_or(DeclType::Plain(Type::Long))
    }

    /// The type after implicit `?` — a `Result<T, E>` call yields `T`.
    fn value_ty(&self, e: &Expr) -> DeclType {
        self.value_ty_of(e, self.type_of(e))
    }

    fn value_ty_of(&self, e: &Expr, ty: DeclType) -> DeclType {
        if self.skip_auto_try {
            return ty;
        }
        if matches!(&e.kind, ExprKind::Raw(_)) {
            return ty;
        }
        match ty {
            DeclType::Result(t, _) => *t,
            other => other,
        }
    }

    fn finish(self, program: &Program) -> CProgram {
        let mut code = String::new();

        // Leading `'` comments carry over as `//`.
        for c in &program.leading_comments {
            code.push_str(&format!("// {}\n", c.trim_start_matches(['\'', ' '])));
        }
        if !program.leading_comments.is_empty() {
            code.push('\n');
        }

        // `strptime`/`timegm` (and later `popen`) are POSIX/GNU extensions —
        // the feature-test macro must precede every system header.
        if self.needs_datetime || self.needs_shell || self.needs_regex {
            code.push_str("#define _GNU_SOURCE\n");
        }
        code.push_str("#include <stdio.h>\n");
        code.push_str("#include <stdlib.h>\n");
        code.push_str("#include <string.h>\n");
        code.push_str("#include <stdbool.h>\n");
        if self.needs_math {
            code.push_str("#include <math.h>\n");
        }
        if self.needs_fs || self.needs_shell {
            code.push_str("#include <errno.h>\n");
        }
        if self.needs_fs {
            code.push_str("#include <sys/stat.h>\n");
        }
        if self.needs_datetime || self.needs_rnd {
            code.push_str("#include <time.h>\n");
        }
        if self.needs_shell {
            code.push_str("#include <sys/wait.h>\n");
            code.push_str("#include <signal.h>\n");
        }
        if self.needs_shell || self.needs_unistd {
            code.push_str("#include <unistd.h>\n");
        }
        if self.needs_regex {
            code.push_str("#include <regex.h>\n");
        }
        // SQLite — a system header (linked `-lsqlite3`).
        if self.needs_database {
            code.push_str("#include <sqlite3.h>\n");
        }
        // libcurl — a system header (linked `-lcurl`).
        if self.needs_http {
            code.push_str("#include <curl/curl.h>\n");
        }
        // Vendored cJSON — a project-local header (bundled beside `main.c`).
        if self.needs_json {
            code.push_str("#include \"cJSON.h\"\n");
        }
        code.push('\n');

        // Struct `typedef`s + module constants come before the functions that
        // use them (and before the prototypes, which name struct types).
        code.push_str(&self.decls);

        // The inlined runtime — only the helpers the body used, in dependency
        // order (`vbr_from_*` build on `vbr_dup`).
        let need_dup = self.need_dup
            || self.need_from_bool
            || self.need_from_double
            || self.need_from_float
            || self.needs_datetime
            || self.needs_regex
            || self.needs_shell
            || self.needs_json
            || self.needs_database
            || self.needs_http
            || self.needs_strings
            || self.needs_val
            || self.needs_cdbl
            || self.needs_clng
            || self.needs_cint
            || self.needs_input
            || self.needs_split
            || self.needs_join
            || self.needs_space
            || self.needs_fmt_double
            || self.needs_fmt_ll;
        if need_dup {
            code.push_str(RT_DUP);
        }
        if self.need_from_ll {
            code.push_str(RT_FROM_LL);
        }
        if self.need_from_bool {
            code.push_str(RT_FROM_BOOL);
        }
        if self.need_from_double {
            code.push_str(RT_FROM_DOUBLE);
        }
        if self.need_from_float {
            code.push_str(RT_FROM_FLOAT);
        }
        if self.need_concat {
            code.push_str(RT_CONCAT);
        }
        if self.needs_rnd {
            code.push_str(RT_RND);
        }
        if self.needs_strings {
            code.push_str(RT_STRINGS);
        }
        if self.needs_instr {
            code.push_str(RT_INSTR);
        }
        if self.needs_val {
            code.push_str(RT_VAL);
        }
        if self.needs_cdbl {
            code.push_str(RT_CDBL);
        }
        if self.needs_clng {
            code.push_str(RT_CLNG);
        }
        if self.needs_cint {
            code.push_str(RT_CINT);
        }
        if self.needs_input {
            code.push_str(RT_INPUT);
        }
        if self.needs_round_places {
            code.push_str(RT_ROUND_PLACES);
        }
        if self.needs_split {
            code.push_str(RT_SPLIT);
        }
        if self.needs_join {
            code.push_str(RT_JOIN);
        }
        if self.needs_space {
            code.push_str(RT_SPACE);
        }
        if self.needs_fmt_double {
            code.push_str(RT_FMT_DOUBLE);
        }
        if self.needs_fmt_ll {
            code.push_str(RT_FMT_LL);
        }
        if need_dup
            || self.need_from_ll
            || self.need_from_bool
            || self.need_from_double
            || self.need_from_float
            || self.need_concat
            || self.needs_rnd
            || self.needs_strings
            || self.needs_instr
            || self.needs_val
            || self.needs_cdbl
            || self.needs_clng
            || self.needs_cint
            || self.needs_input
            || self.needs_round_places
            || self.needs_split
            || self.needs_join
            || self.needs_space
            || self.needs_fmt_double
            || self.needs_fmt_ll
        {
            code.push('\n');
        }

        // Standard-library runtime — after the string helpers (`vbr_dup`, …) and
        // the `Result`/type definitions they build on. Per-function (file I/O)
        // and whole-namespace (DateTime, …) blocks.
        for f in &self.stdlib_fns {
            code.push_str(stdlib_helper(f));
        }
        if self.needs_datetime {
            code.push_str(RT_DATETIME);
        }
        if self.needs_regex {
            code.push_str(RT_REGEX);
        }
        if self.needs_shell {
            code.push_str(RT_SHELL);
        }
        if self.needs_json {
            code.push_str(RT_JSON);
        }
        if self.needs_database {
            code.push_str(RT_DATABASE);
        }
        if self.needs_http {
            code.push_str(RT_HTTP);
        }
        if !self.stdlib_fns.is_empty()
            || self.needs_datetime
            || self.needs_regex
            || self.needs_shell
            || self.needs_json
            || self.needs_database
            || self.needs_http
        {
            code.push('\n');
        }

        // Prototypes for every non-`main` function, so call order is free.
        let mut protos = String::new();
        for func in &program.functions {
            let is_main = func.receiver.is_none() && func.name.eq_ignore_ascii_case("main");
            if !is_main {
                protos.push_str(&format!("{};\n", self.signature(func, false)));
            }
        }
        if !protos.is_empty() {
            code.push_str(&protos);
            code.push('\n');
        }

        let prefix_lines = code.matches('\n').count();
        code.push_str(&self.out);
        let line_map = self
            .line_map
            .into_iter()
            .map(|(c, v)| (c + prefix_lines, v))
            .collect();

        // Vendored/linked dependencies → a project folder + `Makefile`. `libm`
        // (`-lm`) is a link flag too when the maths builtins are used.
        let mut vendored = Vec::new();
        let mut link_flags = Vec::new();
        if self.needs_math {
            link_flags.push("m".to_string());
        }
        if self.needs_json {
            vendored.push("cJSON".to_string());
        }
        if self.needs_database {
            link_flags.push("sqlite3".to_string());
        }
        if self.needs_http {
            link_flags.push("curl".to_string());
        }

        CProgram {
            code,
            warnings: self.warnings,
            vendored,
            link_flags,
            line_map,
        }
    }
}

/// The C source of a standard-library runtime function, by its registered name.
fn stdlib_helper(name: &str) -> &'static str {
    match name {
        "fs_read" => RT_FS_READ,
        "fs_write" => RT_FS_WRITE,
        "fs_delete" => RT_FS_DELETE,
        "fs_exists" => RT_FS_EXISTS,
        _ => "",
    }
}

// ---- runtime helpers (inlined into the output, single-file slice) ----

const RT_DUP: &str = "\
static char* vbr_dup(const char* s) {
    char* d = (char*)malloc(strlen(s) + 1);
    strcpy(d, s);
    return d;
}
";

const RT_FROM_LL: &str = "\
static char* vbr_from_ll(long long v) {
    char* s = (char*)malloc(32);
    snprintf(s, 32, \"%lld\", v);
    return s;
}
";

const RT_FROM_BOOL: &str = "\
static char* vbr_from_bool(bool b) {
    return vbr_dup(b ? \"true\" : \"false\");
}
";

// Rust's `Display` for `f64`: the shortest decimal that round-trips, with no
// trailing `.0`. We reproduce it by trying increasing `%g` precision until the
// text re-parses to the same bits (the classic shortest-round-trip trick).
const RT_FROM_DOUBLE: &str = "\
static char* vbr_from_double(double d) {
    char buf[64];
    for (int p = 1; p <= 17; p++) {
        snprintf(buf, sizeof buf, \"%.*g\", p, d);
        if (strtod(buf, NULL) == d) break;
    }
    return vbr_dup(buf);
}
";

// Same shortest-round-trip trick as `vbr_from_double`, but in `float`
// precision — so a `Single` prints `3.14`, not the `double`-promotion noise
// `3.140000104904175`.
const RT_FROM_FLOAT: &str = "\
static char* vbr_from_float(float f) {
    char buf[32];
    for (int p = 1; p <= 9; p++) {
        snprintf(buf, sizeof buf, \"%.*g\", p, (double)f);
        if (strtof(buf, NULL) == f) break;
    }
    return vbr_dup(buf);
}
";

const RT_RND: &str = "\
static double rnd(void) {
    static int seeded = 0;
    if (!seeded) {
        srand((unsigned)time(NULL));
        seeded = 1;
    }
    return (double)rand() / ((double)RAND_MAX + 1.0);
}
";

const RT_CONCAT: &str = "\
static char* vbr_concat(const char* a, const char* b) {
    char* s = (char*)malloc(strlen(a) + strlen(b) + 1);
    strcpy(s, a);
    strcat(s, b);
    return s;
}
";

// VB string builtins: character counts (UTF-8 scalar values), not `strlen` bytes.
const RT_STRINGS: &str = r#"static int vbr_utf8_clen(unsigned char c) {
    if (c < 0x80) return 1;
    if ((c & 0xE0) == 0xC0) return 2;
    if ((c & 0xF0) == 0xE0) return 3;
    if ((c & 0xF8) == 0xF0) return 4;
    return 1;
}
static long long vbr_len(const char* s) {
    long long n = 0;
    for (const unsigned char* p = (const unsigned char*)s; *p; ) {
        p += vbr_utf8_clen(*p);
        n++;
    }
    return n;
}
static char* vbr_left(const char* s, long long n) {
    if (n <= 0) return vbr_dup("");
    const unsigned char* p = (const unsigned char*)s;
    long long i = 0;
    while (*p && i < n) { p += vbr_utf8_clen(*p); i++; }
    size_t nbytes = (size_t)(p - (const unsigned char*)s);
    char* d = (char*)malloc(nbytes + 1);
    memcpy(d, s, nbytes);
    d[nbytes] = 0;
    return d;
}
static char* vbr_right(const char* s, long long n) {
    if (n <= 0) return vbr_dup("");
    long long len = vbr_len(s);
    long long skip = len > n ? len - n : 0;
    const unsigned char* p = (const unsigned char*)s;
    long long i = 0;
    while (*p && i < skip) { p += vbr_utf8_clen(*p); i++; }
    return vbr_dup((const char*)p);
}
static char* vbr_mid(const char* s, long long start) {
    if (start < 1) start = 1;
    const unsigned char* p = (const unsigned char*)s;
    long long i = 1;
    while (*p && i < start) { p += vbr_utf8_clen(*p); i++; }
    return vbr_dup((const char*)p);
}
static char* vbr_mid_n(const char* s, long long start, long long count) {
    if (count <= 0) return vbr_dup("");
    if (start < 1) start = 1;
    const unsigned char* p = (const unsigned char*)s;
    long long i = 1;
    while (*p && i < start) { p += vbr_utf8_clen(*p); i++; }
    const unsigned char* b = p;
    long long k = 0;
    while (*p && k < count) { p += vbr_utf8_clen(*p); k++; }
    size_t nbytes = (size_t)(p - b);
    char* d = (char*)malloc(nbytes + 1);
    memcpy(d, b, nbytes);
    d[nbytes] = 0;
    return d;
}
static char* vbr_ucase(const char* s) {
    char* d = vbr_dup(s);
    for (char* p = d; *p; p++) if (*p >= 'a' && *p <= 'z') *p = (char)(*p - 32);
    return d;
}
static char* vbr_lcase(const char* s) {
    char* d = vbr_dup(s);
    for (char* p = d; *p; p++) if (*p >= 'A' && *p <= 'Z') *p = (char)(*p + 32);
    return d;
}
static char* vbr_trim(const char* s) {
    while (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\r') s++;
    const char* e = s + strlen(s);
    while (e > s && (e[-1] == ' ' || e[-1] == '\t' || e[-1] == '\n' || e[-1] == '\r')) e--;
    size_t n = (size_t)(e - s);
    char* d = (char*)malloc(n + 1);
    memcpy(d, s, n);
    d[n] = 0;
    return d;
}
static char* vbr_replace(const char* s, const char* a, const char* b) {
    size_t al = strlen(a), bl = strlen(b);
    if (al == 0) return vbr_dup(s);
    size_t count = 0;
    for (const char* p = s; (p = strstr(p, a)); p += al) count++;
    char* out = (char*)malloc(strlen(s) + count * bl + 1);
    char* o = out;
    const char* p = s;
    const char* hit;
    while ((hit = strstr(p, a))) {
        size_t n = (size_t)(hit - p);
        memcpy(o, p, n); o += n;
        memcpy(o, b, bl); o += bl;
        p = hit + al;
    }
    strcpy(o, p);
    return out;
}
static char* vbr_chr(long long n) {
    unsigned char c = (unsigned char)n;
    char* d = (char*)malloc(5);
    if (c < 0x80) { d[0] = (char)c; d[1] = 0; }
    else {
        d[0] = (char)(0xC0 | (c >> 6));
        d[1] = (char)(0x80 | (c & 0x3F));
        d[2] = 0;
    }
    return d;
}
static long long vbr_asc(const char* s) {
    if (!s || !*s) return 0;
    const unsigned char* p = (const unsigned char*)s;
    unsigned char c = p[0];
    if (c < 0x80) return (long long)c;
    if ((c & 0xE0) == 0xC0 && p[1])
        return (long long)(((c & 0x1F) << 6) | (p[1] & 0x3F));
    if ((c & 0xF0) == 0xE0 && p[1] && p[2])
        return (long long)(((c & 0x0F) << 12) | ((p[1] & 0x3F) << 6) | (p[2] & 0x3F));
    if ((c & 0xF8) == 0xF0 && p[1] && p[2] && p[3])
        return (long long)(((c & 0x07) << 18) | ((p[1] & 0x3F) << 12) | ((p[2] & 0x3F) << 6) | (p[3] & 0x3F));
    return (long long)c;
}
"#;

const RT_INSTR: &str = r#"static Option_longlong vbr_instr(const char* hay, const char* needle) {
    const char* hit = strstr(hay, needle);
    if (!hit) return (Option_longlong){ .is_some = false };
    long long chars = 0;
    for (const unsigned char* p = (const unsigned char*)hay; (const char*)p < hit; ) {
        p += vbr_utf8_clen(*p);
        chars++;
    }
    return (Option_longlong){ .is_some = true, .value = chars + 1 };
}
"#;

const RT_VAL: &str = r#"static double vbr_val(const char* s) {
    while (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\r') s++;
    if (!*s) return 0.0;
    char* end;
    double d = strtod(s, &end);
    while (*end == ' ' || *end == '\t' || *end == '\n' || *end == '\r') end++;
    if (end == s || *end) return 0.0;
    return d;
}
"#;

const RT_CDBL: &str = r#"static Result_double_str vbr_cdbl(const char* s) {
    while (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\r') s++;
    if (!*s) return (Result_double_str){ .is_ok = false, .err = vbr_dup("cannot parse float from empty string") };
    char* end;
    double d = strtod(s, &end);
    while (*end == ' ' || *end == '\t' || *end == '\n' || *end == '\r') end++;
    if (end == s || *end) return (Result_double_str){ .is_ok = false, .err = vbr_dup("invalid float literal") };
    return (Result_double_str){ .is_ok = true, .ok = d };
}
"#;

const RT_CLNG: &str = r#"static Result_longlong_str vbr_clng(const char* s) {
    while (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\r') s++;
    if (!*s) return (Result_longlong_str){ .is_ok = false, .err = vbr_dup("cannot parse integer from empty string") };
    char* end;
    long long n = strtoll(s, &end, 10);
    while (*end == ' ' || *end == '\t' || *end == '\n' || *end == '\r') end++;
    if (end == s || *end) return (Result_longlong_str){ .is_ok = false, .err = vbr_dup("invalid digit found in string") };
    return (Result_longlong_str){ .is_ok = true, .ok = n };
}
"#;

const RT_CINT: &str = r#"static Result_int_str vbr_cint(const char* s) {
    while (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\r') s++;
    if (!*s) return (Result_int_str){ .is_ok = false, .err = vbr_dup("cannot parse integer from empty string") };
    char* end;
    long long n = strtoll(s, &end, 10);
    while (*end == ' ' || *end == '\t' || *end == '\n' || *end == '\r') end++;
    if (end == s || *end) return (Result_int_str){ .is_ok = false, .err = vbr_dup("invalid digit found in string") };
    if (n < (-2147483647LL - 1) || n > 2147483647LL)
        return (Result_int_str){ .is_ok = false, .err = vbr_dup("out of range") };
    return (Result_int_str){ .is_ok = true, .ok = (int)n };
}
"#;

const RT_INPUT: &str = r#"static Result_str_str vbr_input_box(const char* prompt) {
    fputs(prompt, stdout);
    fflush(stdout);
    char buf[4096];
    if (!fgets(buf, (int)sizeof buf, stdin))
        return (Result_str_str){ .is_ok = false, .err = vbr_dup("end of input") };
    size_t n = strlen(buf);
    if (n && buf[n - 1] == '\n') buf[--n] = 0;
    if (n && buf[n - 1] == '\r') buf[--n] = 0;
    return (Result_str_str){ .is_ok = true, .ok = vbr_dup(buf) };
}
"#;

const RT_ROUND_PLACES: &str = r#"static double vbr_round_places(double x, long long p) {
    double s = pow(10.0, (double)p);
    return round(x * s) / s;
}
"#;

const RT_SPLIT: &str = r#"static Vec_str vbr_split(const char* s, const char* delim) {
    Vec_str v = {0};
    size_t dlen = strlen(delim);
    if (dlen == 0) {
        Vec_str_push(&v, vbr_dup(s));
        return v;
    }
    const char* p = s;
    for (;;) {
        const char* hit = strstr(p, delim);
        if (!hit) {
            Vec_str_push(&v, vbr_dup(p));
            break;
        }
        size_t n = (size_t)(hit - p);
        char* part = (char*)malloc(n + 1);
        memcpy(part, p, n);
        part[n] = 0;
        Vec_str_push(&v, part);
        p = hit + dlen;
    }
    return v;
}
"#;

const RT_JOIN: &str = r#"static char* vbr_join(Vec_str v, const char* delim) {
    if (v.len == 0) return vbr_dup("");
    size_t dlen = strlen(delim);
    size_t total = dlen * (v.len - 1);
    for (size_t i = 0; i < v.len; i++) total += strlen(v.data[i]);
    char* out = (char*)malloc(total + 1);
    char* o = out;
    for (size_t i = 0; i < v.len; i++) {
        size_t n = strlen(v.data[i]);
        memcpy(o, v.data[i], n);
        o += n;
        if (i + 1 < v.len) {
            memcpy(o, delim, dlen);
            o += dlen;
        }
    }
    *o = 0;
    return out;
}
"#;

const RT_SPACE: &str = r#"static char* vbr_space(long long n) {
    if (n < 0) n = 0;
    char* d = (char*)malloc((size_t)n + 1);
    memset(d, ' ', (size_t)n);
    d[n] = 0;
    return d;
}
"#;

const RT_FMT_DOUBLE: &str = r#"static char* vbr_fmt_double(double x, const char* spec) {
    char b[128];
    snprintf(b, sizeof b, spec, x);
    return vbr_dup(b);
}
"#;

const RT_FMT_LL: &str = r#"static char* vbr_fmt_ll(long long x, const char* spec) {
    char b[128];
    snprintf(b, sizeof b, spec, x);
    return vbr_dup(b);
}
"#;

// ---- standard library: FileSystem (file I/O over stdio + POSIX) ----
// Each fallible call returns the same `Result<_, String>` the Rust stdlib does;
// the error text is the C library's `strerror(errno)`.

const RT_FS_READ: &str = "\
static Result_str_str vbr_fs_read(char* path) {
    FILE* f = fopen(path, \"rb\");
    if (!f) return (Result_str_str){ .is_ok = false, .err = vbr_dup(strerror(errno)) };
    fseek(f, 0, SEEK_END);
    long n = ftell(f);
    fseek(f, 0, SEEK_SET);
    char* buf = (char*)malloc((size_t)n + 1);
    size_t got = fread(buf, 1, (size_t)n, f);
    buf[got] = '\\0';
    fclose(f);
    return (Result_str_str){ .is_ok = true, .ok = buf };
}
";

const RT_FS_WRITE: &str = "\
static Result_unit_str vbr_fs_write(char* path, char* contents) {
    FILE* f = fopen(path, \"wb\");
    if (!f) return (Result_unit_str){ .is_ok = false, .err = vbr_dup(strerror(errno)) };
    fwrite(contents, 1, strlen(contents), f);
    fclose(f);
    return (Result_unit_str){ .is_ok = true };
}
";

const RT_FS_DELETE: &str = "\
static Result_unit_str vbr_fs_delete(char* path) {
    if (remove(path) != 0) return (Result_unit_str){ .is_ok = false, .err = vbr_dup(strerror(errno)) };
    return (Result_unit_str){ .is_ok = true };
}
";

const RT_FS_EXISTS: &str = "\
static bool vbr_fs_exists(char* path) {
    struct stat st;
    return stat(path, &st) == 0 && S_ISREG(st.st_mode);
}
";

// ---- standard library: DateTime (naive local time over <time.h>) ----
// A `DateTime` is a `struct tm`; `Parse`/`Format` use the same strftime patterns
// chrono does. Arithmetic goes through `timegm` (UTC, no DST) so it stays naive,
// matching chrono's `NaiveDateTime + Duration`.
const RT_DATETIME: &str = "\
static Result_DateTime_str vbr_datetime_parse(char* text, char* pattern) {
    struct tm tm = {0};
    if (strptime(text, pattern, &tm) == NULL)
        return (Result_DateTime_str){ .is_ok = false, .err = vbr_dup(\"could not parse date\") };
    return (Result_DateTime_str){ .is_ok = true, .ok = tm };
}
static DateTime vbr_datetime_now(void) {
    time_t t = time(NULL);
    struct tm r;
    localtime_r(&t, &r);
    return r;
}
static long long vbr_datetime_year(DateTime* d) { return d->tm_year + 1900; }
static long long vbr_datetime_month(DateTime* d) { return d->tm_mon + 1; }
static long long vbr_datetime_day(DateTime* d) { return d->tm_mday; }
static char* vbr_datetime_format(DateTime* d, char* pattern) {
    char buf[256];
    strftime(buf, sizeof buf, pattern, d);
    return vbr_dup(buf);
}
static DateTime vbr_datetime_shift(DateTime* d, long long seconds) {
    struct tm t = *d;
    time_t s = timegm(&t) + seconds;
    struct tm r;
    gmtime_r(&s, &r);
    return r;
}
static DateTime vbr_datetime_adddays(DateTime* d, long long days) { return vbr_datetime_shift(d, days * 86400); }
static DateTime vbr_datetime_addhours(DateTime* d, long long hours) { return vbr_datetime_shift(d, hours * 3600); }
static DateTime vbr_datetime_addminutes(DateTime* d, long long mins) { return vbr_datetime_shift(d, mins * 60); }
";

// ---- standard library: Regex (POSIX <regex.h>) ----
// POSIX ERE has no `\\s`/`\\d`/`\\w`, so translate those PCRE escapes (the ones
// Rust's regex crate has) to POSIX classes first — enough for common patterns.
const RT_REGEX: &str = "\
static char* vbr_regex_posix(char* pat) {
    char* out = (char*)malloc(strlen(pat) * 12 + 1);
    char* o = out;
    for (char* p = pat; *p; p++) {
        if (*p == '\\\\' && p[1]) {
            const char* rep = NULL;
            switch (p[1]) {
                case 's': rep = \"[[:space:]]\"; break;
                case 'S': rep = \"[^[:space:]]\"; break;
                case 'd': rep = \"[[:digit:]]\"; break;
                case 'D': rep = \"[^[:digit:]]\"; break;
                case 'w': rep = \"[[:alnum:]_]\"; break;
                case 'W': rep = \"[^[:alnum:]_]\"; break;
            }
            if (rep) { strcpy(o, rep); o += strlen(rep); p++; continue; }
        }
        *o++ = *p;
    }
    *o = '\\0';
    return out;
}
static Result_str_str vbr_regex_replaceall(char* pattern, char* text, char* replacement) {
    char* pat = vbr_regex_posix(pattern);
    regex_t re;
    if (regcomp(&re, pat, REG_EXTENDED) != 0) {
        free(pat);
        return (Result_str_str){ .is_ok = false, .err = vbr_dup(\"invalid regex\") };
    }
    free(pat);
    size_t cap = 64, len = 0, rlen = strlen(replacement);
    char* out = (char*)malloc(cap);
    const char* cur = text;
    int not_bol = 0;
    regmatch_t m;
    while (regexec(&re, cur, 1, &m, not_bol ? REG_NOTBOL : 0) == 0) {
        size_t pre = (size_t)m.rm_so;
        while (len + pre + rlen + 2 > cap) { cap *= 2; out = (char*)realloc(out, cap); }
        memcpy(out + len, cur, pre); len += pre;
        memcpy(out + len, replacement, rlen); len += rlen;
        size_t adv = (size_t)m.rm_eo;
        if (m.rm_eo == m.rm_so) {
            if (cur[m.rm_eo] == '\\0') break;
            out[len++] = cur[m.rm_eo];
            adv = (size_t)m.rm_eo + 1;
        }
        cur += adv;
        not_bol = 1;
        if (*cur == '\\0') break;
    }
    size_t tail = strlen(cur);
    while (len + tail + 1 > cap) { cap *= 2; out = (char*)realloc(out, cap); }
    memcpy(out + len, cur, tail); len += tail;
    out[len] = '\\0';
    regfree(&re);
    return (Result_str_str){ .is_ok = true, .ok = out };
}
";

// ---- standard library: Shell (run/start over POSIX) ----
// `Run` captures stdout via popen and waits; `Start` forks a child and hands
// back a Process handle (kill / poll / wait). A signal-killed child's exit code
// is unknowable, reported as -1, matching the Rust/Python stdlib.
const RT_SHELL: &str = "\
static Result_str_str vbr_shell_run(char* cmd) {
    FILE* p = popen(cmd, \"r\");
    if (!p) return (Result_str_str){ .is_ok = false, .err = vbr_dup(strerror(errno)) };
    size_t cap = 256, len = 0;
    char* out = (char*)malloc(cap);
    int c;
    while ((c = fgetc(p)) != EOF) {
        if (len + 2 > cap) { cap *= 2; out = (char*)realloc(out, cap); }
        out[len++] = (char)c;
    }
    out[len] = '\\0';
    while (len > 0 && (out[len - 1] == '\\n' || out[len - 1] == '\\r' || out[len - 1] == ' ' || out[len - 1] == '\\t'))
        out[--len] = '\\0';
    int status = pclose(p);
    int code = WIFEXITED(status) ? WEXITSTATUS(status) : -1;
    if (code == 0) return (Result_str_str){ .is_ok = true, .ok = out };
    char msg[64];
    snprintf(msg, sizeof msg, \"command failed with code %d\", code);
    free(out);
    return (Result_str_str){ .is_ok = false, .err = vbr_dup(msg) };
}
static Result_Process_str vbr_shell_start(char* cmd) {
    pid_t pid = fork();
    if (pid < 0) return (Result_Process_str){ .is_ok = false, .err = vbr_dup(strerror(errno)) };
    if (pid == 0) { execl(\"/bin/sh\", \"sh\", \"-c\", cmd, (char*)NULL); _exit(127); }
    return (Result_Process_str){ .is_ok = true, .ok = (Process){ .pid = pid, .reaped = 0, .code = 0 } };
}
static void vbr_process_kill(Process* p) {
    if (p->reaped) return;
    kill(p->pid, SIGKILL);
    int st; waitpid(p->pid, &st, 0);
    p->reaped = 1; p->code = -1;
}
static bool vbr_process_isrunning(Process* p) {
    if (p->reaped) return false;
    int st;
    pid_t r = waitpid(p->pid, &st, WNOHANG);
    if (r == 0) return true;
    p->reaped = 1;
    p->code = WIFEXITED(st) ? WEXITSTATUS(st) : -1;
    return false;
}
static long long vbr_process_wait(Process* p) {
    if (p->reaped) return p->code;
    int st; waitpid(p->pid, &st, 0);
    p->reaped = 1;
    p->code = WIFEXITED(st) ? WEXITSTATUS(st) : -1;
    return p->code;
}
";

// ---- standard library: Json (vendored cJSON) ----
// A `Json` wraps a `cJSON*` node owned by the parsed document. The typed
// accessors mirror `vbr_stdlib::Json` — `get_*`/`as_*` return a `Result`, the
// error text echoing serde's `Key '…' is not a …`. (cJSON's parse error and
// number formatting differ from serde in the failure/serialisation paths, which
// this slice's example doesn't exercise.)
const RT_JSON: &str = "\
static char* vbr_json__err(const char* pre, const char* key, const char* post) {
    char* s = (char*)malloc(strlen(pre) + strlen(key) + strlen(post) + 1);
    strcpy(s, pre); strcat(s, key); strcat(s, post);
    return s;
}
static cJSON* vbr_json__field(Json* self, char* key) {
    return cJSON_GetObjectItemCaseSensitive(self->node, key);
}
static Result_Json_str vbr_json_parse(char* text) {
    cJSON* n = cJSON_Parse(text);
    if (!n) return (Result_Json_str){ .is_ok = false, .err = vbr_dup(\"invalid JSON\") };
    return (Result_Json_str){ .is_ok = true, .ok = (Json){ .node = n } };
}
static Json vbr_json_object(void) { return (Json){ .node = cJSON_CreateObject() }; }
static Json vbr_json_array(void) { return (Json){ .node = cJSON_CreateArray() }; }
static bool vbr_json_haskey(Json* self, char* key) { return vbr_json__field(self, key) != NULL; }
static bool vbr_json_isnull(Json* self) { return cJSON_IsNull(self->node); }
static Result_str_str vbr_json_getstring(Json* self, char* key) {
    cJSON* f = vbr_json__field(self, key);
    if (!f) return (Result_str_str){ .is_ok = false, .err = vbr_json__err(\"Key '\", key, \"' not found\") };
    if (!cJSON_IsString(f)) return (Result_str_str){ .is_ok = false, .err = vbr_json__err(\"Key '\", key, \"' is not a string\") };
    return (Result_str_str){ .is_ok = true, .ok = vbr_dup(f->valuestring) };
}
static Result_longlong_str vbr_json_getint(Json* self, char* key) {
    cJSON* f = vbr_json__field(self, key);
    if (!f) return (Result_longlong_str){ .is_ok = false, .err = vbr_json__err(\"Key '\", key, \"' not found\") };
    if (!cJSON_IsNumber(f) || f->valuedouble != (double)(long long)f->valuedouble)
        return (Result_longlong_str){ .is_ok = false, .err = vbr_json__err(\"Key '\", key, \"' is not an integer\") };
    return (Result_longlong_str){ .is_ok = true, .ok = (long long)f->valuedouble };
}
static Result_double_str vbr_json_getfloat(Json* self, char* key) {
    cJSON* f = vbr_json__field(self, key);
    if (!f) return (Result_double_str){ .is_ok = false, .err = vbr_json__err(\"Key '\", key, \"' not found\") };
    if (!cJSON_IsNumber(f)) return (Result_double_str){ .is_ok = false, .err = vbr_json__err(\"Key '\", key, \"' is not a float\") };
    return (Result_double_str){ .is_ok = true, .ok = f->valuedouble };
}
static Result_bool_str vbr_json_getbool(Json* self, char* key) {
    cJSON* f = vbr_json__field(self, key);
    if (!f) return (Result_bool_str){ .is_ok = false, .err = vbr_json__err(\"Key '\", key, \"' not found\") };
    if (!cJSON_IsBool(f)) return (Result_bool_str){ .is_ok = false, .err = vbr_json__err(\"Key '\", key, \"' is not a boolean\") };
    return (Result_bool_str){ .is_ok = true, .ok = cJSON_IsTrue(f) };
}
static Result_vec_Json_str vbr_json_getarray(Json* self, char* key) {
    cJSON* f = vbr_json__field(self, key);
    if (!f) return (Result_vec_Json_str){ .is_ok = false, .err = vbr_json__err(\"Key '\", key, \"' not found\") };
    if (!cJSON_IsArray(f)) return (Result_vec_Json_str){ .is_ok = false, .err = vbr_json__err(\"Key '\", key, \"' is not an array\") };
    Vec_Json out = {0};
    cJSON* it = NULL;
    cJSON_ArrayForEach(it, f) Vec_Json_push(&out, (Json){ .node = it });
    return (Result_vec_Json_str){ .is_ok = true, .ok = out };
}
static Result_Json_str vbr_json_get(Json* self, char* key) {
    cJSON* f = vbr_json__field(self, key);
    if (!f) return (Result_Json_str){ .is_ok = false, .err = vbr_json__err(\"Key '\", key, \"' not found\") };
    return (Result_Json_str){ .is_ok = true, .ok = (Json){ .node = f } };
}
static Result_str_str vbr_json_asstring(Json* self) {
    if (!cJSON_IsString(self->node)) return (Result_str_str){ .is_ok = false, .err = vbr_dup(\"value is not a string\") };
    return (Result_str_str){ .is_ok = true, .ok = vbr_dup(self->node->valuestring) };
}
static Result_longlong_str vbr_json_asint(Json* self) {
    if (!cJSON_IsNumber(self->node) || self->node->valuedouble != (double)(long long)self->node->valuedouble)
        return (Result_longlong_str){ .is_ok = false, .err = vbr_dup(\"value is not an integer\") };
    return (Result_longlong_str){ .is_ok = true, .ok = (long long)self->node->valuedouble };
}
static Result_double_str vbr_json_asfloat(Json* self) {
    if (!cJSON_IsNumber(self->node)) return (Result_double_str){ .is_ok = false, .err = vbr_dup(\"value is not a float\") };
    return (Result_double_str){ .is_ok = true, .ok = self->node->valuedouble };
}
static Result_bool_str vbr_json_asbool(Json* self) {
    if (!cJSON_IsBool(self->node)) return (Result_bool_str){ .is_ok = false, .err = vbr_dup(\"value is not a boolean\") };
    return (Result_bool_str){ .is_ok = true, .ok = cJSON_IsTrue(self->node) };
}
static Result_str_str vbr_json_tostring(Json* self) {
    char* s = cJSON_PrintUnformatted(self->node);
    if (!s) return (Result_str_str){ .is_ok = false, .err = vbr_dup(\"could not serialise\") };
    char* d = vbr_dup(s); free(s);
    return (Result_str_str){ .is_ok = true, .ok = d };
}
static Result_str_str vbr_json_topretty(Json* self) {
    char* s = cJSON_Print(self->node);
    if (!s) return (Result_str_str){ .is_ok = false, .err = vbr_dup(\"could not serialise\") };
    char* d = vbr_dup(s); free(s);
    return (Result_str_str){ .is_ok = true, .ok = d };
}
static void vbr_json_setstring(Json* self, char* key, char* val) {
    cJSON_DeleteItemFromObjectCaseSensitive(self->node, key);
    cJSON_AddStringToObject(self->node, key, val);
}
static void vbr_json_setint(Json* self, char* key, long long val) {
    cJSON_DeleteItemFromObjectCaseSensitive(self->node, key);
    cJSON_AddNumberToObject(self->node, key, (double)val);
}
static void vbr_json_setbool(Json* self, char* key, bool val) {
    cJSON_DeleteItemFromObjectCaseSensitive(self->node, key);
    cJSON_AddBoolToObject(self->node, key, val);
}
static void vbr_json_set(Json* self, char* key, Json val) {
    cJSON_DeleteItemFromObjectCaseSensitive(self->node, key);
    cJSON_AddItemToObject(self->node, key, val.node);
}
static void vbr_json_push(Json* self, Json val) {
    cJSON_AddItemToArray(self->node, val.node);
}
";

// ---- standard library: Database (SQLite, linked -lsqlite3) ----
// A `Database` is a live connection. Params bind positionally as text (SQLite
// column affinity types them); a `Query` row comes back as a `Json` object keyed
// by column name, each column carrying its storage class (INTEGER/REAL → number,
// TEXT → string, NULL → json null) — so the `Json` accessors read it typed.
const RT_DATABASE: &str = "\
static Result_Database_str vbr_db_open(char* path) {
    sqlite3* conn;
    if (sqlite3_open(path, &conn) != SQLITE_OK) {
        char* e = vbr_dup(sqlite3_errmsg(conn));
        sqlite3_close(conn);
        return (Result_Database_str){ .is_ok = false, .err = e };
    }
    return (Result_Database_str){ .is_ok = true, .ok = (Database){ .conn = conn } };
}
static void vbr_db__bind(sqlite3_stmt* stmt, Vec_str params) {
    for (size_t i = 0; i < params.len; i++)
        sqlite3_bind_text(stmt, (int)i + 1, params.data[i], -1, SQLITE_TRANSIENT);
}
static Result_longlong_str vbr_db_execute(Database* self, char* sql, Vec_str params) {
    sqlite3_stmt* stmt;
    if (sqlite3_prepare_v2(self->conn, sql, -1, &stmt, NULL) != SQLITE_OK)
        return (Result_longlong_str){ .is_ok = false, .err = vbr_dup(sqlite3_errmsg(self->conn)) };
    vbr_db__bind(stmt, params);
    int rc = sqlite3_step(stmt);
    if (rc != SQLITE_DONE && rc != SQLITE_ROW) {
        char* e = vbr_dup(sqlite3_errmsg(self->conn));
        sqlite3_finalize(stmt);
        return (Result_longlong_str){ .is_ok = false, .err = e };
    }
    sqlite3_finalize(stmt);
    return (Result_longlong_str){ .is_ok = true, .ok = sqlite3_changes(self->conn) };
}
static Result_vec_Json_str vbr_db_query(Database* self, char* sql, Vec_str params) {
    sqlite3_stmt* stmt;
    if (sqlite3_prepare_v2(self->conn, sql, -1, &stmt, NULL) != SQLITE_OK)
        return (Result_vec_Json_str){ .is_ok = false, .err = vbr_dup(sqlite3_errmsg(self->conn)) };
    vbr_db__bind(stmt, params);
    Vec_Json out = {0};
    int ncol = sqlite3_column_count(stmt);
    while (sqlite3_step(stmt) == SQLITE_ROW) {
        cJSON* obj = cJSON_CreateObject();
        for (int i = 0; i < ncol; i++) {
            const char* name = sqlite3_column_name(stmt, i);
            switch (sqlite3_column_type(stmt, i)) {
                case SQLITE_INTEGER: cJSON_AddNumberToObject(obj, name, (double)sqlite3_column_int64(stmt, i)); break;
                case SQLITE_FLOAT:   cJSON_AddNumberToObject(obj, name, sqlite3_column_double(stmt, i)); break;
                case SQLITE_TEXT:    cJSON_AddStringToObject(obj, name, (const char*)sqlite3_column_text(stmt, i)); break;
                default:             cJSON_AddNullToObject(obj, name); break;
            }
        }
        Vec_Json_push(&out, (Json){ .node = obj });
    }
    sqlite3_finalize(stmt);
    return (Result_vec_Json_str){ .is_ok = true, .ok = out };
}
static long long vbr_db_lastinsertid(Database* self) {
    return sqlite3_last_insert_rowid(self->conn);
}
";

// ---- standard library: Http (one-shot requests over libcurl, linked -lcurl) ----
// Each call is an independent blocking request (no shared session), matching the
// Rust stdlib. The response body accumulates through a write callback; a 60s
// timeout turns a hung server into an `Err` rather than a forever-wait.
const RT_HTTP: &str = "\
struct vbr_http_buf { char* data; size_t len; };
static size_t vbr_http__write(char* ptr, size_t size, size_t nmemb, void* userdata) {
    size_t n = size * nmemb;
    struct vbr_http_buf* b = (struct vbr_http_buf*)userdata;
    char* d = (char*)realloc(b->data, b->len + n + 1);
    if (!d) return 0;
    b->data = d;
    memcpy(b->data + b->len, ptr, n);
    b->len += n;
    b->data[b->len] = '\\0';
    return n;
}
static Result_str_str vbr_http__perform(CURL* curl, struct vbr_http_buf* buf, struct curl_slist* hdrs) {
    curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, vbr_http__write);
    curl_easy_setopt(curl, CURLOPT_WRITEDATA, buf);
    curl_easy_setopt(curl, CURLOPT_TIMEOUT, 60L);
    curl_easy_setopt(curl, CURLOPT_FOLLOWLOCATION, 1L);
    CURLcode rc = curl_easy_perform(curl);
    if (hdrs) curl_slist_free_all(hdrs);
    if (rc != CURLE_OK) {
        char* e = vbr_dup(curl_easy_strerror(rc));
        free(buf->data);
        curl_easy_cleanup(curl);
        return (Result_str_str){ .is_ok = false, .err = e };
    }
    curl_easy_cleanup(curl);
    return (Result_str_str){ .is_ok = true, .ok = buf->data ? buf->data : vbr_dup(\"\") };
}
static Result_str_str vbr_http_get(char* url) {
    CURL* curl = curl_easy_init();
    if (!curl) return (Result_str_str){ .is_ok = false, .err = vbr_dup(\"curl init failed\") };
    struct vbr_http_buf buf = {0};
    curl_easy_setopt(curl, CURLOPT_URL, url);
    return vbr_http__perform(curl, &buf, NULL);
}
static Result_str_str vbr_http_post(char* url, char* body, Map_str_str headers) {
    CURL* curl = curl_easy_init();
    if (!curl) return (Result_str_str){ .is_ok = false, .err = vbr_dup(\"curl init failed\") };
    struct vbr_http_buf buf = {0};
    curl_easy_setopt(curl, CURLOPT_URL, url);
    curl_easy_setopt(curl, CURLOPT_POSTFIELDS, body);
    struct curl_slist* hdrs = NULL;
    for (size_t i = 0; i < headers.len; i++) {
        char* k = headers.entries[i].key;
        char* v = headers.entries[i].val;
        char* line = (char*)malloc(strlen(k) + strlen(v) + 3);
        strcpy(line, k); strcat(line, \": \"); strcat(line, v);
        hdrs = curl_slist_append(hdrs, line);
        free(line);
    }
    if (hdrs) curl_easy_setopt(curl, CURLOPT_HTTPHEADER, hdrs);
    return vbr_http__perform(curl, &buf, hdrs);
}
";

// ---- helpers ----

/// The C type a `DeclType` lowers to (slice 1–2 scalars, strings, structs).
fn c_type(ty: &DeclType) -> String {
    match ty {
        DeclType::Plain(Type::Integer) => "int".to_string(),
        DeclType::Plain(Type::Long | Type::LongLong) => "long long".to_string(),
        DeclType::Plain(Type::Byte) => "unsigned char".to_string(),
        DeclType::Plain(Type::Single) => "float".to_string(),
        DeclType::Plain(Type::Double) => "double".to_string(),
        DeclType::Plain(Type::Boolean) => "bool".to_string(),
        DeclType::Plain(Type::Text) => "char*".to_string(),
        // A user struct is `typedef`'d, so its bare name is the type.
        DeclType::Named(n) => n.clone(),
        // Collections are monomorphised: `Vec<Long>` → the `Vec_longlong` type.
        DeclType::Vec(_) => vec_name(ty),
        DeclType::Map(..) => map_name(ty),
        DeclType::Option(_) => opt_name(ty),
        DeclType::Result(..) => res_name(ty),
        _ => "long long".to_string(),
    }
}

/// Wrap a success type in `Result<T, String>`. `None` is unit `Result<()>`.
fn result_of(inner: Option<&DeclType>) -> DeclType {
    let t = inner.cloned().unwrap_or_else(|| DeclType::Tuple(Vec::new()));
    DeclType::Result(Box::new(t), Box::new(DeclType::Plain(Type::Text)))
}

fn c_body_diverges(stmts: &[Stmt]) -> bool {
    match stmts.iter().rev().find(|s| !matches!(s, Stmt::Comment(_) | Stmt::LineMark(_))) {
        Some(Stmt::Return(_) | Stmt::RaiseError(_) | Stmt::Break | Stmt::Continue) => true,
        Some(Stmt::If { branches, else_body }) => {
            else_body.as_ref().is_some_and(|e| c_body_diverges(e))
                && branches.iter().all(|(_, b)| c_body_diverges(b))
        }
        Some(Stmt::Match { arms, .. }) => !arms.is_empty() && arms.iter().all(|a| c_body_diverges(&a.body)),
        Some(Stmt::HandleErr { body, .. }) => c_body_diverges(body),
        _ => false,
    }
}

fn is_unit(t: &DeclType) -> bool {
    matches!(t, DeclType::Tuple(v) if v.is_empty())
}

/// Does this expression end in an iterator consumer the `Dim`-level lowering
/// handles as a loop? (`collect`/`sum`/`any`/`all`; `count`/`len` stay a field.)
fn is_iter_terminal(e: &Expr) -> bool {
    matches!(
        iter::parse(e),
        Some(iter::Chain {
            terminal: iter::Terminal::Collect
                | iter::Terminal::Sum
                | iter::Terminal::Any { .. }
                | iter::Terminal::All { .. },
            ..
        })
    )
}

/// Is `name` a standard-library value type (with instance methods)?
fn is_stdlib_type(name: &str) -> bool {
    matches!(name.to_ascii_lowercase().as_str(), "datetime" | "process" | "json" | "database")
}

/// Does `ty` mention the named type anywhere (including nested in a `Result`/
/// `Vec`/…)? Used to detect a stdlib value type from the inferred types.
fn type_mentions(ty: &DeclType, name: &str) -> bool {
    match ty {
        DeclType::Named(n) => n == name,
        DeclType::Vec(e) | DeclType::Option(e) => type_mentions(e, name),
        DeclType::Map(a, b) | DeclType::Result(a, b) => type_mentions(a, name) || type_mentions(b, name),
        DeclType::Tuple(ts) => ts.iter().any(|t| type_mentions(t, name)),
        _ => false,
    }
}

/// A type's name fragment for monomorphised container type names.
fn mangle(t: &DeclType) -> String {
    match t {
        DeclType::Plain(Type::Integer) => "int".to_string(),
        DeclType::Plain(Type::Long | Type::LongLong) => "longlong".to_string(),
        DeclType::Plain(Type::Byte) => "byte".to_string(),
        DeclType::Plain(Type::Single) => "float".to_string(),
        DeclType::Plain(Type::Double) => "double".to_string(),
        DeclType::Plain(Type::Boolean) => "bool".to_string(),
        DeclType::Plain(Type::Text) => "str".to_string(),
        DeclType::Named(n) => n.clone(),
        DeclType::Vec(e) => format!("vec_{}", mangle(e)),
        DeclType::Map(k, v) => format!("map_{}_{}", mangle(k), mangle(v)),
        DeclType::Option(t) => format!("opt_{}", mangle(t)),
        DeclType::Result(t, e) => format!("res_{}_{}", mangle(t), mangle(e)),
        DeclType::Tuple(v) if v.is_empty() => "unit".to_string(),
        _ => "unknown".to_string(),
    }
}

fn vec_name(ty: &DeclType) -> String {
    match ty {
        DeclType::Vec(e) => format!("Vec_{}", mangle(e)),
        _ => "Vec".to_string(),
    }
}

fn map_name(ty: &DeclType) -> String {
    match ty {
        DeclType::Map(k, v) => format!("Map_{}_{}", mangle(k), mangle(v)),
        _ => "Map".to_string(),
    }
}

fn opt_name(ty: &DeclType) -> String {
    match ty {
        DeclType::Option(t) => format!("Option_{}", mangle(t)),
        _ => "Option".to_string(),
    }
}

fn res_name(ty: &DeclType) -> String {
    match ty {
        DeclType::Result(t, e) => format!("Result_{}_{}", mangle(t), mangle(e)),
        _ => "Result".to_string(),
    }
}

fn is_single(ty: &DeclType) -> bool {
    matches!(ty, DeclType::Plain(Type::Single))
}

/// Every generic instantiation used in the program, in one dependency-respecting
/// order (inner types precede the containers holding them) — so a nested
/// container like `Result<Vec<Json>>` emits its `Vec_Json` typedef before the
/// `Result_vec_Json_str` that embeds it, across all container kinds.
#[derive(Default)]
struct Collected {
    order: Vec<DeclType>,
}

fn gather_types(program: &Program, c: &mut Collected) {
    for s in &program.structs {
        for f in &s.fields {
            visit_ty(&f.ty, c);
        }
    }
    for f in &program.functions {
        for p in &f.params {
            visit_ty(&p.ty, c);
        }
        if let Some(r) = &f.ret {
            visit_ty(r, c);
        }
        gather_body(&f.body, c);
    }
}

fn gather_body(stmts: &[Stmt], c: &mut Collected) {
    for s in stmts {
        match s {
            Stmt::Dim { ty, .. } => visit_ty(ty, c),
            Stmt::DestructureDim { ty: Some(t), .. } => visit_ty(t, c),
            Stmt::If { branches, else_body } => {
                for (_, b) in branches {
                    gather_body(b, c);
                }
                if let Some(b) = else_body {
                    gather_body(b, c);
                }
            }
            Stmt::For { body, .. } | Stmt::ForEach { body, .. } | Stmt::DoLoop { body, .. } => {
                gather_body(body, c)
            }
            Stmt::Match { arms, .. } => {
                for a in arms {
                    gather_body(&a.body, c);
                }
            }
            _ => {}
        }
    }
}

fn visit_ty(t: &DeclType, c: &mut Collected) {
    match t {
        DeclType::Vec(e) | DeclType::Option(e) => visit_ty(e, c),
        DeclType::Map(a, b) | DeclType::Result(a, b) => {
            visit_ty(a, c);
            visit_ty(b, c);
        }
        _ => return,
    }
    // Each container is recorded *after* its inner types, so the single ordered
    // list is already a valid decl-before-use emission order.
    if !c.order.contains(t) {
        c.order.push(t.clone());
    }
}

fn is_text(ty: &DeclType) -> bool {
    matches!(ty, DeclType::Plain(Type::Text))
}

fn is_bool(ty: &DeclType) -> bool {
    matches!(ty, DeclType::Plain(Type::Boolean))
}

fn is_float(ty: &DeclType) -> bool {
    matches!(ty, DeclType::Plain(Type::Single | Type::Double))
}

/// A Bust identifier as a C one. VB is case-insensitive, so everything lowercases
/// (which also turns `Function Main` into C's `main`).
fn c_name(name: &str) -> String {
    name.to_ascii_lowercase()
}

/// A non-concat binary operator in C. `Xor` on booleans is `!=` (0/1 values).
fn bin_op(op: BinOp) -> &'static str {
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
        BinOp::And => "&&",
        BinOp::Or => "||",
        BinOp::Xor => "!=",
        // Handled specially (never reached here).
        BinOp::Pow | BinOp::Concat => "+",
    }
}

/// A maths builtin → its `<math.h>` name, or `None` for a user function.
fn c_math_builtin(name: &str) -> Option<&'static str> {
    Some(match name.to_ascii_lowercase().as_str() {
        "sqr" => "sqrt",
        "int" => "floor",
        "round" => "round", // C's `round` is half-away-from-zero, like Rust's
        "abs" => "fabs",
        "sin" => "sin",
        "cos" => "cos",
        "tan" => "tan",
        "atn" => "atan",
        "exp" => "exp",
        "log" => "log", // natural log, like Rust's `ln`
        _ => return None,
    })
}

/// A C double literal — always with a decimal point so it reads as a `double`.
fn c_float(f: f64) -> String {
    let s = format!("{}", f);
    if s.contains('.') || s.contains('e') || s.contains('E') || s.contains("inf") || s.contains("NaN") {
        s
    } else {
        format!("{}.0", s)
    }
}

/// A C string literal (double-quoted, the usual escapes).
fn c_string(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
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
        Stmt::Match { .. } => "Match",
        _ => "statement",
    }
}

fn expr_name(e: &ExprKind) -> &'static str {
    match e {
        ExprKind::MethodCall { .. } => "method call",
        ExprKind::Field(..) => "field access",
        ExprKind::Index(..) => "indexing",
        ExprKind::List(_) => "list literal",
        ExprKind::Tuple(_) => "tuple",
        ExprKind::StructLit { .. } => "struct literal",
        ExprKind::InlineRust(_) => "inline Rust",
        ExprKind::InlinePython { .. } => "inline Python",
        _ => "expression",
    }
}
