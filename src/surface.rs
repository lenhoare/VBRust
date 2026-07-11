//! The shared State/View/Events core.
//!
//! A GUI `Window` (Iced) and a TUI `Screen` (ratatui) are the same Elm-style
//! machine — state is the source of truth, the view is derived from it, events
//! update it — rendered by different backends. This module owns everything that
//! machine-shape implies, independent of the renderer: the program prologue
//! (constants/structs/enums/functions), state-field maps, the `state.field`
//! rewrite, event-body lowering, `Await` splitting, blocking-call checks, and
//! stdlib import collection. `gui.rs` and `tui.rs` are view renderers plus a
//! runtime shell over this core — a future backend (say, a web `Page`) would be
//! a third renderer, not a third copy.

use crate::ast::*;
use crate::diagnostics::Diagnostics;
use crate::resolver;
use crate::transpiler::{
    decltype_rust, emit_const, emit_enum, emit_fn, emit_impl, emit_stmt, emit_struct,
    note_builtins, render_expr, rust_name, stdlib_type,
};
use std::collections::{HashMap, HashSet};

/// The program-wide lookup tables every backend builds before emitting: enum
/// names, function/method signatures, constants, and struct fields — plus, in
/// a multi-file project, the sibling module names and their harvested
/// interfaces, so a helper function or event can call `Life.StepLife(…)`
/// with the same argument treatment a plain program gets.
pub(crate) struct Tables {
    pub enums: HashSet<String>,
    pub fns: resolver::FnTable,
    pub methods: resolver::MethodTable,
    pub consts: HashMap<String, String>,
    pub structs: resolver::StructTable,
    pub modules: HashSet<String>,
    pub interfaces: resolver::ProjectInterfaces,
}

pub(crate) fn build_tables(
    program: &Program,
    modules: &[String],
    interfaces: &resolver::ProjectInterfaces,
) -> Tables {
    let mut t = Tables {
        enums: program.enums.iter().map(|e| e.name.clone()).collect(),
        fns: resolver::build_fn_table(program),
        methods: resolver::build_method_table(program),
        consts: resolver::build_const_map(program),
        structs: resolver::build_struct_table(program),
        modules: modules.iter().cloned().collect(),
        interfaces: interfaces.clone(),
    };
    // Siblings' Public Types/Enums join the tables under their bare names —
    // state fields, events, and views use a foreign type like a local one
    // (`transpile_module` adds the matching `use crate::module::Name;`).
    resolver::merge_sibling_types(&mut t.enums, &mut t.structs, &mut t.methods, interfaces);
    t
}

/// The `mod <name>;` declarations a multi-file project's crate root carries —
/// one per sibling module, alphabetical, mirroring the plain-program entry.
pub(crate) fn emit_mod_decls(modules: &[String], is_entry: bool, out: &mut String) {
    if !is_entry || modules.is_empty() {
        return;
    }
    let mut mods: Vec<&String> = modules.iter().collect();
    mods.sort();
    for m in mods {
        out.push_str(&format!("mod {};\n", m));
    }
    out.push('\n');
}

/// Emit the items a surface program defines around its windows/screens: leading
/// comments, constants, structs, enums, methods (grouped into `impl` blocks),
/// and free functions — everything except `Main`, which becomes the backend's
/// `fn main`. `special_fn` lets a backend claim a function and emit it its own
/// way (the GUI's paint functions); return `true` to mark it handled.
pub(crate) fn emit_shared_items(
    program: &Program,
    t: &Tables,
    diags: &mut Diagnostics,
    out: &mut String,
    special_fn: &mut dyn FnMut(&Function, &mut Diagnostics, &mut String) -> bool,
) {
    for comment in &program.leading_comments {
        out.push_str(&format!("// {}\n", comment));
    }
    if !program.leading_comments.is_empty() {
        out.push('\n');
    }
    for c in &program.constants {
        emit_const(c, out, diags);
    }
    if !program.constants.is_empty() {
        out.push('\n');
    }
    for s in &program.structs {
        emit_struct(s, diags, out);
        out.push('\n');
    }
    for e in &program.enums {
        emit_enum(e, out);
        out.push('\n');
    }

    let is_main = |f: &Function| f.receiver.is_none() && f.name.eq_ignore_ascii_case("Main");
    for f in &program.functions {
        if !is_main(f) {
            note_builtins(&f.body, diags);
        }
    }
    // Methods, grouped into `impl` blocks (receivers in first-seen order).
    let mut receivers: Vec<&String> = Vec::new();
    for f in &program.functions {
        if let Some(r) = &f.receiver {
            if !receivers.contains(&r) {
                receivers.push(r);
            }
        }
    }
    for recv in receivers {
        emit_impl(
            recv, program, &t.fns, &t.methods, &t.consts, &t.modules, &t.interfaces, &t.enums,
            &t.structs, diags, out,
        );
        out.push('\n');
    }
    // Free functions, except `Main`.
    for f in program.functions.iter().filter(|f| f.receiver.is_none() && !is_main(f)) {
        if !special_fn(f, diags, out) {
            emit_fn(
                f, &t.fns, &t.methods, &t.consts, &t.modules, &t.interfaces, &t.enums, &t.structs,
                diags, out, 0, None,
            );
        }
        out.push('\n');
    }
}

/// Find the surface launched by a `<Name>.Run` statement inside `Function
/// Main()` — the property form (`Counter.Run`) or the call form
/// (`Counter.Run()`). `find` looks the name up among the backend's surfaces
/// (windows or screens); scanning continues past names it doesn't recognise.
pub(crate) fn launched<'a, T>(
    program: &'a Program,
    find: impl Fn(&str) -> Option<&'a T>,
) -> Option<&'a T> {
    let main = program.functions.iter().find(|f| f.name.eq_ignore_ascii_case("Main"))?;
    for stmt in &main.body {
        if let Stmt::Expr(e) = stmt {
            let (recv, method) = match e {
                Expr::Field(recv, m) => (recv.as_ref(), m),
                Expr::MethodCall { recv, method, .. } => (recv.as_ref(), method),
                _ => continue,
            };
            if !method.eq_ignore_ascii_case("run") {
                continue;
            }
            if let Expr::Ident(name) = recv {
                if let Some(found) = find(name) {
                    return Some(found);
                }
            }
        }
    }
    None
}

