//! The **C backend** — VBR's third target, after Rust and Python. Where the
//! Python target could lean on dynamic typing and a garbage collector, C gives
//! us neither: every declaration needs a type (supplied by the neutral typing
//! pass, [`crate::types`]) and every heap value must be freed by hand. VBR's
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

use std::collections::HashMap;

use crate::ast::*;
use crate::pattern::{self, Pat};
use crate::transpiler::convert_returns;
use crate::types::{type_program, TypeTable};

/// The generated C, plus any constructs that couldn't cross cleanly.
pub struct CProgram {
    pub code: String,
    pub warnings: Vec<String>,
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
        indent: 0,
        warnings: Vec::new(),
        needs_math: false,
        need_dup: false,
        need_from_ll: false,
        need_from_bool: false,
        need_from_double: false,
        need_from_float: false,
        need_concat: false,
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
    indent: usize,
    warnings: Vec<String>,
    // Runtime helpers switched on as the body needs them.
    needs_math: bool,
    need_dup: bool,
    need_from_ll: bool,
    need_from_bool: bool,
    need_from_double: bool,
    need_from_float: bool,
    need_concat: bool,
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

    fn program(&mut self, program: &Program) {
        if !program.windows.is_empty() || !program.screens.is_empty() || !program.pages.is_empty() {
            self.warn(
                "GUI/TUI/Web surfaces (`Window`/`Screen`/`Page`) are Rust-only — \
                 the C target is for the core language.",
            );
        }
        for c in &program.constants {
            self.const_names.insert(c.name.to_ascii_lowercase(), c.name.clone());
        }
        for e in &program.enums {
            let is_data = e.variants.iter().any(|v| !v.payload.is_empty());
            let variants = e.variants.iter().map(|v| (v.name.clone(), v.payload.clone())).collect();
            self.enums
                .insert(e.name.to_ascii_lowercase(), EnumInfo { name: e.name.clone(), is_data, variants });
        }
        self.enum_typedefs(program);
        self.struct_typedefs(program);
        self.constants(program);
        for (i, func) in program.functions.iter().enumerate() {
            if i > 0 {
                self.out.push('\n');
            }
            self.function(func);
        }
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
        let sig = self.signature(func, is_main);
        self.line(&format!("{} {{", sig));
        self.indent += 1;

        let mut body = func.body.clone();
        convert_returns(&mut body, &func.name);
        self.block(&body);

        // `Function Main()` returns nothing in VBR; C's `main` returns a status.
        if is_main {
            self.line("return 0;");
        }
        self.indent -= 1;
        self.line("}");
    }

