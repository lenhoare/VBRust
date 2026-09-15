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
use crate::span::Span;
use crate::transpiler::{
    body_never_returns, boxed_handle_map, collect_expr_idents, decltype_rust, emit_const, emit_enum,
    emit_fn, emit_impl, emit_stmt, emit_struct, note_builtins, render_expr, render_rust_for_handles,
    rust_name, stdlib_type, with_emit_handles,
};
use std::collections::{HashMap, HashSet};

/// The program-wide lookup tables every backend builds before emitting: enum
/// names, function/method signatures, constants, and struct fields — plus, in
/// a multi-file project, the sibling module names and their harvested
/// interfaces, so a helper function or event can call `Life.StepLife(…)`
/// with the same argument treatment a plain program gets.
#[derive(Clone)]
pub(crate) struct Tables {
    pub enums: HashSet<String>,
    pub fns: resolver::FnTable,
    pub methods: resolver::MethodTable,
    pub consts: resolver::ConstMap,
    pub structs: resolver::StructTable,
    pub modules: HashSet<String>,
    pub interfaces: resolver::ProjectInterfaces,
    /// The in-block `Sub` helper names of the Screen/Window currently being
    /// emitted (snake-cased). A call to one inside an event/helper body is
    /// rewritten to a method call on the state receiver. Set per screen.
    pub screen_subs: HashSet<String>,
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
        screen_subs: HashSet::new(),
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
    // The `Log` sink helper — a surface (a `Screen` especially) leans on `Log`
    // because `Debug.Print` would corrupt its display. Emitted once here, shared
    // by all three surface emitters, only when the program logs.
    if crate::transpiler::program_uses_log(program) {
        out.push_str(crate::transpiler::LOG_HELPER);
        out.push('\n');
    }
    if crate::transpiler::program_uses_rnd(program) {
        out.push_str(crate::transpiler::RND_HELPER);
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
        if !is_main(f) && !f.gpu {
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
        if f.gpu {
            continue;
        }
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
            let (recv, method) = match &e.kind {
                ExprKind::Field(recv, m) => (recv.as_ref(), m),
                ExprKind::MethodCall { recv, method, .. } => (recv.as_ref(), method),
                _ => continue,
            };
            if !method.eq_ignore_ascii_case("run") {
                continue;
            }
            if let ExprKind::Ident(name) = &recv.kind {
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
pub(crate) fn surface_std_imports(
    events: &[GuiEvent],
    subs: &[GuiEvent],
    helpers: &[Function],
    state: &[StateField],
) -> String {
    let mut out = String::new();
    let in_event = events.iter().any(|e| crate::transpiler::body_uses_hashmap(&e.body));
    let in_sub = subs.iter().any(|s| crate::transpiler::body_uses_hashmap(&s.body));
    let in_helper = helpers.iter().any(|f| crate::transpiler::body_uses_hashmap(&f.body));
    let in_state = state.iter().any(|f| crate::transpiler::ty_uses_hashmap(&f.ty));
    if in_event || in_sub || in_helper || in_state {
        out.push_str("use std::collections::HashMap;\n");
    }
    out
}

/// True when a `State` field initialiser can fail. Ordinary Vinyl functions are
/// always `Result` internally, so any user/module call is fallible — not only
/// those declared `As Result<T>`. Stdlib constructors that return `Result`
/// (`Database.Open`, `FileSystem.Read`, …) stay on the list too.
pub(crate) fn fallible_init(e: &Expr, t: &Tables) -> bool {
    match &e.kind {
        ExprKind::Call { name, args } => {
            t.fns.contains_key(&rust_name(name)) || args.iter().any(|a| fallible_init(a, t))
        }
        ExprKind::MethodCall { recv, method, args } => {
            let m = rust_name(method);
            let stdlib_fail = match &recv.kind {
                ExprKind::Ident(r) => {
                    if let Some(canon) = stdlib_type(r) {
                        matches!(
                            (canon, m.as_str()),
                            ("Database", "open")
                                | ("Json", "parse")
                                | ("DateTime", "parse")
                                | ("FileSystem", "read")
                                | ("FileSystem", "read_lines")
                                | ("FileSystem", "list")
                                | ("FileSystem", "write")
                                | ("FileSystem", "create_folder")
                                | ("FileSystem", "create_folder_all")
                                | ("Shell", "run")
                                | ("Shell", "start")
                        )
                    } else {
                        t.interfaces
                            .get(&rust_name(r))
                            .is_some_and(|i| i.fns.contains_key(&m))
                    }
                }
                _ => false,
            };
            stdlib_fail
                || fallible_init(recv, t)
                || args.iter().any(|a| fallible_init(a, t))
        }
        ExprKind::Binary { lhs, rhs, .. } => fallible_init(lhs, t) || fallible_init(rhs, t),
        ExprKind::Field(inner, _)
        | ExprKind::Index(inner, _)
        | ExprKind::Try(inner)
        | ExprKind::Cast(inner, _)
        | ExprKind::Not(inner)
        | ExprKind::ParallelSum(inner, _)
        | ExprKind::Await(inner)
        | ExprKind::Raw(inner)
        | ExprKind::Ref(inner)
        | ExprKind::MutRef(inner)
        | ExprKind::Deref(inner) => fallible_init(inner, t),
        ExprKind::List(xs) | ExprKind::Tuple(xs) => xs.iter().any(|x| fallible_init(x, t)),
        ExprKind::ListRepeat { value, count } => fallible_init(value, t) || fallible_init(count, t),
        ExprKind::StructLit { fields, .. } => fields.iter().any(|(_, v)| fallible_init(v, t)),
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
        if let Some(Expr { kind: ExprKind::MethodCall { recv, .. }, .. }) = &f.init {
            if let ExprKind::Ident(r) = &recv.kind {
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
/// thread); the browser backends have no threads — `Http.Get` / `Http.Post`
/// map to generated wrappers over the browser's own async `fetch` instead.
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

/// Sibling of `HTTP_GET_HELPER` for `Await Http.Post` — same error shape, plus
/// a body and the request-header map the stdlib call takes.
pub(crate) const HTTP_POST_HELPER: &str = "\
/// The browser's `fetch`, shaped like the stdlib's `Http.Post`: the response
/// body on success; any failure (network, CORS, an HTTP error status) as a
/// `String` error.
async fn http_post(
    url: &str,
    body: &str,
    headers: std::collections::HashMap<String, String>,
) -> Result<String, String> {
    let mut builder = gloo_net::http::Request::post(url);
    for (name, value) in headers {
        builder = builder.header(&name, &value);
    }
    let response = builder
        .body(body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.ok() {
        return Err(format!(\"HTTP {}\", response.status()));
    }
    response.text().await.map_err(|e| e.to_string())
}

";

/// Append the Get/Post fetch wrappers a browser backend used, once each.
pub(crate) fn emit_browser_http_helpers(out: &mut String) {
    let get = out.contains("http_get(");
    let post = out.contains("http_post(");
    if get {
        out.push_str(HTTP_GET_HELPER);
    }
    if post {
        out.push_str(HTTP_POST_HELPER);
    }
}

/// Analyse every event: split each around an `Await` (None = synchronous), and
/// check that no blocking stdlib call runs un-`Await`ed (it would freeze the
/// UI). One entry per event, in order.
///
/// A `Sub` that contains `Await` (or tail-calls one that does) is flattened into
/// any Event that ends with a call to it, so the same one-cut split applies.
/// Those Subs are not emitted as methods — they only run as that Event's body.
pub(crate) fn analyze_events(
    events: &[GuiEvent],
    subs: &[GuiEvent],
    field_ty: &HashMap<String, DeclType>,
    fns: &resolver::FnTable,
    diags: &mut Diagnostics,
    backend: AsyncBackend,
    helpers: &[Function],
) -> Vec<Option<AwaitSplit>> {
    let async_names = async_sub_names(subs);
    for s in subs {
        if async_names.contains(&rust_name(&s.name))
            && s.params.iter().any(|p| p.mode == ParamMode::ByRef)
        {
            diags.error_once(
                &format!("await-sub-byref-{}", rust_name(&s.name)),
                format!(
                    "`{}` uses `Await`, so it can't take `ByRef` parameters — the resume \
                     can't hold a borrow. Pass `ByVal`, or keep the `Await` in the Event.",
                    s.name
                ),
            );
        }
    }
    for s in subs {
        if async_names.contains(&rust_name(&s.name)) {
            let mut body = s.body.clone();
            flatten_async_tails(&mut body, subs, &async_names, diags);
            reject_async_sub_calls(&body, &async_names, diags);
        } else {
            reject_async_sub_calls(&s.body, &async_names, diags);
        }
    }
    let io_helpers = io_helper_map(helpers, subs, &async_names);
    for s in subs {
        if !async_names.contains(&rust_name(&s.name)) {
            check_blocking_without_await(&s.body, &s.params, field_ty, &io_helpers, diags);
        }
    }
    events
        .iter()
        .map(|e| {
            let mut flat = e.clone();
            flatten_async_tails(&mut flat.body, subs, &async_names, diags);
            reject_async_sub_calls(&flat.body, &async_names, diags);
            check_blocking_without_await(&flat.body, &e.params, field_ty, &io_helpers, diags);
            await_split(&flat, field_ty, fns, diags, backend)
        })
        .collect()
}

/// Subs whose body contains `Await`, or that end with a call to such a Sub.
pub(crate) fn async_sub_names(subs: &[GuiEvent]) -> HashSet<String> {
    let all: HashSet<String> = subs.iter().map(|s| rust_name(&s.name)).collect();
    let mut async_set: HashSet<String> = subs
        .iter()
        .filter(|s| s.body.iter().any(stmt_has_await))
        .map(|s| rust_name(&s.name))
        .collect();
    let mut changed = true;
    while changed {
        changed = false;
        for s in subs {
            let n = rust_name(&s.name);
            if async_set.contains(&n) {
                continue;
            }
            let Some(i) = last_real_index(&s.body) else { continue };
            if let Some((callee, _)) = call_to_named_sub(&s.body[i], &all) {
                if async_set.contains(&rust_name(callee)) {
                    async_set.insert(n);
                    changed = true;
                }
            }
        }
    }
    async_set
}

fn skippable_stmt(s: &Stmt) -> bool {
    matches!(s, Stmt::Comment(_) | Stmt::LineMark(_))
}

fn last_real_index(body: &[Stmt]) -> Option<usize> {
    body.iter().rposition(|s| !skippable_stmt(s))
}

fn call_to_named_sub<'a>(s: &'a Stmt, names: &HashSet<String>) -> Option<(&'a String, &'a [Expr])> {
    match s {
        Stmt::Expr(e) => match &e.kind {
            ExprKind::Call { name, args } if names.contains(&rust_name(name)) => {
                Some((name, args.as_slice()))
            }
            _ => None,
        },
        _ => None,
    }
}

/// Replace a trailing call to an async `Sub` with that Sub's params-as-`Dim`s
/// plus its body, repeating until the tail is no longer such a call.
fn flatten_async_tails(
    body: &mut Vec<Stmt>,
    subs: &[GuiEvent],
    async_names: &HashSet<String>,
    diags: &mut Diagnostics,
) {
    let by_name: HashMap<String, &GuiEvent> =
        subs.iter().map(|s| (rust_name(&s.name), s)).collect();
    let mut seen: HashSet<String> = HashSet::new();
    loop {
        let Some(i) = last_real_index(body) else { break };
        let Some((name, args)) = call_to_named_sub(&body[i], async_names) else { break };
        let key = rust_name(name);
        let args = args.to_vec();
        let display = name.clone();
        if !seen.insert(key.clone()) {
            diags.error_once(
                "await-sub-cycle",
                format!(
                    "`{}` tail-calls itself through an `Await` chain — Vinyl keeps one \
                     `Await` per event, with no recursive async.",
                    display
                ),
            );
            break;
        }
        let Some(sub) = by_name.get(&key).copied() else { break };
        if sub.params.len() != args.len() {
            diags.error_once(
                &format!("await-sub-args-{}", key),
                format!(
                    "`{}` takes {} parameter(s), but the call has {}.",
                    sub.name,
                    sub.params.len(),
                    args.len()
                ),
            );
            break;
        }
        // Same-name `DoFetch(picked)` / `Sub DoFetch(ByVal picked)` would emit
        // `Dim picked = picked`, which is the unknown-size copy error. The
        // caller's local (or state field) is already that name — skip the bind.
        let mut replacement: Vec<Stmt> = Vec::new();
        for (p, a) in sub.params.iter().zip(args) {
            if let ExprKind::Ident(name) = &a.kind {
                if rust_name(name) == rust_name(&p.name) {
                    continue;
                }
            }
            replacement.push(Stmt::Dim {
                name: p.name.clone(),
                name_span: Span::none(),
                ty: p.ty.clone(),
                init: Some(a),
                deferred: false,
                line: 0,
            });
        }
        replacement.extend(sub.body.iter().cloned());
        let trailing = body.split_off(i);
        body.extend(replacement);
        body.extend(trailing.into_iter().skip(1));
    }
}

fn reject_async_sub_calls(stmts: &[Stmt], async_names: &HashSet<String>, diags: &mut Diagnostics) {
    for s in stmts {
        walk_stmt_async_calls(s, async_names, diags);
    }
}

fn walk_expr_async_calls(e: &Expr, async_names: &HashSet<String>, diags: &mut Diagnostics) {
    match &e.kind {
        ExprKind::Call { name, args } => {
            if async_names.contains(&rust_name(name)) {
                diags.error_once(
                    &format!("await-sub-tail-{}", rust_name(name)),
                    format!(
                        "`{}` uses `Await`, so a call to it must be the last statement of an \
                         Event (or of another Sub that itself ends with that call). Vinyl keeps \
                         one `Await` per event — code after the call would need a second resume.",
                        name
                    ),
                );
            }
            for a in args {
                walk_expr_async_calls(a, async_names, diags);
            }
        }
        ExprKind::Await(i)
        | ExprKind::Not(i)
        | ExprKind::ParallelSum(i, _)
        | ExprKind::Ref(i)
        | ExprKind::MutRef(i)
        | ExprKind::Deref(i)
        | ExprKind::Cast(i, _)
        | ExprKind::Try(i)
        | ExprKind::Field(i, _)
        | ExprKind::TupleIndex(i, _)
        | ExprKind::Closure { body: i, .. }
        | ExprKind::Raw(i) => walk_expr_async_calls(i, async_names, diags),
        ExprKind::Binary { lhs, rhs, .. }
        | ExprKind::Index(lhs, rhs)
        | ExprKind::ListRepeat { value: lhs, count: rhs } => {
            walk_expr_async_calls(lhs, async_names, diags);
            walk_expr_async_calls(rhs, async_names, diags);
        }
        ExprKind::MethodCall { recv, args, .. } => {
            walk_expr_async_calls(recv, async_names, diags);
            for a in args {
                walk_expr_async_calls(a, async_names, diags);
            }
        }
        ExprKind::Tuple(es) | ExprKind::List(es) => {
            for e2 in es {
                walk_expr_async_calls(e2, async_names, diags);
            }
        }
        ExprKind::StructLit { fields, .. } => {
            for (_, v) in fields {
                walk_expr_async_calls(v, async_names, diags);
            }
        }
        _ => {}
    }
}

fn walk_stmt_async_calls(s: &Stmt, async_names: &HashSet<String>, diags: &mut Diagnostics) {
    match s {
        Stmt::Dim { init: Some(e), .. }
        | Stmt::Print(e)
        | Stmt::Log(_, e)
        | Stmt::Expr(e)
        | Stmt::Return(Some(e))
        | Stmt::RaiseError(e)
        | Stmt::Assert(e)
        | Stmt::Set { value: e, .. }
        | Stmt::DestructureDim { value: e, .. } => walk_expr_async_calls(e, async_names, diags),
        Stmt::Assign { target, value, .. } => {
            walk_expr_async_calls(target, async_names, diags);
            walk_expr_async_calls(value, async_names, diags);
        }
        Stmt::HandleErr { target, call, body, .. } => {
            if let Some(t) = target {
                walk_expr_async_calls(t, async_names, diags);
            }
            walk_expr_async_calls(call, async_names, diags);
            reject_async_sub_calls(body, async_names, diags);
        }
        Stmt::If { branches, else_body } => {
            for (c, b) in branches {
                walk_expr_async_calls(c, async_names, diags);
                reject_async_sub_calls(b, async_names, diags);
            }
            if let Some(b) = else_body {
                reject_async_sub_calls(b, async_names, diags);
            }
        }
        Stmt::Match { scrutinee, arms, .. } => {
            walk_expr_async_calls(scrutinee, async_names, diags);
            for a in arms {
                if let Some(g) = &a.guard {
                    walk_expr_async_calls(g, async_names, diags);
                }
                reject_async_sub_calls(&a.body, async_names, diags);
            }
        }
        Stmt::For { from, to, step, body, .. } => {
            walk_expr_async_calls(from, async_names, diags);
            walk_expr_async_calls(to, async_names, diags);
            if let Some(st) = step {
                walk_expr_async_calls(st, async_names, diags);
            }
            reject_async_sub_calls(body, async_names, diags);
        }
        Stmt::ForEach { iter, body, .. } => {
            walk_expr_async_calls(iter, async_names, diags);
            reject_async_sub_calls(body, async_names, diags);
        }
        Stmt::DoLoop { cond, body } => {
            if let Some(c) = cond {
                match c {
                    DoCond::PreWhile(e)
                    | DoCond::PreUntil(e)
                    | DoCond::PostWhile(e)
                    | DoCond::PostUntil(e) => walk_expr_async_calls(e, async_names, diags),
                }
            }
            reject_async_sub_calls(body, async_names, diags);
        }
        Stmt::GpuInto { body, .. } => reject_async_sub_calls(body, async_names, diags),
        _ => {}
    }
}

/// `Await` in a Function / Draw / Godot body of a surface program is a lie: it
/// would strip to a blocking call on the UI thread. Console `Main` may `Await`
/// (there is no UI thread — it just runs the call).
pub(crate) fn check_await_honesty(program: &Program, diags: &mut Diagnostics) {
    let surface = !program.windows.is_empty()
        || !program.screens.is_empty()
        || !program.pages.is_empty()
        || !program.sketches.is_empty()
        || !program.godot_nodes.is_empty();
    if !surface {
        return;
    }
    for f in &program.functions {
        if f.body.iter().any(stmt_has_await) {
            diags.error_once(
                "await-in-function",
                "`Await` inside a Function would run on the UI thread and freeze the window. \
                 Put `Await` in an Event, or in a `Sub` that an Event ends with. To run this \
                 Function off-thread, write `Match Await Load(…)` in the Event and keep the \
                 Function itself synchronous.",
            );
        }
    }
    for c in &program.canvases {
        if c.body.iter().any(stmt_has_await) {
            diags.error_once(
                "await-in-draw",
                "`Await` belongs in an Event (or a `Sub` the Event ends with), not in `Draw`.",
            );
        }
    }
    for s in &program.sketches {
        if s.draw.iter().any(stmt_has_await)
            || s.gpu_draw.as_ref().is_some_and(|b| b.iter().any(stmt_has_await))
        {
            diags.error_once(
                "await-in-draw",
                "`Await` belongs in an Event (or a `Sub` the Event ends with), not in `Draw`.",
            );
        }
    }
    for n in &program.godot_nodes {
        if n.events.iter().any(|e| e.body.iter().any(stmt_has_await))
            || n.handlers.iter().any(|e| e.body.iter().any(stmt_has_await))
        {
            diags.error_once(
                "await-in-godot",
                "`Await` isn't available in a Godot `On` handler yet — Godot drives those \
                 callbacks, and they have no off-thread resume. Keep the body quick, or do \
                 the slow work before the node starts.",
            );
        }
    }
}

/// The stdlib namespaces used across all event bodies, sorted and deduped —
/// ready for a `use vbr_stdlib::{…}` line. Marks each so the vbr_stdlib dep and
/// feature get added. (The web backend collects without marking — see
/// `collect_event_stdlib` — since its `Http` is the browser's fetch, not ours.)
pub(crate) fn event_stdlib_imports(
    events: &[GuiEvent],
    subs: &[GuiEvent],
    diags: &mut Diagnostics,
) -> Vec<String> {
    let mut used: Vec<String> = Vec::new();
    for e in events {
        collect_event_stdlib(&e.body, &mut used);
    }
    for s in subs {
        collect_event_stdlib(&s.body, &mut used);
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
    let passed_by_ref = resolver::resolve_event_body(
        &mut body, params, field_ty, &t.fns, &t.methods, &t.consts, &t.modules, &t.interfaces,
        &t.enums, &t.structs, diags,
    );
    // A `Dim`'d For counter would be shadowed by the loop's own binding —
    // drop the dead `let`, exactly as in a plain function body.
    crate::transpiler::elide_for_counter_dims(&mut body);
    let mut mutated: HashSet<String> = HashSet::new();
    crate::transpiler::collect_mutated(&body, &mut mutated);
    mutated.extend(passed_by_ref);
    let empty: HashSet<String> = HashSet::new();
    let mut handles = boxed_handle_map(params, &body);
    for (name, ty) in field_ty {
        if matches!(ty, DeclType::Handle) {
            handles.insert(name.clone(), format!("{recv}.{name}"));
        }
    }
    for p in params {
        if matches!(p.ty, DeclType::Handle) {
            let n = rust_name(&p.name);
            handles.insert(n.clone(), n);
        }
    }
    with_emit_handles(handles, false, || {
        for stmt in body {
            // The rewrite turns state fields into `recv.field` and a call to an
            // in-block `Sub` helper into `recv.helper(...)` (`t.screen_subs`).
            let mut rewritten = rewrite_stmt(stmt, recv, fields, &t.enums, &t.screen_subs);
            coerce_state_strings(&mut rewritten, recv, field_ty);
            emit_stmt(&rewritten, &mutated, &empty, indent, diags, out);
        }
    });
}

/// Run an event body so an unhandled error ends the event, not the app.
pub(crate) fn emit_event_stmts_caught(
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
    let pad = "    ".repeat(indent);
    out.push_str(&format!("{}{{\n", pad));
    out.push_str(&format!("{}    let __vbr_event: Result<(), String> = (|| {{\n", pad));
    emit_event_stmts(stmts, params, recv, fields, field_ty, t, indent + 2, diags, out);
    if !body_never_returns(stmts) {
        out.push_str(&format!("{}        Ok(())\n", pad));
    }
    out.push_str(&format!("{}    }})();\n", pad));
    out.push_str(&format!(
        "{}    if let Err(__e) = __vbr_event {{\n        {}eprintln!(\"Error: {{}}\", __e);\n{}    }}\n",
        pad, pad, pad
    ));
    out.push_str(&format!("{}}}\n", pad));
}

/// Emit the pre-await half of an async event, then `emit_spawn`.
///
/// `Dim` locals used by the awaited call (a JSON body, a header map) have to
/// outlive the error-catching closure so the spawn / `send_future` that
/// follows can move them. When `carry` is empty this is the usual caught
/// pre-body plus `emit_spawn`. When it isn't, the closure returns those
/// locals on success. `err_tail` is extra code for the failure arm — a GUI
/// kick-off passes `Task::none()` so `update` still returns a Task; a Page
/// or Screen leaves it empty.
pub(crate) fn emit_async_kickoff(
    pre: &[Stmt],
    params: &[Param],
    recv: &'static str,
    fields: &HashSet<String>,
    field_ty: &HashMap<String, DeclType>,
    t: &Tables,
    indent: usize,
    diags: &mut Diagnostics,
    out: &mut String,
    carry: &[String],
    err_tail: &str,
    emit_spawn: impl FnOnce(&mut String, &mut Diagnostics),
) {
    if carry.is_empty() {
        emit_event_stmts_caught(pre, params, recv, fields, field_ty, t, indent, diags, out);
        emit_spawn(out, diags);
        return;
    }
    let pad = "    ".repeat(indent);
    let inner = "    ".repeat(indent + 1);
    let inner2 = "    ".repeat(indent + 2);
    let tuple = match carry.len() {
        1 => carry[0].clone(),
        _ => format!("({})", carry.join(", ")),
    };
    out.push_str(&format!("{}{{\n", pad));
    out.push_str(&format!("{}let __vbr_event: Result<_, String> = (|| {{\n", inner));
    emit_event_stmts(pre, params, recv, fields, field_ty, t, indent + 2, diags, out);
    if !body_never_returns(pre) {
        out.push_str(&format!("{}Ok({})\n", inner2, tuple));
    }
    out.push_str(&format!("{}}})();\n", inner));
    out.push_str(&format!("{}match __vbr_event {{\n", inner));
    out.push_str(&format!("{}Err(__e) => {{\n", inner2));
    out.push_str(&format!(
        "{}    eprintln!(\"Error: {{}}\", __e);\n",
        inner2
    ));
    if !err_tail.is_empty() {
        out.push_str(&format!("{}    {}\n", inner2, err_tail));
    }
    out.push_str(&format!("{}}}\n", inner2));
    out.push_str(&format!("{}Ok({}) => {{\n", inner2, tuple));
    emit_spawn(out, diags);
    out.push_str(&format!("{}}}\n", inner2));
    out.push_str(&format!("{}}}\n", inner));
    out.push_str(&format!("{}}}\n", pad));
}

/// `Dim` names declared in `pre` that the awaited call reads — those bindings
/// must leave the kick-off closure so the spawn can move them.
fn carry_dims(call: &Expr, pre: &[Stmt]) -> Vec<String> {
    let dims: HashSet<String> = pre
        .iter()
        .filter_map(|s| match s {
            Stmt::Dim { name, .. } => Some(rust_name(name)),
            _ => None,
        })
        .collect();
    let mut used = HashSet::new();
    collect_expr_idents(call, &mut used);
    let mut carry: Vec<String> = used.into_iter().filter(|n| dims.contains(n)).collect();
    carry.sort();
    carry
}

/// Build a per-screen view of the tables: `screen_subs` filled with this block's
/// `Sub` helper names, and each helper registered in the fn table so a call to it
/// gets the usual argument coercion (borrow a String, widen a number) before it's
/// rewritten to a method on the state receiver.
pub(crate) fn with_subs(base: &Tables, subs: &[GuiEvent]) -> Tables {
    let async_names = async_sub_names(subs);
    let mut t = base.clone();
    for s in subs {
        let n = rust_name(&s.name);
        t.fns.insert(
            n.clone(),
            resolver::FnSig {
                modes: s.params.iter().map(|p| p.mode).collect(),
                param_types: s.params.iter().map(|p| p.ty.clone()).collect(),
                ret: None,
            },
        );
        // An async Sub is inlined into the Event that tail-calls it — not a
        // method on the state. Leaving it out of `screen_subs` means a leftover
        // call isn't rewritten to a missing `state.foo()`.
        if !async_names.contains(&n) {
            t.screen_subs.insert(n);
        }
    }
    t
}

/// Emit each in-block `Sub` as a method on the state struct — direct `self.field`
/// access, callable from events and other helpers. `ty` is the state struct name.
pub(crate) fn emit_subs(
    subs: &[GuiEvent],
    ty: &str,
    fields: &HashSet<String>,
    field_ty: &HashMap<String, DeclType>,
    t: &Tables,
    diags: &mut Diagnostics,
    out: &mut String,
) {
    if subs.is_empty() {
        return;
    }
    let async_names = async_sub_names(subs);
    let sync: Vec<&GuiEvent> = subs
        .iter()
        .filter(|s| !async_names.contains(&rust_name(&s.name)))
        .collect();
    if sync.is_empty() {
        return;
    }
    out.push_str(&format!("impl {} {{\n", ty));
    for s in sync {
        let params: Vec<String> = s
            .params
            .iter()
            .map(|p| crate::transpiler::render_param_ty(p, Some(&t.enums)))
            .collect();
        let sep = if params.is_empty() { "" } else { ", " };
        out.push_str(&format!(
            "    fn {}(&mut self{}{}) -> Result<(), String> {{\n",
            rust_name(&s.name),
            sep,
            params.join(", ")
        ));
        emit_event_stmts(&s.body, &s.params, "self", fields, field_ty, t, 2, diags, out);
        if !body_never_returns(&s.body) {
            out.push_str("        Ok(())\n");
        }
        out.push_str("    }\n");
    }
    out.push_str("}\n\n");
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
    if let ExprKind::Ident(name) = &scrutinee.kind {
        if matches!(field_ty.get(&rust_name(name)), Some(DeclType::Plain(Type::Text))) {
            return format!("{}.as_str()", rendered);
        }
    }
    rendered
}

/// A `State` field initialiser: a `String` becomes owned, numbers adapt to type,
/// an enum variant (`Size.Small`) resolves to its path (`Size::Small`), and a
/// `Vec` / `HashMap` / `Option` / fixed array with no initialiser starts empty.
///
/// The initialiser first runs the ordinary resolver pass (as a synthetic `Dim`
/// of the field's type) — an initialiser and a function-body `Dim` are the same
/// language, so a call initialiser gets the same argument treatment (`&` on a
/// ByVal collection, owned strings, numeric casts) it would get anywhere else.
pub(crate) fn render_init(
    init: Option<&Expr>,
    ty: &DeclType,
    t: &Tables,
    prior: &HashMap<String, DeclType>,
    diags: &mut Diagnostics,
) -> String {
    let init = init.map(|e| {
        let mut body = vec![Stmt::Dim {
            name: "field".to_string(),
            name_span: crate::span::Span::none(),
            ty: ty.clone(),
            init: Some(e.clone()),
            deferred: false,
            line: 0,
        }];
        // `prior` are the earlier `State` fields, seeded like state so a later
        // initialiser can read one (`rects = LiveRects(grid, …)`) and it's typed
        // against it — borrowed as `&grid` for a ByVal collection, not moved.
        resolver::resolve_event_body(
            &mut body, &[], prior, &t.fns, &t.methods, &t.consts, &t.modules,
            &t.interfaces, &t.enums, &t.structs, diags,
        );
        match body.pop() {
            Some(Stmt::Dim { init: Some(e), .. }) => e,
            _ => e.clone(),
        }
    });
    match (ty, init) {
        (DeclType::Vec(_), None) => "Vec::new()".to_string(),
        (DeclType::Map(..), None) => "HashMap::new()".to_string(),
        (DeclType::Option(_), None) => "None".to_string(),
        (DeclType::CudaBuffer(_), None) => "__vbr_cuda_alloc(0)".to_string(),
        (DeclType::Array(t, n), None) => {
            let d = crate::transpiler::array_default(*t);
            format!("[{}; {}]", d, n)
        }
        (DeclType::Array2D(t, r, c), None) => {
            let d = crate::transpiler::array_default(*t);
            format!("[[{}; {}]; {}]", d, c, r)
        }
        // A bare string literal still needs owning; anything else the resolver
        // has already made owned where needed.
        (DeclType::Plain(Type::Text), Some(e)) if matches!(e.kind, ExprKind::Str(_)) => {
            format!("{}.to_string()", render_expr(&e, None))
        }
        (DeclType::Plain(Type::Text), Some(e)) => render_expr(&e, None),
        (DeclType::Plain(t), Some(e)) => render_expr(&e, Some(*t)),
        (DeclType::Handle, Some(e)) => match &e.kind {
            ExprKind::InlineRust(raw) => render_rust_for_handles(raw, 0, true),
            _ => render_expr(&e, None),
        },
        // Enum / Vec-with-initialiser / other — the resolver has rewritten
        // `Size.Small` → `Size::Small` and referenced call arguments.
        (_, Some(e)) => render_expr(&e, None),
        (_, None) => "Default::default()".to_string(),
    }
}

/// Emit each `State` initialiser as a `let` binding, in declaration order, so a
/// later field can read an earlier one (the struct that follows is built from the
/// returned names, by field-init shorthand). A fallible initialiser gets `?`.
/// `override_init(f)` lets a surface substitute a field's init string (the GUI's
/// `TextEditor` content) — return `None` for the normal `render_init` path.
pub(crate) fn emit_state_lets(
    state: &[StateField],
    t: &Tables,
    indent: usize,
    diags: &mut Diagnostics,
    out: &mut String,
    mut override_init: impl FnMut(&StateField) -> Option<String>,
) -> Vec<String> {
    let pad = "    ".repeat(indent);
    let mut prior: HashMap<String, DeclType> = HashMap::new();
    let mut names = Vec::new();
    let mut handle_qual: HashMap<String, String> = HashMap::new();
    for f in state {
        let init = match override_init(f) {
            Some(s) => s,
            None => with_emit_handles(handle_qual.clone(), false, || {
                render_init(f.init.as_ref(), &f.ty, t, &prior, diags)
            }),
        };
        // `?` comes from the resolver's auto-try inside `render_init`. `fallible_init`
        // only chooses `init()` vs `Default` — pushing another `?` here made `??`.
        let name = rust_name(&f.name);
        out.push_str(&format!("{}let {} = {};\n", pad, name, init));
        if matches!(f.ty, DeclType::Handle) {
            handle_qual.insert(name.clone(), name.clone());
        }
        prior.insert(name.clone(), f.ty.clone());
        names.push(name);
    }
    names
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
        Stmt::Assign { target: Expr { kind: ExprKind::Field(recv, fname), .. }, value, .. }
            if matches!(&recv.kind, ExprKind::Ident(n) if n == state_recv)
                && matches!(field_ty.get(&rust_name(fname)), Some(DeclType::Plain(Type::Text)))
                && matches!(&value.kind, ExprKind::Str(_)) =>
        {
            let inner = std::mem::replace(&mut value.kind, ExprKind::Int(0)).at(value.span);
            value.kind = ExprKind::MethodCall {
                recv: Box::new(inner),
                method: "to_string".to_string(),
                args: Vec::new(),
            };
        }
        // `draft = text` on a TextArea field — Iced holds `text_editor::Content`,
        // not a String. Wrap the RHS so a file's text (or a literal) loads.
        // `with_text` takes `&str` (iced 0.13), so borrow the RHS.
        Stmt::Assign { target: Expr { kind: ExprKind::Field(recv, fname), .. }, value, .. }
            if matches!(&recv.kind, ExprKind::Ident(n) if n == state_recv)
                && matches!(field_ty.get(&rust_name(fname)), Some(DeclType::Named(n)) if n == "TextArea") =>
        {
            let span = value.span;
            let inner = std::mem::replace(&mut value.kind, ExprKind::Int(0)).at(span);
            value.kind = ExprKind::Call {
                name: "iced::widget::text_editor::Content::with_text".to_string(),
                args: vec![ExprKind::Ref(Box::new(inner)).at(span)],
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
        Stmt::HandleErr { body, .. } => {
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
        | Stmt::Destroy { .. }
        | Stmt::DestructureDim { .. }
        | Stmt::HandleDim { .. }
        | Stmt::Return(_)
        | Stmt::RaiseError(_)
        | Stmt::Expr(_)
        | Stmt::Print(_)
        | Stmt::Log(..)
        | Stmt::Break
        | Stmt::Continue
        | Stmt::Draw(_)
        | Stmt::GpuInto { .. }
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
    pub(crate) carry: Vec<String>,     // Dim locals the spawn must take (body, headers, …)
}

/// What we need to know about an awaited stdlib call.
struct AwaitInfo {
    snapshots: Vec<String>,
    call_src: String,
    ret_type: String,
    blocking: bool,
}

const AWAIT_POSITION: &str = "`Await` must be a *top-level* statement in an event — the value of a `Match` \
     (`Match Await Http.Get(url)`) or a `Dim` (`Dim x = Await …`), not nested inside \
     an `If`/`For`/`Match`. To guard the call, put the check *before* the `Await` \
     (`If busy Then Return` / set a flag first), or move it into the awaited helper \
     (return early on the guard). Vinyl keeps async deliberately simple: one `Await` \
     per event, at the top.";

fn report_await_position(diags: &mut Diagnostics) {
    diags.error_once("await-position", AWAIT_POSITION);
}

fn more_than_one_await(cont: &[Stmt], diags: &mut Diagnostics) -> bool {
    if cont.iter().any(stmt_has_await) {
        report_await_position(diags);
        true
    } else {
        false
    }
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
        Stmt::Match { scrutinee: Expr { kind: ExprKind::Await(call), .. }, arms, line, if_let } => {
            let info = awaitable_info(call, field_ty, &locals, fns, diags, backend)?;
            // Continuation runs `match result { <arms> }`, then any trailing code.
            let mut cont = vec![Stmt::Match {
                scrutinee: ExprKind::Ident("result".to_string()).synth(),
                arms: arms.clone(),
                line: *line,
                if_let: *if_let,
            }];
            cont.extend(e.body[idx + 1..].iter().cloned());
            if more_than_one_await(&cont, diags) {
                return None;
            }
            let pre = e.body[..idx].to_vec();
            let carry = carry_dims(call, &pre);
            Some(AwaitSplit {
                pre,
                snapshots: info.snapshots,
                call_src: info.call_src,
                ret_type: info.ret_type,
                blocking: info.blocking,
                bind: "result".to_string(),
                cont,
                carry,
            })
        }
        Stmt::Dim { name, init: Some(Expr { kind: ExprKind::Await(call), .. }), .. } => {
            let info = awaitable_info(call, field_ty, &locals, fns, diags, backend)?;
            let pre = e.body[..idx].to_vec();
            let carry = carry_dims(call, &pre);
            let cont = e.body[idx + 1..].to_vec();
            if more_than_one_await(&cont, diags) {
                return None;
            }
            Some(AwaitSplit {
                pre,
                snapshots: info.snapshots,
                call_src: info.call_src,
                ret_type: info.ret_type,
                blocking: info.blocking,
                bind: rust_name(name),
                cont,
                carry,
            })
        }
        _ => {
            report_await_position(diags);
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
        match &a.kind {
            ExprKind::Ident(name) if field_ty.contains_key(&rust_name(name)) => {
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
            ExprKind::Ident(name) if matches!(locals.get(&rust_name(name)), Some(DeclType::Plain(Type::Text))) => {
                arg_src.push(format!("&{}", rust_name(name)));
            }
            _ => arg_src.push(render_expr(a, None)),
        }
    }
    (snapshots, arg_src)
}

/// Resolve an awaited call to its Rust form, result type, and how to run it: a
/// known stdlib call (`Http.Get`), or one of the program's own functions (whose
/// return type the `FnTable` records). Natively both run off the UI thread; on
/// the web `Http.Get` / `Http.Post` map to generated fetch wrappers instead
/// (the browser is single-threaded — its HTTP is async by nature).
fn awaitable_info(
    call: &Expr,
    field_ty: &HashMap<String, DeclType>,
    locals: &HashMap<String, DeclType>,
    fns: &resolver::FnTable,
    diags: &mut Diagnostics,
    backend: AsyncBackend,
) -> Option<AwaitInfo> {
    match &call.kind {
        // A stdlib call: `Http.Get(url)`.
        ExprKind::MethodCall { recv, method, args } => {
            let canon = match &(&**recv).kind {
                ExprKind::Ident(r) => stdlib_type(r),
                _ => None,
            };
            let Some(canon) = canon else {
                diags.error_once(
                    "await-not-awaitable",
                    "`Await` works on a stdlib call (`Http.Get(url)`, `Http.Post(url, body, headers)`) \
                     or one of your own functions.",
                );
                return None;
            };
            let m = rust_name(method);
            if backend.is_browser() {
                let helper = match (canon, m.as_str()) {
                    ("Http", "get") => "http_get",
                    ("Http", "post") => "http_post",
                    _ => {
                        diags.error_once(
                            "await-unsupported",
                            format!(
                                "`Await {}.{}` isn't supported in {} yet — it awaits \
                                 `Http.Get` or `Http.Post` (the browser's fetch).",
                                canon,
                                method,
                                backend.surface_name()
                            ),
                        );
                        return None;
                    }
                };
                // No vbr_stdlib on wasm — the call goes to a generated wrapper
                // over the browser's fetch (gloo-net).
                let (snapshots, arg_src) = snapshot_args(args, field_ty, locals, backend.recv());
                return Some(AwaitInfo {
                    snapshots,
                    call_src: format!("{helper}({})", arg_src.join(", ")),
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
                            "`Await {}.{}` isn't supported yet — V1 awaits `Http.Get`, \
                             `Http.Post`, `Shell.Run`, or your own functions.",
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
        ExprKind::Call { name, args } => {
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
            let ret_type = format!("Result<{}, String>", decltype_rust(dt));
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

/// How a blocking call can leave the UI thread.
#[derive(Clone, Copy)]
enum BlockKind {
    /// `Await Http.Get` / `Http.Post` / `Shell.Run` — the Event can Await it directly.
    DirectAwait,
    /// Disk / SQLite / CSV — not in the V1 Await-stdlib set (a live `Database`
    /// handle isn't `Send`). Put the work in a Function and Await that Function.
    ExtractFn,
}

fn filesystem_disk(method: &str) -> bool {
    matches!(
        method,
        "read"
            | "read_lines"
            | "write"
            | "append"
            | "copy"
            | "move_file"
            | "delete"
            | "create_folder"
            | "create_folder_all"
            | "exists"
            | "folder_exists"
            | "list"
            | "delete_folder"
            | "delete_folder_all"
    )
}

fn blocking_kind_of(e: &Expr, env: &HashMap<String, DeclType>) -> Option<(BlockKind, String)> {
    let ExprKind::MethodCall { recv, method, .. } = &e.kind else {
        return None;
    };
    let m = rust_name(method);
    let ExprKind::Ident(r) = &(&**recv).kind else {
        return None;
    };
    if let Some(c) = stdlib_type(r) {
        let kind = match (c, m.as_str()) {
            ("Http", "get") | ("Http", "post") | ("Shell", "run") => Some(BlockKind::DirectAwait),
            ("FileSystem", meth) if filesystem_disk(meth) => Some(BlockKind::ExtractFn),
            ("Database", "open") => Some(BlockKind::ExtractFn),
            ("DataFrame", "read_csv") | ("DataFrame", "write_csv") => Some(BlockKind::ExtractFn),
            _ => None,
        };
        return kind.map(|k| (k, format!("{}.{}", c, method)));
    }
    if let Some(DeclType::Named(n)) = env.get(&rust_name(r)) {
        let kind = match (n.as_str(), m.as_str()) {
            ("Database", "execute") | ("Database", "query") => Some(BlockKind::ExtractFn),
            ("DataFrame", "write_csv") => Some(BlockKind::ExtractFn),
            _ => None,
        };
        return kind.map(|k| (k, format!("{}.{}", n, method)));
    }
    None
}

fn io_helper_map<'a>(
    helpers: &'a [Function],
    subs: &'a [GuiEvent],
    async_names: &HashSet<String>,
) -> HashMap<String, (&'a [Param], &'a [Stmt])> {
    let mut m = HashMap::new();
    for f in helpers {
        m.insert(rust_name(&f.name), (f.params.as_slice(), f.body.as_slice()));
    }
    for s in subs {
        let n = rust_name(&s.name);
        if !async_names.contains(&n) {
            m.insert(n, (s.params.as_slice(), s.body.as_slice()));
        }
    }
    m
}

struct IoWalk<'a> {
    helpers: &'a HashMap<String, (&'a [Param], &'a [Stmt])>,
    memo: HashMap<String, bool>,
    visiting: HashSet<String>,
}

impl<'a> IoWalk<'a> {
    fn helper_does_io(&mut self, name: &str) -> bool {
        let key = rust_name(name);
        if let Some(&b) = self.memo.get(&key) {
            return b;
        }
        if !self.visiting.insert(key.clone()) {
            return false;
        }
        let does = if let Some((params, body)) = self.helpers.get(&key).copied() {
            let mut env: HashMap<String, DeclType> = params
                .iter()
                .map(|p| (rust_name(&p.name), p.ty.clone()))
                .collect();
            self.stmts_do_io(body, &mut env)
        } else {
            false
        };
        self.visiting.remove(&key);
        self.memo.insert(key, does);
        does
    }

    fn stmts_do_io(&mut self, stmts: &[Stmt], env: &mut HashMap<String, DeclType>) -> bool {
        stmts.iter().any(|s| self.stmt_does_io(s, env))
    }

    fn stmt_does_io(&mut self, s: &Stmt, env: &mut HashMap<String, DeclType>) -> bool {
        match s {
            Stmt::Dim { name, ty, init, .. } => {
                let hit = init.as_ref().is_some_and(|e| self.expr_does_io(e, env));
                env.insert(rust_name(name), ty.clone());
                hit
            }
            Stmt::Assign { target, value, .. } => {
                self.expr_does_io(target, env) || self.expr_does_io(value, env)
            }
            Stmt::Set { value: e, .. }
            | Stmt::DestructureDim { value: e, .. }
            | Stmt::Print(e)
            | Stmt::Log(_, e)
            | Stmt::Expr(e)
            | Stmt::Return(Some(e))
            | Stmt::RaiseError(e)
            | Stmt::Assert(e) => self.expr_does_io(e, env),
            Stmt::HandleErr { target, call, body, .. } => {
                target.as_ref().is_some_and(|t| self.expr_does_io(t, env))
                    || self.expr_does_io(call, env)
                    || self.stmts_do_io(body, env)
            }
            Stmt::If { branches, else_body } => {
                branches.iter().any(|(c, b)| self.expr_does_io(c, env) || self.stmts_do_io(b, env))
                    || else_body.as_ref().is_some_and(|b| self.stmts_do_io(b, env))
            }
            Stmt::Match { scrutinee, arms, .. } => {
                self.expr_does_io(scrutinee, env)
                    || arms.iter().any(|a| {
                        a.guard.as_ref().is_some_and(|g| self.expr_does_io(g, env))
                            || self.stmts_do_io(&a.body, env)
                    })
            }
            Stmt::For { from, to, step, body, .. } => {
                self.expr_does_io(from, env)
                    || self.expr_does_io(to, env)
                    || step.as_ref().is_some_and(|st| self.expr_does_io(st, env))
                    || self.stmts_do_io(body, env)
            }
            Stmt::ForEach { iter, body, .. } => {
                self.expr_does_io(iter, env) || self.stmts_do_io(body, env)
            }
            Stmt::DoLoop { cond, body } => {
                cond.as_ref().is_some_and(|c| match c {
                    DoCond::PreWhile(e)
                    | DoCond::PreUntil(e)
                    | DoCond::PostWhile(e)
                    | DoCond::PostUntil(e) => self.expr_does_io(e, env),
                }) || self.stmts_do_io(body, env)
            }
            Stmt::GpuInto { body, .. } => self.stmts_do_io(body, env),
            _ => false,
        }
    }

    fn expr_does_io(&mut self, e: &Expr, env: &HashMap<String, DeclType>) -> bool {
        if let ExprKind::Await(inner) = &e.kind {
            return self.expr_children_do_io(inner, env);
        }
        if blocking_kind_of(e, env).is_some() {
            return true;
        }
        if let ExprKind::Call { name, args } = &e.kind {
            if self.helper_does_io(name) {
                return true;
            }
            return args.iter().any(|a| self.expr_does_io(a, env));
        }
        self.expr_children_do_io(e, env)
    }

    fn expr_children_do_io(&mut self, e: &Expr, env: &HashMap<String, DeclType>) -> bool {
        match &e.kind {
            ExprKind::Await(i)
            | ExprKind::Not(i)
            | ExprKind::ParallelSum(i, _)
            | ExprKind::Ref(i)
            | ExprKind::MutRef(i)
            | ExprKind::Deref(i)
            | ExprKind::Cast(i, _)
            | ExprKind::Try(i)
            | ExprKind::Raw(i)
            | ExprKind::Field(i, _)
            | ExprKind::TupleIndex(i, _)
            | ExprKind::Closure { body: i, .. } => self.expr_does_io(i, env),
            ExprKind::Binary { lhs, rhs, .. }
            | ExprKind::Index(lhs, rhs)
            | ExprKind::ListRepeat { value: lhs, count: rhs } => {
                self.expr_does_io(lhs, env) || self.expr_does_io(rhs, env)
            }
            ExprKind::MethodCall { recv, args, .. } => {
                self.expr_does_io(recv, env) || args.iter().any(|a| self.expr_does_io(a, env))
            }
            ExprKind::Call { args, .. } | ExprKind::Tuple(args) | ExprKind::List(args) => {
                args.iter().any(|a| self.expr_does_io(a, env))
            }
            ExprKind::StructLit { fields, .. } => {
                fields.iter().any(|(_, v)| self.expr_does_io(v, env))
            }
            _ => false,
        }
    }
}

/// Teaching diagnostic: blocking I/O in an event (or a sync helper it calls)
/// without `Await` would freeze the window.
fn check_blocking_without_await(
    stmts: &[Stmt],
    params: &[Param],
    field_ty: &HashMap<String, DeclType>,
    helpers: &HashMap<String, (&[Param], &[Stmt])>,
    diags: &mut Diagnostics,
) {
    let mut env = field_ty.clone();
    for p in params {
        env.insert(rust_name(&p.name), p.ty.clone());
    }
    let mut walk = IoWalk {
        helpers,
        memo: HashMap::new(),
        visiting: HashSet::new(),
    };
    fn report_block(kind: BlockKind, label: &str, diags: &mut Diagnostics) {
        match kind {
            BlockKind::DirectAwait => diags.error_once(
                "blocking-no-await",
                "This stdlib call waits for I/O, so calling it directly in an event would \
                 freeze the UI until it finishes. Use `Await` so it runs off the UI thread \
                 — e.g. `Match Await Http.Get(url) … End Match`.",
            ),
            BlockKind::ExtractFn => diags.error_once(
                &format!("blocking-extract-{}", rust_name(label)),
                format!(
                    "`{}` waits for I/O, so calling it in an event would freeze the UI. \
                     Put the work in a Function and `Match Await Load(…)` from the Event \
                     — Vinyl runs that Function off the UI thread. (`Await {}` isn't a \
                     stdlib form; Await your function, or `Http.Get` / `Http.Post` / `Shell.Run`.)",
                    label, label
                ),
            ),
        }
    }
    fn ex(
        e: &Expr,
        awaited: bool,
        env: &HashMap<String, DeclType>,
        walk: &mut IoWalk,
        diags: &mut Diagnostics,
    ) {
        if let ExprKind::Await(inner) = &e.kind {
            ex(inner, true, env, walk, diags);
            return;
        }
        if !awaited {
            if let Some((kind, label)) = blocking_kind_of(e, env) {
                report_block(kind, &label, diags);
            }
            if let ExprKind::Call { name, .. } = &e.kind {
                if rust_name(name) == "sleep" {
                    diags.error_once(
                        "sleep-in-event",
                        "`Sleep` pauses the whole UI thread — the screen freezes and keys go \
                         unanswered. To run something after a delay, use a timer instead: \
                         `Every <ms> <Event>`.",
                    );
                } else if walk.helper_does_io(name) {
                    diags.error_once(
                        &format!("blocking-fn-{}", rust_name(name)),
                        format!(
                            "`{}` waits for I/O, so calling it directly in an event would freeze \
                             the UI. Use `Match Await {}(…)` so it runs off the UI thread.",
                            name, name
                        ),
                    );
                }
            }
        }
        match &e.kind {
            ExprKind::Not(i) | ExprKind::ParallelSum(i, _) | ExprKind::Ref(i) | ExprKind::MutRef(i) | ExprKind::Deref(i) | ExprKind::Cast(i, _)
            | ExprKind::Try(i) | ExprKind::Raw(i) | ExprKind::Field(i, _) | ExprKind::TupleIndex(i, _)
            | ExprKind::Closure { body: i, .. } => ex(i, false, env, walk, diags),
            ExprKind::Binary { lhs, rhs, .. }
            | ExprKind::Index(lhs, rhs)
            | ExprKind::ListRepeat { value: lhs, count: rhs } => {
                ex(lhs, false, env, walk, diags);
                ex(rhs, false, env, walk, diags);
            }
            ExprKind::MethodCall { recv, args, .. } => {
                ex(recv, false, env, walk, diags);
                for a in args {
                    ex(a, false, env, walk, diags);
                }
            }
            ExprKind::Call { args, .. } => {
                for a in args {
                    ex(a, false, env, walk, diags);
                }
            }
            ExprKind::Tuple(es) | ExprKind::List(es) => {
                for e2 in es {
                    ex(e2, false, env, walk, diags);
                }
            }
            ExprKind::StructLit { fields, .. } => {
                for (_, v) in fields {
                    ex(v, false, env, walk, diags);
                }
            }
            _ => {}
        }
    }
    fn st(
        s: &Stmt,
        env: &mut HashMap<String, DeclType>,
        walk: &mut IoWalk,
        diags: &mut Diagnostics,
    ) {
        match s {
            Stmt::Assign { target, value, .. } => {
                ex(target, false, env, walk, diags);
                ex(value, false, env, walk, diags);
            }
            Stmt::Dim { name, ty, init, .. } => {
                if let Some(e) = init {
                    ex(e, false, env, walk, diags);
                }
                env.insert(rust_name(name), ty.clone());
            }
            Stmt::Set { value: e, .. }
            | Stmt::DestructureDim { value: e, .. }
            | Stmt::Print(e)
            | Stmt::Log(_, e)
            | Stmt::Expr(e)
            | Stmt::Return(Some(e))
            | Stmt::RaiseError(e)
            | Stmt::Assert(e) => ex(e, false, env, walk, diags),
            Stmt::HandleErr { target, call, body, .. } => {
                if let Some(t) = target {
                    ex(t, false, env, walk, diags);
                }
                ex(call, false, env, walk, diags);
                for s2 in body {
                    st(s2, env, walk, diags);
                }
            }
            Stmt::If { branches, else_body } => {
                for (c, b) in branches {
                    ex(c, false, env, walk, diags);
                    for s2 in b {
                        st(s2, env, walk, diags);
                    }
                }
                if let Some(b) = else_body {
                    for s2 in b {
                        st(s2, env, walk, diags);
                    }
                }
            }
            Stmt::Match { scrutinee, arms, .. } => {
                ex(scrutinee, false, env, walk, diags);
                for a in arms {
                    if let Some(g) = &a.guard {
                        ex(g, false, env, walk, diags);
                    }
                    for s2 in &a.body {
                        st(s2, env, walk, diags);
                    }
                }
            }
            Stmt::For { from, to, step, body, .. } => {
                ex(from, false, env, walk, diags);
                ex(to, false, env, walk, diags);
                if let Some(st_e) = step {
                    ex(st_e, false, env, walk, diags);
                }
                for s2 in body {
                    st(s2, env, walk, diags);
                }
            }
            Stmt::ForEach { iter, body, .. } => {
                ex(iter, false, env, walk, diags);
                for s2 in body {
                    st(s2, env, walk, diags);
                }
            }
            Stmt::DoLoop { cond, body } => {
                if let Some(c) = cond {
                    match c {
                        DoCond::PreWhile(e)
                        | DoCond::PreUntil(e)
                        | DoCond::PostWhile(e)
                        | DoCond::PostUntil(e) => ex(e, false, env, walk, diags),
                    }
                }
                for s2 in body {
                    st(s2, env, walk, diags);
                }
            }
            Stmt::GpuInto { body, .. } => {
                for s2 in body {
                    st(s2, env, walk, diags);
                }
            }
            _ => {}
        }
    }
    for s in stmts {
        st(s, &mut env, &mut walk, diags);
    }
}

/// Does a statement contain an `Await` (in any expression position)?
pub(crate) fn stmt_has_await(s: &Stmt) -> bool {
    match s {
        Stmt::Dim { init: Some(e), .. }
        | Stmt::Print(e)
        | Stmt::Log(_, e)
        | Stmt::Expr(e)
        | Stmt::Return(Some(e))
        | Stmt::RaiseError(e)
        | Stmt::Assert(e)
        | Stmt::Set { value: e, .. }
        | Stmt::DestructureDim { value: e, .. } => expr_has_await(e),
        Stmt::Assign { target, value, .. } => expr_has_await(target) || expr_has_await(value),
        Stmt::HandleErr { target, call, body, .. } => {
            target.as_ref().is_some_and(expr_has_await)
                || expr_has_await(call)
                || body.iter().any(stmt_has_await)
        }
        Stmt::Match { scrutinee, arms, .. } => {
            expr_has_await(scrutinee)
                || arms.iter().any(|a| {
                    a.guard.as_ref().is_some_and(expr_has_await) || a.body.iter().any(stmt_has_await)
                })
        }
        Stmt::If { branches, else_body } => {
            branches.iter().any(|(c, b)| expr_has_await(c) || b.iter().any(stmt_has_await))
                || else_body.as_ref().is_some_and(|b| b.iter().any(stmt_has_await))
        }
        Stmt::For { from, to, step, body, .. } => {
            expr_has_await(from)
                || expr_has_await(to)
                || step.as_ref().is_some_and(expr_has_await)
                || body.iter().any(stmt_has_await)
        }
        Stmt::ForEach { iter, body, .. } => expr_has_await(iter) || body.iter().any(stmt_has_await),
        Stmt::DoLoop { cond, body, .. } => {
            cond.as_ref().is_some_and(|c| match c {
                DoCond::PreWhile(e)
                | DoCond::PreUntil(e)
                | DoCond::PostWhile(e)
                | DoCond::PostUntil(e) => expr_has_await(e),
            }) || body.iter().any(stmt_has_await)
        }
        Stmt::GpuInto { body, .. } => body.iter().any(stmt_has_await),
        _ => false,
    }
}

fn expr_has_await(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Await(_) => true,
        ExprKind::Not(i) | ExprKind::ParallelSum(i, _) | ExprKind::Ref(i) | ExprKind::MutRef(i) | ExprKind::Deref(i) | ExprKind::Cast(i, _)
        | ExprKind::Try(i) | ExprKind::Field(i, _) | ExprKind::TupleIndex(i, _) | ExprKind::Closure { body: i, .. } => {
            expr_has_await(i)
        }
        ExprKind::Binary { lhs, rhs, .. }
        | ExprKind::Index(lhs, rhs)
        | ExprKind::ListRepeat { value: lhs, count: rhs } => {
            expr_has_await(lhs) || expr_has_await(rhs)
        }
        ExprKind::MethodCall { recv, args, .. } => {
            expr_has_await(recv) || args.iter().any(expr_has_await)
        }
        ExprKind::Call { args, .. } => args.iter().any(expr_has_await),
        ExprKind::Tuple(es) => es.iter().any(expr_has_await),
        ExprKind::StructLit { fields, .. } => fields.iter().any(|(_, v)| expr_has_await(v)),
        _ => false,
    }
}

/// Collect the stdlib namespaces (e.g. `Http`) used in event bodies — for the
/// `use vbr_stdlib::{…}` line. Pure collection; marking is the caller's call.
pub(crate) fn collect_event_stdlib(stmts: &[Stmt], out: &mut Vec<String>) {
    fn ex(e: &Expr, out: &mut Vec<String>) {
        match &e.kind {
            ExprKind::MethodCall { recv, args, .. } => {
                if let ExprKind::Ident(r) = &(&**recv).kind {
                    if let Some(c) = stdlib_type(r) {
                        out.push(c.to_string());
                    }
                }
                ex(recv, out);
                for a in args {
                    ex(a, out);
                }
            }
            ExprKind::Await(i) | ExprKind::Not(i) | ExprKind::ParallelSum(i, _) | ExprKind::Ref(i) | ExprKind::MutRef(i) | ExprKind::Deref(i)
            | ExprKind::Cast(i, _) | ExprKind::Try(i) | ExprKind::Field(i, _) | ExprKind::TupleIndex(i, _)
            | ExprKind::Closure { body: i, .. } => ex(i, out),
            ExprKind::Binary { lhs, rhs, .. }
            | ExprKind::Index(lhs, rhs)
            | ExprKind::ListRepeat { value: lhs, count: rhs } => {
                ex(lhs, out);
                ex(rhs, out);
            }
            ExprKind::Call { args, .. } => {
                for a in args {
                    ex(a, out);
                }
            }
            ExprKind::Tuple(es) => {
                for e2 in es {
                    ex(e2, out);
                }
            }
            ExprKind::StructLit { fields, .. } => {
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
/// in a window's view/events, `self` inside a canvas `Draw` block. (View
/// expressions never call helper `Sub`s, so this passes no sub set.)
pub(crate) fn rewrite_expr_with(
    e: Expr,
    recv: &'static str,
    fields: &HashSet<String>,
    enums: &HashSet<String>,
) -> Expr {
    rewrite_expr_subs(e, recv, fields, enums, &HashSet::new())
}

/// As `rewrite_expr_with`, but also rewrites a call to an in-block `Sub` helper
/// (`subs`) into a method call on the receiver — `TryMove(0)` → `recv.trymove(0)`.
fn rewrite_expr_subs(
    e: Expr,
    recv: &'static str,
    fields: &HashSet<String>,
    enums: &HashSet<String>,
    subs: &HashSet<String>,
) -> Expr {
    let go = |e: Expr| rewrite_expr_subs(e, recv, fields, enums, subs);
    // Rewrites replace the *kind*; the span survives, so a rewritten
    // `count` → `state.count` still points at the `count` the user wrote.
    let span = e.span;
    let kind = match e.kind {
        // `Color.Red` (field on an enum name) → the path `Color::Red`.
        ExprKind::Field(inner, variant) if matches!(&inner.kind, ExprKind::Ident(n) if enums.contains(n)) => {
            match inner.kind {
                ExprKind::Ident(n) => ExprKind::ConstRef(format!("{}::{}", n, variant)),
                _ => unreachable!(),
            }
        }
        ExprKind::Ident(name) if fields.contains(&rust_name(&name)) => {
            ExprKind::Field(Box::new(ExprKind::Ident(recv.to_string()).at(span)), name)
        }
        ExprKind::Binary { op, lhs, rhs } => ExprKind::Binary {
            op,
            lhs: Box::new(go(*lhs)),
            rhs: Box::new(go(*rhs)),
        },
        ExprKind::Not(inner) => ExprKind::Not(Box::new(go(*inner))),
        ExprKind::ParallelSum(inner, ty) => ExprKind::ParallelSum(Box::new(go(*inner)), ty),
        // A call to an in-block `Sub` helper → a method call on the receiver.
        ExprKind::Call { name, args } if subs.contains(&rust_name(&name)) => ExprKind::MethodCall {
            recv: Box::new(ExprKind::Ident(recv.to_string()).at(span)),
            method: name,
            args: args.into_iter().map(go).collect(),
        },
        ExprKind::Call { name, args } => ExprKind::Call {
            name,
            args: args.into_iter().map(go).collect(),
        },
        // `Shape.Circle(r)` on an enum → the variant constructor `Shape::Circle(r)`.
        ExprKind::MethodCall { recv: r, method, args } if matches!(&r.kind, ExprKind::Ident(e) if enums.contains(e)) => {
            let e = match r.kind {
                ExprKind::Ident(n) => n,
                _ => unreachable!(),
            };
            ExprKind::Call {
                name: format!("{}::{}", e, method),
                args: args.into_iter().map(go).collect(),
            }
        }
        ExprKind::MethodCall { recv: r, method, args } => ExprKind::MethodCall {
            recv: Box::new(go(*r)),
            method,
            args: args.into_iter().map(go).collect(),
        },
        ExprKind::Field(inner, f) => ExprKind::Field(Box::new(go(*inner)), f),
        ExprKind::Index(a, b) => ExprKind::Index(Box::new(go(*a)), Box::new(go(*b))),
        ExprKind::Cast(inner, t) => ExprKind::Cast(Box::new(go(*inner)), t),
        // Wrappers the resolver may have added around a state field (`&db` for
        // a ByVal struct arg, `x?` chaining) — recurse through them all, or the
        // field inside never becomes `state.<field>`.
        ExprKind::Ref(inner) => ExprKind::Ref(Box::new(go(*inner))),
        ExprKind::MutRef(inner) => ExprKind::MutRef(Box::new(go(*inner))),
        ExprKind::Deref(inner) => ExprKind::Deref(Box::new(go(*inner))),
        ExprKind::Try(inner) => ExprKind::Try(Box::new(go(*inner))),
        ExprKind::Raw(inner) => ExprKind::Raw(Box::new(go(*inner))),
        ExprKind::Await(inner) => ExprKind::Await(Box::new(go(*inner))),
        ExprKind::Tuple(elems) => ExprKind::Tuple(elems.into_iter().map(go).collect()),
        ExprKind::List(elems) => ExprKind::List(elems.into_iter().map(go).collect()),
        ExprKind::ListRepeat { value, count } => ExprKind::ListRepeat {
            value: Box::new(go(*value)),
            count: Box::new(go(*count)),
        },
        ExprKind::StructLit { name, fields } => ExprKind::StructLit {
            name,
            fields: fields.into_iter().map(|(n, v)| (n, go(v))).collect(),
        },
        ExprKind::Closure { params, body, by_ref_params } => ExprKind::Closure {
            params,
            body: Box::new(go(*body)),
            by_ref_params,
        },
        ExprKind::TupleIndex(inner, i) => ExprKind::TupleIndex(Box::new(go(*inner)), i),
        // Leaves — nothing inside to rewrite. Listed explicitly (no `_`) so that
        // a new `Expr` variant carrying a child fails to compile here rather than
        // silently dropping a state field, the way #10/#16 once did. `InlineRust`
        // and `InlinePython` are opaque bodies; `InlinePython`'s `inputs` are
        // variable *names* (strings), not `Expr`s, so a state field passed into a
        // Python block still can't be rewritten — a known, separate limitation.
        leaf @ (ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::Str(_)
        | ExprKind::Ident(_)
        | ExprKind::ConstRef(_)
        | ExprKind::InlineRust(_)
        | ExprKind::InlinePython { .. }) => leaf,
    };
    kind.at(span)
}

pub(crate) fn rewrite_stmt(
    s: Stmt,
    recv: &'static str,
    fields: &HashSet<String>,
    enums: &HashSet<String>,
    subs: &HashSet<String>,
) -> Stmt {
    let re = |e: Expr| rewrite_expr_subs(e, recv, fields, enums, subs);
    match s {
        Stmt::Assign { target, value, op } => Stmt::Assign {
            target: re(target),
            value: re(value),
            op,
        },
        Stmt::Print(e) => Stmt::Print(re(e)),
        Stmt::Log(level, e) => Stmt::Log(level, re(e)),
        Stmt::Expr(e) => Stmt::Expr(re(e)),
        Stmt::If { branches, else_body } => Stmt::If {
            branches: branches
                .into_iter()
                .map(|(c, b)| {
                    (
                        re(c),
                        b.into_iter().map(|s| rewrite_stmt(s, recv, fields, enums, subs)).collect(),
                    )
                })
                .collect(),
            else_body: else_body
                .map(|b| b.into_iter().map(|s| rewrite_stmt(s, recv, fields, enums, subs)).collect()),
        },
        Stmt::Match { scrutinee, arms, line, if_let } => Stmt::Match {
            scrutinee: re(scrutinee),
            arms: arms
                .into_iter()
                .map(|a| MatchArm {
                    pattern: a.pattern,
                    guard: a.guard.map(&re),
                    body: a
                        .body
                        .into_iter()
                        .map(|s| rewrite_stmt(s, recv, fields, enums, subs))
                        .collect(),
                })
                .collect(),
            line,
            if_let,
        },
        Stmt::Dim { name, name_span, ty, init, deferred, line } => Stmt::Dim {
            name,
            name_span,
            ty,
            init: init.map(re),
            deferred,
            line,
        },
        Stmt::For { var, from, to, step, body, ty, parallel, device, device_bufs, line } => Stmt::For {
            var,
            from: re(from),
            to: re(to),
            step: step.map(&re),
            body: body.into_iter().map(|s| rewrite_stmt(s, recv, fields, enums, subs)).collect(),
            ty,
            parallel,
            device,
            device_bufs,
            line,
        },
        Stmt::ForEach { var1, var2, iter, body } => Stmt::ForEach {
            var1,
            var2,
            iter: re(iter),
            body: body.into_iter().map(|s| rewrite_stmt(s, recv, fields, enums, subs)).collect(),
        },
        Stmt::DoLoop { cond, body } => Stmt::DoLoop {
            cond: cond.map(|c| match c {
                DoCond::PreWhile(e) => DoCond::PreWhile(re(e)),
                DoCond::PreUntil(e) => DoCond::PreUntil(re(e)),
                DoCond::PostWhile(e) => DoCond::PostWhile(re(e)),
                DoCond::PostUntil(e) => DoCond::PostUntil(re(e)),
            }),
            body: body.into_iter().map(|s| rewrite_stmt(s, recv, fields, enums, subs)).collect(),
        },
        Stmt::Set { name, mutable, value } => Stmt::Set { name, mutable, value: re(value) },
        Stmt::DestructureDim { names, ty, value } => {
            Stmt::DestructureDim { names, ty, value: re(value) }
        }
        Stmt::Return(e) => Stmt::Return(e.map(re)),
        Stmt::RaiseError(e) => Stmt::RaiseError(re(e)),
        Stmt::HandleErr { target, call, err_name, body, line } => Stmt::HandleErr {
            target: target.map(re),
            call: re(call),
            err_name,
            body: body.into_iter().map(|s| rewrite_stmt(s, recv, fields, enums, subs)).collect(),
            line,
        },
        // Leaves and canvas-only forms — no bare state field to rewrite. Listed
        // explicitly (no `_`) so a new statement carrying an expression or a body
        // must be handled here, not swallowed. `Draw` only appears in a canvas
        // `Draw` block, rewritten by `gui::rewrite_canvas_stmt`, never in an event.
        leaf @ (Stmt::HandleDim { .. }
        | Stmt::Break
        | Stmt::Continue
        | Stmt::Destroy { .. }
        | Stmt::Draw(_)
        | Stmt::GpuInto { .. }
        // `Assert` only appears in a `Test` block, never a surface event body.
        | Stmt::Assert(_)
        | Stmt::Comment(_)
        | Stmt::LineMark(_)) => leaf,
    }
}