/// The `std` imports a surface's event bodies need, independent of the renderer.
/// Each backend owns its *crate* imports (Iced / ratatui / Yew), but `std` types
/// used inside events are common to all of them — so the decision lives here,
/// once. Today that's `HashMap` (e.g. an `Http.Post` headers map built in an
/// event). A new surface gets this by calling it in its preamble; a new std type
/// is added here, not re-discovered in every emitter.
/// The `std` `use` lines a surface needs — scanning both **event bodies** and
/// **helper functions/methods** (a `HashMap` `Dim` in a helper like
/// `ChatComplete` needs the import just as much as one built in an event; the
/// old scan saw only events). `helpers` is the program's free functions and
/// methods; `Main` carries no state and is fine to include (it's just more body
/// to scan).
pub(crate) fn surface_std_imports(events: &[GuiEvent], helpers: &[Function]) -> String {
    let mut out = String::new();
    let in_event = events.iter().any(|e| crate::transpiler::body_uses_hashmap(&e.body));
    let in_helper = helpers.iter().any(|f| crate::transpiler::body_uses_hashmap(&f.body));
    if in_event || in_helper {
        out.push_str("use std::collections::HashMap;\n");
    }
    out
}

/// True when a `State` field initialiser is a *fallible* call — one returning a
/// `Result` that the generated `init()` unwraps with `?`: a known stdlib
/// constructor (`Database.Open`, `Json.Parse`, …), one of the program's own
/// functions whose declared return type is a `Result`, or a sibling module's
/// public function returning one (`Life.LoadDb()`).
pub(crate) fn fallible_init(e: &Expr, t: &Tables) -> bool {
    match e {
        Expr::MethodCall { recv, method, .. } => {
            let Expr::Ident(r) = &**recv else { return false };
            if let Some(canon) = stdlib_type(r) {
                return matches!(
                    (canon, rust_name(method).as_str()),
                    ("Database", "open")
                        | ("Json", "parse")
                        | ("DateTime", "parse")
                        | ("FileSystem", "read")
                        | ("FileSystem", "read_lines")
                        | ("Shell", "run")
                        | ("Shell", "start")
                );
            }
            matches!(
                t.interfaces
                    .get(&rust_name(r))
                    .and_then(|i| i.fns.get(&rust_name(method)))
                    .and_then(|s| s.ret.as_ref()),
                Some(DeclType::Result(..))
            )
        }
        Expr::Call { name, .. } => matches!(
            t.fns.get(&rust_name(name)).and_then(|s| s.ret.as_ref()),
            Some(DeclType::Result(..))
        ),
        _ => false,
    }
}

/// Does any `State` field need the fallible `init()` constructor?
pub(crate) fn state_fallible(state: &[StateField], t: &Tables) -> bool {
    state.iter().any(|f| f.init.as_ref().map_or(false, |e| fallible_init(e, t)))
}

/// The stdlib namespaces used by `State` initialisers (e.g. `Database` for
/// `Database.Open`) — marked for the Cargo feature and returned for the
/// file-top `use vbr_stdlib::{…}` line.
pub(crate) fn state_stdlib(state: &[StateField], diags: &mut Diagnostics) -> Vec<&'static str> {
    let mut used = Vec::new();
    for f in state {
        if let Some(Expr::MethodCall { recv, .. }) = &f.init {
            if let Expr::Ident(r) = &**recv {
                if let Some(canon) = stdlib_type(r) {
                    if !used.contains(&canon) {
                        used.push(canon);
                        diags.mark(&format!("stdlib:{}", canon));
                    }
                }
            }
        }
    }
    used
}

/// The two views of a `State` block the emitters need: the field-name set (to
/// rewrite `count` → `state.count`) and name → declared type (for coercions).
pub(crate) fn state_maps(
    state: &[StateField],
) -> (HashSet<String>, HashMap<String, DeclType>) {
    let field_ty: HashMap<String, DeclType> =
        state.iter().map(|f| (rust_name(&f.name), f.ty.clone())).collect();
    let fields: HashSet<String> = field_ty.keys().cloned().collect();
    (fields, field_ty)
}

/// How a backend runs an awaited call. `Native` (Window/Screen) offloads the
/// blocking vbr_stdlib to a thread (`tokio::task::spawn_blocking` / a spawned
/// thread); the browser backends have no threads — `Http.Get` maps to the
/// generated `http_get` wrapper over the browser's own async `fetch` instead.
/// Also decides the state receiver the async split snapshots against: `state`
/// in an update fn or a Screen's key/timer closure, `self` in a Yew component.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum AsyncBackend {
    Native,
    Web,       // a Yew `Page`
    WebScreen, // a Ratzilla `Screen` (`vbr runweb` on a TUI program)
}

impl AsyncBackend {
    fn recv(self) -> &'static str {
        match self {
            AsyncBackend::Native | AsyncBackend::WebScreen => "state",
            AsyncBackend::Web => "self",
        }
    }

    fn is_browser(self) -> bool {
        !matches!(self, AsyncBackend::Native)
    }

    /// The surface name for teaching messages ("a Page" / "a browser Screen").
    fn surface_name(self) -> &'static str {
        match self {
            AsyncBackend::Web => "a Page",
            _ => "a browser Screen",
        }
    }
}