    /// The C signature (no trailing `;` — the caller adds `{` or `;`).
    fn signature(&self, func: &Function, is_main: bool) -> String {
        if is_main {
            return "int main(void)".to_string();
        }
        let ret = match &func.ret {
            Some(t) => c_type(t),
            None => "void".to_string(),
        };
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
            Stmt::LineMark(_) => {}
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
                let v = self.expr(e);
                self.line(&format!("return {};", v));
            }
            Stmt::Return(None) => self.line("return;"),
            // A bare expression used for its effect (`alice.HaveBirthday()`).
            Stmt::Expr(e) => {
                let v = self.expr(e);
                self.line(&format!("{};", v));
            }
            Stmt::If { branches, else_body } => self.if_stmt(branches, else_body.as_deref()),
            Stmt::For { var, from, to, step, body } => self.for_stmt(var, from, to, step.as_ref(), body),
            Stmt::DoLoop { cond, body } => self.do_loop(cond, body),
            Stmt::Match { scrutinee, arms, .. } => self.match_stmt(scrutinee, arms),
            Stmt::Break => self.line("break;"),
            Stmt::Continue => self.line("continue;"),
            other => {
                self.warn(format!("`{}` doesn't lower to C yet.", stmt_name(other)));
                self.line(&format!("/* [VBR→C] unsupported: {} */", stmt_name(other)));
            }
        }
    }

    fn dim(&mut self, name: &str, ty: &DeclType, init: Option<&Expr>) {
        let cty = c_type(ty);
        let var = c_name(name);
        match init {
            // A string literal is copied onto the heap so it's uniform with
            // concat results (both `char*` you may later `= Nothing`).
            Some(Expr { kind: ExprKind::Str(s), .. }) if is_text(ty) => {
                self.need_dup = true;
                let lit = c_string(s);
                self.line(&format!("{} {} = vbr_dup({});", cty, var, lit));
            }
            Some(e) => {
                let v = self.expr(e);
                self.line(&format!("{} {} = {};", cty, var, v));
            }
            None => {
                let zero = if is_text(ty) { "NULL" } else { "0" };
                self.line(&format!("{} {} = {};", cty, var, zero));
            }
        }
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

    /// A C expression yielding a `char*` for `value`, converting per its type —
    /// the counterpart of Rust's `Display` / Python's `_vb`.
    fn as_str(&mut self, e: &Expr) -> String {
        let ty = self.type_of(e);
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
            ExprKind::MethodCall { recv, method, args } => self.method_call(recv, method, args),
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
            ExprKind::Call { name, args } => self.call(name, args),
            other => {
                self.warn(format!("`{}` doesn't lower to C yet.", expr_name(other)));
                "0 /* [VBR→C] unsupported */".to_string()
            }
        }
    }

    fn method_call(&mut self, recv: &Expr, method: &str, args: &[Expr]) -> String {
        let struct_name = match self.type_of(recv) {
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

        let scrut_key = match &sty {
            DeclType::Named(n) => Some(n.to_ascii_lowercase()),
            _ => None,
        };
        let is_data = scrut_key.as_ref().and_then(|k| self.enums.get(k)).map(|e| e.is_data).unwrap_or(false);

        let last = arms.len().saturating_sub(1);
        // Does an earlier guardless arm already catch everything?
        let has_catch_all = arms.iter().any(|a| {
            a.guard.is_none() && matches!(pattern::parse(&a.pattern), Pat::Wildcard | Pat::Binding(_))
        });

        for (i, arm) in arms.iter().enumerate() {
            let pat = pattern::parse(&arm.pattern);
            // Bind a bare `x` to the scrutinee temp for the guard/body.
            let mut added: Vec<String> = Vec::new();
            if let Pat::Binding(x) = &pat {
                self.aliases.insert(x.to_ascii_lowercase(), temp.clone());
                added.push(x.to_ascii_lowercase());
            }

            // The unconditional `else`: a wildcard/bare binding, or the last
            // guardless arm of an exhaustive match (so C sees the chain as total).
            let is_closer = arm.guard.is_none()
                && (matches!(pat, Pat::Wildcard | Pat::Binding(_)) || (i == last && !has_catch_all));

            let cond = if matches!(pat, Pat::Wildcard | Pat::Binding(_)) || is_closer {
                None
            } else {
                Some(self.condition(&pat, &temp, is_data))
            };
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
            // A data variant's payload fields become locals in the arm block.
            if let Pat::Variant { variant, binds, .. } = &pat {
                if let Some(key) = &scrut_key {
                    let (_, cvariant, payloads) = self.variant_canon(key, variant);
                    for (idx, b) in binds.iter().enumerate() {
                        let pty = payloads.get(idx).map(c_type).unwrap_or_else(|| "long long".to_string());
                        self.line(&format!("{} {} = {}.data.{}.f{};", pty, c_name(b), temp, cvariant, idx));
                    }
                }
            }
            self.block(&arm.body);
            self.indent -= 1;
            for a in added {
                self.aliases.remove(&a);
            }
        }
        self.line("}");
    }

    /// The C boolean condition testing whether `temp` matches `pat`.
    fn condition(&self, pat: &Pat, temp: &str, is_data: bool) -> String {
        match pat {
            Pat::Int(n) => format!("{} == {}", temp, n),
            Pat::Range(a, b) => format!("{} >= {} && {} <= {}", temp, a, temp, b),
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
            Pat::Other(s) => format!("0 /* unsupported pattern: {} */", s),
        }
    }

    fn tag_const(&self, enom: &str, variant: &str) -> String {
        let (ename, cvariant, _) = self.variant_canon(&enom.to_ascii_lowercase(), variant);
        format!("{}_{}", ename, cvariant)
    }

    fn call(&mut self, name: &str, args: &[Expr]) -> String {
        let rendered: Vec<String> = args.iter().map(|a| self.expr(a)).collect();
        // Maths builtins → the C standard library (all in `<math.h>`).
        if let Some(cfn) = c_math_builtin(name) {
            self.needs_math = true;
            return format!("{}({})", cfn, rendered.join(", "));
        }
        format!("{}({})", c_name(name), rendered.join(", "))
    }

    fn type_of(&self, e: &Expr) -> DeclType {
        self.types.get(&e.span).cloned().unwrap_or(DeclType::Plain(Type::Long))
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

        code.push_str("#include <stdio.h>\n");
        code.push_str("#include <stdlib.h>\n");
        code.push_str("#include <string.h>\n");
        code.push_str("#include <stdbool.h>\n");
        if self.needs_math {
            code.push_str("#include <math.h>\n");
        }
        code.push('\n');

        // Struct `typedef`s + module constants come before the functions that
        // use them (and before the prototypes, which name struct types).
        code.push_str(&self.decls);

        // The inlined runtime — only the helpers the body used, in dependency
        // order (`vbr_from_*` build on `vbr_dup`).
        let need_dup = self.need_dup || self.need_from_bool || self.need_from_double || self.need_from_float;
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
        if need_dup
            || self.need_from_ll
            || self.need_from_bool
            || self.need_from_double
            || self.need_from_float
            || self.need_concat
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

        code.push_str(&self.out);

        CProgram { code, warnings: self.warnings }
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

const RT_CONCAT: &str = "\
static char* vbr_concat(const char* a, const char* b) {
    char* s = (char*)malloc(strlen(a) + strlen(b) + 1);
    strcpy(s, a);
    strcat(s, b);
    return s;
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
        _ => "long long".to_string(),
    }
}

fn is_single(ty: &DeclType) -> bool {
    matches!(ty, DeclType::Plain(Type::Single))
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

/// A VBR identifier as a C one. VB is case-insensitive, so everything lowercases
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