/// The fetch wrapper a browser backend emits (once) when an event awaits
/// `Http.Get` — shared by the Page (Yew) and browser-Screen (Ratzilla)
/// emitters.
pub(crate) const HTTP_GET_HELPER: &str = "\
/// The browser's `fetch`, shaped like the stdlib's `Http.Get`: the response
/// body on success; any failure (network, CORS, an HTTP error status) as a
/// `String` error.
async fn http_get(url: &str) -> Result<String, String> {
    let response = gloo_net::http::Request::get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.ok() {
        return Err(format!(\"HTTP {}\", response.status()));
    }
    response.text().await.map_err(|e| e.to_string())
}

";

/// Analyse every event: split each around an `Await` (None = synchronous), and
/// check that no blocking stdlib call runs un-`Await`ed (it would freeze the
/// UI). One entry per event, in order.
pub(crate) fn analyze_events(
    events: &[GuiEvent],
    field_ty: &HashMap<String, DeclType>,
    fns: &resolver::FnTable,
    diags: &mut Diagnostics,
    backend: AsyncBackend,
) -> Vec<Option<AwaitSplit>> {
    let splits =
        events.iter().map(|e| await_split(e, field_ty, fns, diags, backend)).collect();
    for e in events {
        check_blocking_without_await(&e.body, diags);
    }
    splits
}

/// The stdlib namespaces used across all event bodies, sorted and deduped —
/// ready for a `use vbr_stdlib::{…}` line. Marks each so the vbr_stdlib dep and
/// feature get added. (The web backend collects without marking — see
/// `collect_event_stdlib` — since its `Http` is the browser's fetch, not ours.)
pub(crate) fn event_stdlib_imports(events: &[GuiEvent], diags: &mut Diagnostics) -> Vec<String> {
    let mut used: Vec<String> = Vec::new();
    for e in events {
        collect_event_stdlib(&e.body, &mut used);
    }
    used.sort();
    used.dedup();
    for ns in &used {
        diags.mark(&format!("stdlib:{}", ns));
    }
    used
}

/// Lower a chunk of an event body (the whole body of a sync event, or the
/// pre-await / continuation halves of an async one) and emit it at `indent`.
/// The chunk first runs the ordinary resolver pass — a function body and an
/// event body are the same language — with the state fields and the event's
/// params in scope; then state-field references become `<recv>.field`
/// (`state` in a Window/Screen update, `self` in a Page's Yew component).
pub(crate) fn emit_event_stmts(
    stmts: &[Stmt],
    params: &[Param],
    recv: &'static str,
    fields: &HashSet<String>,
    field_ty: &HashMap<String, DeclType>,
    t: &Tables,
    indent: usize,
    diags: &mut Diagnostics,
    out: &mut String,
) {
    let mut body: Vec<Stmt> = stmts.to_vec();
    resolver::resolve_event_body(
        &mut body, params, field_ty, &t.fns, &t.methods, &t.consts, &t.modules, &t.interfaces,
        &t.enums, &t.structs, diags,
    );
    // A `Dim`'d For counter would be shadowed by the loop's own binding —
    // drop the dead `let`, exactly as in a plain function body.
    crate::transpiler::elide_for_counter_dims(&mut body);
    // A local reassigned or mutated in place (`headers.insert(…)`) needs
    // `let mut`, exactly as in a plain function body.
    let mut mutated: HashSet<String> = HashSet::new();
    crate::transpiler::collect_mutated(&body, &mut mutated);
    let empty: HashSet<String> = HashSet::new();
    for stmt in body {
        let mut rewritten = rewrite_stmt(stmt, recv, fields, &t.enums);
        coerce_state_strings(&mut rewritten, recv, field_ty);
        emit_stmt(&rewritten, &mutated, &empty, indent, diags, out);
    }
}

/// The scrutinee of a view `Match`: a bare `String` state field is matched as a
/// slice (`<recv>.name.as_str()`) so string-literal patterns line up.
pub(crate) fn match_scrutinee(
    scrutinee: &Expr,
    recv: &'static str,
    fields: &HashSet<String>,
    field_ty: &HashMap<String, DeclType>,
    enums: &HashSet<String>,
) -> String {
    let rendered = render_expr(&rewrite_expr_with(scrutinee.clone(), recv, fields, enums), None);
    if let Expr::Ident(name) = scrutinee {
        if matches!(field_ty.get(&rust_name(name)), Some(DeclType::Plain(Type::Text))) {
            return format!("{}.as_str()", rendered);
        }
    }
    rendered
}

/// A `State` field initialiser: a `String` becomes owned, numbers adapt to type,
/// an enum variant (`Size.Small`) resolves to its path (`Size::Small`), and a
/// `Vec` with no initialiser starts empty.
///
/// The initialiser first runs the ordinary resolver pass (as a synthetic `Dim`
/// of the field's type) — an initialiser and a function-body `Dim` are the same
/// language, so a call initialiser gets the same argument treatment (`&` on a
/// ByVal collection, owned strings, numeric casts) it would get anywhere else.
pub(crate) fn render_init(
    init: Option<&Expr>,
    ty: &DeclType,
    t: &Tables,
    diags: &mut Diagnostics,
) -> String {
    let init = init.map(|e| {
        let mut body = vec![Stmt::Dim {
            name: "field".to_string(),
            ty: ty.clone(),
            init: Some(e.clone()),
            line: 0,
        }];
        resolver::resolve_event_body(
            &mut body, &[], &HashMap::new(), &t.fns, &t.methods, &t.consts, &t.modules,
            &t.interfaces, &t.enums, &t.structs, diags,
        );
        match body.pop() {
            Some(Stmt::Dim { init: Some(e), .. }) => e,
            _ => e.clone(),
        }
    });
    match (ty, init) {
        (DeclType::Vec(_), None) => "Vec::new()".to_string(),
        // A bare string literal still needs owning; anything else the resolver
        // has already made owned where needed.
        (DeclType::Plain(Type::Text), Some(e @ Expr::Str(_))) => {
            format!("{}.to_string()", render_expr(&e, None))
        }
        (DeclType::Plain(Type::Text), Some(e)) => render_expr(&e, None),
        (DeclType::Plain(t), Some(e)) => render_expr(&e, Some(*t)),
        // Enum / Vec-with-initialiser / other — the resolver has rewritten
        // `Size.Small` → `Size::Small` and referenced call arguments.
        (_, Some(e)) => render_expr(&e, None),
        // A non-collection field without an initialiser shouldn't reach here (the
        // parser requires one); fall back to Default.
        (_, None) => "Default::default()".to_string(),
    }
}

/// Belt-and-braces after the resolver pass: a string literal assigned to a
/// `String` state field gets its `.to_string()` (`status = "x"` →
/// `state.status = "x".to_string()`), recursing through `Match`/`If` bodies.
/// The resolver normally does this from the typed environment; this catches
/// any assignment shape it doesn't cover. `state_recv` is the receiver the
/// state rewrite used (`state` or `self`).
pub(crate) fn coerce_state_strings(
    s: &mut Stmt,
    state_recv: &str,
    field_ty: &HashMap<String, DeclType>,
) {
    match s {
        Stmt::Assign { target: Expr::Field(recv, fname), value, .. }
            if matches!(&**recv, Expr::Ident(n) if n == state_recv)
                && matches!(field_ty.get(&rust_name(fname)), Some(DeclType::Plain(Type::Text)))
                && matches!(value, Expr::Str(_)) =>
        {
            let inner = std::mem::replace(value, Expr::Int(0));
            *value = Expr::MethodCall {
                recv: Box::new(inner),
                method: "to_string".to_string(),
                args: Vec::new(),
            };
        }
        Stmt::Match { arms, .. } => {
            for a in arms {
                for s2 in &mut a.body {
                    coerce_state_strings(s2, state_recv, field_ty);
                }
            }
        }
        Stmt::If { branches, else_body } => {
            for (_, b) in branches {
                for s2 in b {
                    coerce_state_strings(s2, state_recv, field_ty);
                }
            }
            if let Some(b) = else_body {
                for s2 in b {
                    coerce_state_strings(s2, state_recv, field_ty);
                }
            }
        }
        // Loop bodies carry statements too — descend into them, or an
        // `entry = "..."` inside a `For`/`Do` in an event never gets coerced
        // (the #16-shaped hole, one pass over).
        Stmt::For { body, .. } | Stmt::ForEach { body, .. } | Stmt::DoLoop { body, .. } => {
            for s2 in body {
                coerce_state_strings(s2, state_recv, field_ty);
            }
        }
        // Statements with no nested statements — nothing to descend into. Listed
        // explicitly (no `_`) so a future block-bearing statement is forced to
        // decide here rather than silently skipping coercion inside it.
        Stmt::Assign { .. }
        | Stmt::Dim { .. }
        | Stmt::Set { .. }
        | Stmt::DestructureDim { .. }
        | Stmt::HandleDim { .. }
        | Stmt::Return(_)
        | Stmt::Expr(_)
        | Stmt::Print(_)
        | Stmt::Break
        | Stmt::Continue
        | Stmt::Draw(_)
        // `Assert` only appears in a `Test` block, never a surface event body.
        | Stmt::Assert(_)
        | Stmt::Comment(_)
        | Stmt::LineMark(_) => {}
    }
}

/// The pieces of an event handler split around an `Await`.
pub(crate) struct AwaitSplit {
    pub(crate) pre: Vec<Stmt>,         // statements before the await (run in the kick-off)
    pub(crate) snapshots: Vec<String>, // `let url = state.url.clone();` for state used in the call
    pub(crate) call_src: String,       // the awaited call, e.g. `Http::get(&url)`
    pub(crate) ret_type: String,       // its result type, e.g. `Result<String, String>`
    pub(crate) blocking: bool,         // wrap the call in `spawn_blocking`
    pub(crate) bind: String,           // continuation binding: `result` (Match) or the Dim name
    pub(crate) cont: Vec<Stmt>,        // continuation statements (run when the result arrives)
}

/// What we need to know about an awaited stdlib call.
struct AwaitInfo {
    snapshots: Vec<String>,
    call_src: String,
    ret_type: String,
    blocking: bool,
}

/// Analyse an event for `Await`. `None` means a synchronous event. V1 supports a
/// single `Await` as the value of a `Match` (`Match Await Http.Get(url)`) or a
/// `Dim` (`Dim x = Await …`).
pub(crate) fn await_split(
    e: &GuiEvent,
    field_ty: &HashMap<String, DeclType>,
    fns: &resolver::FnTable,
    diags: &mut Diagnostics,
    backend: AsyncBackend,
) -> Option<AwaitSplit> {
    let idx = e.body.iter().position(stmt_has_await)?;
    // Locals visible where the `Await` sits — event params plus any `Dim`
    // declared before it — so an owned-String local (a built-up request body,
    // say) borrows as `&str` for the awaited call, like a state field does.
    let locals = local_types(&e.params, &e.body[..idx]);
    match &e.body[idx] {
        Stmt::Match { scrutinee: Expr::Await(call), arms, line } => {
            let info = awaitable_info(call, field_ty, &locals, fns, diags, backend)?;
            // Continuation runs `match result { <arms> }`, then any trailing code.
            let mut cont = vec![Stmt::Match {
                scrutinee: Expr::Ident("result".to_string()),
                arms: arms.clone(),
                line: *line,
            }];
            cont.extend(e.body[idx + 1..].iter().cloned());
            Some(AwaitSplit {
                pre: e.body[..idx].to_vec(),
                snapshots: info.snapshots,
                call_src: info.call_src,
                ret_type: info.ret_type,
                blocking: info.blocking,
                bind: "result".to_string(),
                cont,
            })
        }
        Stmt::Dim { name, init: Some(Expr::Await(call)), .. } => {
            let info = awaitable_info(call, field_ty, &locals, fns, diags, backend)?;
            Some(AwaitSplit {
                pre: e.body[..idx].to_vec(),
                snapshots: info.snapshots,
                call_src: info.call_src,
                ret_type: info.ret_type,
                blocking: info.blocking,
                bind: rust_name(name),
                cont: e.body[idx + 1..].to_vec(),
            })
        }
        _ => {
            diags.error_once(
                "await-position",
                "`Await` must be a *top-level* statement in an event — the value of a `Match` \
                 (`Match Await Http.Get(url)`) or a `Dim` (`Dim x = Await …`), not nested inside \
                 an `If`/`For`/`Match`. To guard the call, put the check *before* the `Await` \
                 (`If busy Then Return` / set a flag first), or move it into the awaited helper \
                 (return early on the guard). VBR keeps async deliberately simple: one `Await` \
                 per event, at the top.",
            );
            None
        }
    }
}

/// The declared types of the locals in scope at an `Await`: the event's params
/// and every `Dim` before it. Later declarations win, matching Rust shadowing.
fn local_types(params: &[Param], pre: &[Stmt]) -> HashMap<String, DeclType> {
    let mut m: HashMap<String, DeclType> =
        params.iter().map(|p| (rust_name(&p.name), p.ty.clone())).collect();
    for s in pre {
        if let Stmt::Dim { name, ty, .. } = s {
            m.insert(rust_name(name), ty.clone());
        }
    }
    m
}

/// The async task can't borrow the state, so snapshot (clone) any state fields
/// used as args, and render the call against those owned locals. Returns the
/// `let …` snapshot lines and the rendered argument list. `recv` is where the
/// state lives (`state` in an update fn, `self` in a Yew component).
fn snapshot_args(
    args: &[Expr],
    field_ty: &HashMap<String, DeclType>,
    locals: &HashMap<String, DeclType>,
    recv: &str,
) -> (Vec<String>, Vec<String>) {
    let mut snapshots = Vec::new();
    let mut arg_src = Vec::new();
    for a in args {
        match a {
            Expr::Ident(name) if field_ty.contains_key(&rust_name(name)) => {
                let f = rust_name(name);
                snapshots.push(format!("let {} = {}.{}.clone();", f, recv, f));
                if matches!(field_ty.get(&f), Some(DeclType::Plain(Type::Text))) {
                    arg_src.push(format!("&{}", f));
                } else {
                    arg_src.push(f);
                }
            }
            // A local (an event param or a `Dim` before the `Await`) is captured
            // by the async closure directly — no clone. An owned `String` still
            // borrows as `&str` for a stdlib `&str` param, just like a field.
            Expr::Ident(name) if matches!(locals.get(&rust_name(name)), Some(DeclType::Plain(Type::Text))) => {
                arg_src.push(format!("&{}", rust_name(name)));
            }
            other => arg_src.push(render_expr(other, None)),
        }
    }
    (snapshots, arg_src)
}

/// Resolve an awaited call to its Rust form, result type, and how to run it: a
/// known stdlib call (`Http.Get`), or one of the program's own functions (whose
/// return type the `FnTable` records). Natively both run off the UI thread; on
/// the web `Http.Get` maps to the generated `http_get` fetch wrapper instead
/// (the browser is single-threaded — its HTTP is async by nature).
fn awaitable_info(
    call: &Expr,
    field_ty: &HashMap<String, DeclType>,
    locals: &HashMap<String, DeclType>,
    fns: &resolver::FnTable,
    diags: &mut Diagnostics,
    backend: AsyncBackend,
) -> Option<AwaitInfo> {
    match call {
        // A stdlib call: `Http.Get(url)`.
        Expr::MethodCall { recv, method, args } => {
            let canon = match &**recv {
                Expr::Ident(r) => stdlib_type(r),
                _ => None,
            };
            let Some(canon) = canon else {
                diags.error_once(
                    "await-not-awaitable",
                    "`Await` works on a stdlib call (`Http.Get(url)`) or one of your own functions.",
                );
                return None;
            };
            let m = rust_name(method);
            if backend.is_browser() {
                if (canon, m.as_str()) != ("Http", "get") {
                    diags.error_once(
                        "await-unsupported",
                        format!(
                            "`Await {}.{}` isn't supported in {} yet — it awaits \
                             `Http.Get` (the browser's fetch).",
                            canon,
                            method,
                            backend.surface_name()
                        ),
                    );
                    return None;
                }
                // No vbr_stdlib on wasm — the call goes to the generated
                // `http_get` wrapper over the browser's fetch (gloo-net).
                let (snapshots, arg_src) = snapshot_args(args, field_ty, locals, backend.recv());
                return Some(AwaitInfo {
                    snapshots,
                    call_src: format!("http_get({})", arg_src.join(", ")),
                    ret_type: "Result<String, String>".to_string(),
                    blocking: false,
                });
            }
            let (ret_type, blocking) = match (canon, m.as_str()) {
                ("Http", "get") | ("Http", "post") | ("Shell", "run") => {
                    ("Result<String, String>".to_string(), true)
                }
                _ => {
                    diags.error_once(
                        "await-unsupported",
                        format!(
                            "`Await {}.{}` isn't supported yet — V1 awaits `Http.Get` or your \
                             own functions.",
                            canon, method
                        ),
                    );
                    return None;
                }
            };
            diags.mark(&format!("stdlib:{}", canon));
            let (snapshots, arg_src) = snapshot_args(args, field_ty, locals, backend.recv());
            let call_src = format!("{}::{}({})", canon, m, arg_src.join(", "));
            Some(AwaitInfo { snapshots, call_src, ret_type, blocking })
        }
        // One of the program's own functions — its return type comes from the
        // FnTable; it's synchronous Rust, so run it via `spawn_blocking`.
        Expr::Call { name, args } => {
            if backend.is_browser() {
                diags.error_once(
                    "page-await-fn",
                    format!(
                        "`Await {}(…)` isn't available in {} — the browser is \
                         single-threaded, with no background thread to run your function on. \
                         `Await` there works on `Http.Get`.",
                        name,
                        backend.surface_name()
                    ),
                );
                return None;
            }
            let Some(sig) = fns.get(&rust_name(name)) else {
                diags.error_once(
                    "await-unknown-fn",
                    format!("`Await {}(…)` — there's no function `{}` to await.", name, name),
                );
                return None;
            };
            let Some(dt) = &sig.ret else {
                diags.error_once(
                    "await-no-return",
                    format!(
                        "`Await {}(…)` needs `{}` to return a value, so its result can come back.",
                        name, name
                    ),
                );
                return None;
            };
            let ret_type = decltype_rust(dt);
            let (snapshots, arg_src) = snapshot_args(args, field_ty, locals, backend.recv());
            let call_src = format!("{}({})", rust_name(name), arg_src.join(", "));
            Some(AwaitInfo { snapshots, call_src, ret_type, blocking: true })
        }
        _ => {
            diags.error_once(
                "await-not-awaitable",
                "`Await` works on a stdlib call (`Http.Get(url)`) or one of your own functions.",
            );
            None
        }
    }
}

/// True if `e` is a stdlib call that blocks on I/O — so in a GUI event it must be
/// `Await`ed, or it freezes the window. (Same set `awaitable_info` knows about.)
fn is_blocking_stdlib_call(e: &Expr) -> bool {
    if let Expr::MethodCall { recv, method, .. } = e {
        if let Expr::Ident(r) = &**recv {
            if let Some(c) = stdlib_type(r) {
                return matches!(
                    (c, rust_name(method).as_str()),
                    ("Http", "get") | ("Http", "post") | ("Shell", "run")
                );
            }
        }
    }
    false
}

/// Teaching diagnostic: a blocking stdlib call used in an event *without* `Await`
/// would freeze the window. A call directly under `Await` is fine.
pub(crate) fn check_blocking_without_await(stmts: &[Stmt], diags: &mut Diagnostics) {
    fn ex(e: &Expr, awaited: bool, diags: &mut Diagnostics) {
        // The expression directly under `Await` is allowed to block.
        if let Expr::Await(inner) = e {
            ex(inner, true, diags);
            return;
        }
        if !awaited && is_blocking_stdlib_call(e) {
            diags.error_once(
                "blocking-no-await",
                "This stdlib call waits for I/O, so calling it directly in an event would \
                 freeze the UI until it finishes. Use `Await` so it runs off the UI thread \
                 — e.g. `Match Await Http.Get(url) … End Match`.",
            );
        }
        // `Sleep` in an event freezes the whole UI — and unlike I/O there's
        // nothing to await; the surface way to "do something later" is a timer.
        if let Expr::Call { name, .. } = e {
            if rust_name(name) == "sleep" {
                diags.error_once(
                    "sleep-in-event",
                    "`Sleep` pauses the whole UI thread — the screen freezes and keys go \
                     unanswered. To run something after a delay, use a timer instead: \
                     `Every <ms> <Event>`.",
                );
            }
        }
        // Children are never "awaited" by this expression.
        match e {
            Expr::Not(i) | Expr::Ref(i) | Expr::MutRef(i) | Expr::Deref(i) | Expr::Cast(i, _)
            | Expr::Try(i) | Expr::Field(i, _) | Expr::TupleIndex(i, _)
            | Expr::Closure { body: i, .. } => ex(i, false, diags),
            Expr::Binary { lhs, rhs, .. } | Expr::Index(lhs, rhs) => {
                ex(lhs, false, diags);
                ex(rhs, false, diags);
            }
            Expr::MethodCall { recv, args, .. } => {
                ex(recv, false, diags);
                for a in args {
                    ex(a, false, diags);
                }
            }
            Expr::Call { args, .. } => {
                for a in args {
                    ex(a, false, diags);
                }
            }
            Expr::Tuple(es) => {
                for e2 in es {
                    ex(e2, false, diags);
                }
            }
            Expr::StructLit { fields, .. } => {
                for (_, v) in fields {
                    ex(v, false, diags);
                }
            }
            _ => {}
        }
    }
    fn st(s: &Stmt, diags: &mut Diagnostics) {
        match s {
            Stmt::Assign { target, value, .. } => {
                ex(target, false, diags);
                ex(value, false, diags);
            }
            Stmt::Dim { init: Some(e), .. } => ex(e, false, diags),
            Stmt::Print(e) | Stmt::Expr(e) | Stmt::Return(Some(e)) => ex(e, false, diags),
            Stmt::If { branches, else_body } => {
                for (c, b) in branches {
                    ex(c, false, diags);
                    for s2 in b {
                        st(s2, diags);
                    }
                }
                if let Some(b) = else_body {
                    for s2 in b {
                        st(s2, diags);
                    }
                }
            }
            Stmt::Match { scrutinee, arms, .. } => {
                ex(scrutinee, false, diags);
                for a in arms {
                    for s2 in &a.body {
                        st(s2, diags);
                    }
                }
            }
            Stmt::For { body, .. } | Stmt::ForEach { body, .. } | Stmt::DoLoop { body, .. } => {
                for s2 in body {
                    st(s2, diags);
                }
            }
            _ => {}
        }
    }
    for s in stmts {
        st(s, diags);
    }
}

/// Does a statement contain an `Await` (in any expression position)?
pub(crate) fn stmt_has_await(s: &Stmt) -> bool {
    match s {
        Stmt::Dim { init: Some(e), .. } => expr_has_await(e),
        Stmt::Assign { target, value, .. } => expr_has_await(target) || expr_has_await(value),
        Stmt::Print(e) | Stmt::Expr(e) | Stmt::Return(Some(e)) => expr_has_await(e),
        Stmt::Match { scrutinee, arms, .. } => {
            expr_has_await(scrutinee) || arms.iter().any(|a| a.body.iter().any(stmt_has_await))
        }
        Stmt::If { branches, else_body } => {
            branches.iter().any(|(c, b)| expr_has_await(c) || b.iter().any(stmt_has_await))
                || else_body.as_ref().map_or(false, |b| b.iter().any(stmt_has_await))
        }
        Stmt::For { body, .. } | Stmt::ForEach { body, .. } | Stmt::DoLoop { body, .. } => {
            body.iter().any(stmt_has_await)
        }
        _ => false,
    }
}

fn expr_has_await(e: &Expr) -> bool {
    match e {
        Expr::Await(_) => true,
        Expr::Not(i) | Expr::Ref(i) | Expr::MutRef(i) | Expr::Deref(i) | Expr::Cast(i, _)
        | Expr::Try(i) | Expr::Field(i, _) | Expr::TupleIndex(i, _) | Expr::Closure { body: i, .. } => {
            expr_has_await(i)
        }
        Expr::Binary { lhs, rhs, .. } | Expr::Index(lhs, rhs) => {
            expr_has_await(lhs) || expr_has_await(rhs)
        }
        Expr::MethodCall { recv, args, .. } => {
            expr_has_await(recv) || args.iter().any(expr_has_await)
        }
        Expr::Call { args, .. } => args.iter().any(expr_has_await),
        Expr::Tuple(es) => es.iter().any(expr_has_await),
        Expr::StructLit { fields, .. } => fields.iter().any(|(_, v)| expr_has_await(v)),
        _ => false,
    }
}

/// Collect the stdlib namespaces (e.g. `Http`) used in event bodies — for the
/// `use vbr_stdlib::{…}` line. Pure collection; marking is the caller's call.
pub(crate) fn collect_event_stdlib(stmts: &[Stmt], out: &mut Vec<String>) {
    fn ex(e: &Expr, out: &mut Vec<String>) {
        match e {
            Expr::MethodCall { recv, args, .. } => {
                if let Expr::Ident(r) = &**recv {
                    if let Some(c) = stdlib_type(r) {
                        out.push(c.to_string());
                    }
                }
                ex(recv, out);
                for a in args {
                    ex(a, out);
                }
            }
            Expr::Await(i) | Expr::Not(i) | Expr::Ref(i) | Expr::MutRef(i) | Expr::Deref(i)
            | Expr::Cast(i, _) | Expr::Try(i) | Expr::Field(i, _) | Expr::TupleIndex(i, _)
            | Expr::Closure { body: i, .. } => ex(i, out),
            Expr::Binary { lhs, rhs, .. } | Expr::Index(lhs, rhs) => {
                ex(lhs, out);
                ex(rhs, out);
            }
            Expr::Call { args, .. } => {
                for a in args {
                    ex(a, out);
                }
            }
            Expr::Tuple(es) => {
                for e2 in es {
                    ex(e2, out);
                }
            }
            Expr::StructLit { fields, .. } => {
                for (_, v) in fields {
                    ex(v, out);
                }
            }
            _ => {}
        }
    }
    fn st(s: &Stmt, out: &mut Vec<String>) {
        match s {
            Stmt::Assign { target, value, .. } => {
                ex(target, out);
                ex(value, out);
            }
            Stmt::Dim { init: Some(e), .. } => ex(e, out),
            Stmt::Print(e) | Stmt::Expr(e) | Stmt::Return(Some(e)) => ex(e, out),
            Stmt::If { branches, else_body } => {
                for (c, b) in branches {
                    ex(c, out);
                    for s2 in b {
                        st(s2, out);
                    }
                }
                if let Some(b) = else_body {
                    for s2 in b {
                        st(s2, out);
                    }
                }
            }
            Stmt::Match { scrutinee, arms, .. } => {
                ex(scrutinee, out);
                for a in arms {
                    for s2 in &a.body {
                        st(s2, out);
                    }
                }
            }
            Stmt::For { body, .. } | Stmt::ForEach { body, .. } | Stmt::DoLoop { body, .. } => {
                for s2 in body {
                    st(s2, out);
                }
            }
            _ => {}
        }
    }
    for s in stmts {
        st(s, out);
    }
}

/// Replace a bare reference to a state field with `state.field`, and an enum
/// variant `Color.Red` with the path `Color::Red`, so an event/view expression
/// reaches the window's state and names variants correctly.
pub(crate) fn rewrite_expr(e: Expr, fields: &HashSet<String>, enums: &HashSet<String>) -> Expr {
    rewrite_expr_with(e, "state", fields, enums)
}

/// The general form: a bare state-field reference becomes `<recv>.field` — `state`
/// in a window's view/events, `self` inside a canvas `Draw` block.
pub(crate) fn rewrite_expr_with(
    e: Expr,
    recv: &'static str,
    fields: &HashSet<String>,
    enums: &HashSet<String>,
) -> Expr {
    let go = |e: Expr| rewrite_expr_with(e, recv, fields, enums);
    match e {
        // `Color.Red` (field on an enum name) → the path `Color::Red`.
        Expr::Field(inner, variant) if matches!(&*inner, Expr::Ident(n) if enums.contains(n)) => {
            match *inner {
                Expr::Ident(n) => Expr::ConstRef(format!("{}::{}", n, variant)),
                _ => unreachable!(),
            }
        }
        Expr::Ident(name) if fields.contains(&rust_name(&name)) => {
            Expr::Field(Box::new(Expr::Ident(recv.to_string())), name)
        }
        Expr::Binary { op, lhs, rhs } => Expr::Binary {
            op,
            lhs: Box::new(go(*lhs)),
            rhs: Box::new(go(*rhs)),
        },
        Expr::Not(inner) => Expr::Not(Box::new(go(*inner))),
        Expr::Call { name, args } => Expr::Call {
            name,
            args: args.into_iter().map(go).collect(),
        },
        // `Shape.Circle(r)` on an enum → the variant constructor `Shape::Circle(r)`.
        Expr::MethodCall { recv: r, method, args } if matches!(&*r, Expr::Ident(e) if enums.contains(e)) => {
            let e = match *r {
                Expr::Ident(n) => n,
                _ => unreachable!(),
            };
            Expr::Call {
                name: format!("{}::{}", e, method),
                args: args.into_iter().map(go).collect(),
            }
        }
        Expr::MethodCall { recv: r, method, args } => Expr::MethodCall {
            recv: Box::new(go(*r)),
            method,
            args: args.into_iter().map(go).collect(),
        },
        Expr::Field(inner, f) => Expr::Field(Box::new(go(*inner)), f),
        Expr::Index(a, b) => Expr::Index(Box::new(go(*a)), Box::new(go(*b))),
        Expr::Cast(inner, t) => Expr::Cast(Box::new(go(*inner)), t),
        // Wrappers the resolver may have added around a state field (`&db` for
        // a ByVal struct arg, `x?` chaining) — recurse through them all, or the
        // field inside never becomes `state.<field>`.
        Expr::Ref(inner) => Expr::Ref(Box::new(go(*inner))),
        Expr::MutRef(inner) => Expr::MutRef(Box::new(go(*inner))),
        Expr::Deref(inner) => Expr::Deref(Box::new(go(*inner))),
        Expr::Try(inner) => Expr::Try(Box::new(go(*inner))),
        Expr::Await(inner) => Expr::Await(Box::new(go(*inner))),
        Expr::Tuple(elems) => Expr::Tuple(elems.into_iter().map(go).collect()),
        Expr::List(elems) => Expr::List(elems.into_iter().map(go).collect()),
        Expr::StructLit { name, fields } => Expr::StructLit {
            name,
            fields: fields.into_iter().map(|(n, v)| (n, go(v))).collect(),
        },
        Expr::Closure { params, body, by_ref_params } => Expr::Closure {
            params,
            body: Box::new(go(*body)),
            by_ref_params,
        },
        Expr::TupleIndex(inner, i) => Expr::TupleIndex(Box::new(go(*inner)), i),
        // Leaves — nothing inside to rewrite. Listed explicitly (no `_`) so that
        // a new `Expr` variant carrying a child fails to compile here rather than
        // silently dropping a state field, the way #10/#16 once did. `InlineRust`
        // and `InlinePython` are opaque bodies; `InlinePython`'s `inputs` are
        // variable *names* (strings), not `Expr`s, so a state field passed into a
        // Python block still can't be rewritten — a known, separate limitation.
        leaf @ (Expr::Int(_)
        | Expr::Float(_)
        | Expr::Bool(_)
        | Expr::Str(_)
        | Expr::Ident(_)
        | Expr::ConstRef(_)
        | Expr::InlineRust(_)
        | Expr::InlinePython { .. }) => leaf,
    }
}

pub(crate) fn rewrite_stmt(
    s: Stmt,
    recv: &'static str,
    fields: &HashSet<String>,
    enums: &HashSet<String>,
) -> Stmt {
    let re = |e: Expr| rewrite_expr_with(e, recv, fields, enums);
    match s {
        Stmt::Assign { target, value, op } => Stmt::Assign {
            target: re(target),
            value: re(value),
            op,
        },
        Stmt::Print(e) => Stmt::Print(re(e)),
        Stmt::Expr(e) => Stmt::Expr(re(e)),
        Stmt::If { branches, else_body } => Stmt::If {
            branches: branches
                .into_iter()
                .map(|(c, b)| {
                    (
                        re(c),
                        b.into_iter().map(|s| rewrite_stmt(s, recv, fields, enums)).collect(),
                    )
                })
                .collect(),
            else_body: else_body
                .map(|b| b.into_iter().map(|s| rewrite_stmt(s, recv, fields, enums)).collect()),
        },
        Stmt::Match { scrutinee, arms, line } => Stmt::Match {
            scrutinee: re(scrutinee),
            arms: arms
                .into_iter()
                .map(|a| MatchArm {
                    pattern: a.pattern,
                    guard: a.guard.map(&re),
                    body: a
                        .body
                        .into_iter()
                        .map(|s| rewrite_stmt(s, recv, fields, enums))
                        .collect(),
                })
                .collect(),
            line,
        },
        Stmt::Dim { name, ty, init, line } => Stmt::Dim {
            name,
            ty,
            init: init.map(re),
            line,
        },
        Stmt::For { var, from, to, step, body } => Stmt::For {
            var,
            from: re(from),
            to: re(to),
            step: step.map(&re),
            body: body.into_iter().map(|s| rewrite_stmt(s, recv, fields, enums)).collect(),
        },
        Stmt::ForEach { var1, var2, iter, body } => Stmt::ForEach {
            var1,
            var2,
            iter: re(iter),
            body: body.into_iter().map(|s| rewrite_stmt(s, recv, fields, enums)).collect(),
        },
        Stmt::DoLoop { cond, body } => Stmt::DoLoop {
            cond: cond.map(|c| match c {
                DoCond::PreWhile(e) => DoCond::PreWhile(re(e)),
                DoCond::PreUntil(e) => DoCond::PreUntil(re(e)),
                DoCond::PostWhile(e) => DoCond::PostWhile(re(e)),
                DoCond::PostUntil(e) => DoCond::PostUntil(re(e)),
            }),
            body: body.into_iter().map(|s| rewrite_stmt(s, recv, fields, enums)).collect(),
        },
        Stmt::Set { name, mutable, value } => Stmt::Set { name, mutable, value: re(value) },
        Stmt::DestructureDim { names, ty, value } => {
            Stmt::DestructureDim { names, ty, value: re(value) }
        }
        Stmt::Return(e) => Stmt::Return(e.map(re)),
        // Leaves and canvas-only forms — no bare state field to rewrite. Listed
        // explicitly (no `_`) so a new statement carrying an expression or a body
        // must be handled here, not swallowed. `Draw` only appears in a canvas
        // `Draw` block, rewritten by `gui::rewrite_canvas_stmt`, never in an event.
        leaf @ (Stmt::HandleDim { .. }
        | Stmt::Break
        | Stmt::Continue
        | Stmt::Draw(_)
        // `Assert` only appears in a `Test` block, never a surface event body.
        | Stmt::Assert(_)
        | Stmt::Comment(_)
        | Stmt::LineMark(_)) => leaf,
    }
}
