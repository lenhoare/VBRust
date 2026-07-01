//! GUI codegen — a `Window` becomes an Iced 0.13 application.
//!
//! Slice 1: `Window` / `State` / `View` / `Event` → a `State` struct, a `Message`
//! enum, an `update` function, a `view` function, and `main`. The model is The
//! Elm Architecture, which is what Iced is: state is the source of truth, the
//! view is derived from it, and events update it.

use crate::ast::*;
use crate::diagnostics::Diagnostics;
use crate::resolver;
use crate::transpiler::{
    decltype_rust, emit_const, emit_enum, emit_fn, emit_impl, emit_stmt, emit_struct, note_builtins,
    render_expr, stdlib_type, to_snake,
};
use std::collections::{HashMap, HashSet};

/// What the view renderer needs to know about a window's state: the field names
/// (to rewrite `count` → `state.count`) and their types (so a `String` match
/// scrutinee gets `.as_str()`).
struct ViewCtx<'a> {
    fields: &'a HashSet<String>,
    field_ty: &'a HashMap<String, DeclType>,
    enums: &'a HashSet<String>,
    /// For each canvas placed in this view, the state fields its `Draw` block
    /// reads — snapshotted into the canvas Program when it's constructed.
    canvas_snaps: &'a HashMap<String, Vec<String>>,
}

/// Emit a complete GUI program: each window's definition, then `fn main`, which
/// launches the window named by `<Window>.Run` inside `Function Main()`.
pub fn emit_gui_program(program: &Program, diags: &mut Diagnostics) -> String {
    let mut out = String::new();
    for comment in &program.leading_comments {
        out.push_str(&format!("// {}\n", comment));
    }
    if !program.leading_comments.is_empty() {
        out.push('\n');
    }
    // Top-level items a GUI may define and reference (enums for Radio, etc.).
    for c in &program.constants {
        emit_const(c, &mut out, diags);
    }
    if !program.constants.is_empty() {
        out.push('\n');
    }
    for s in &program.structs {
        emit_struct(s, diags, &mut out);
        out.push('\n');
    }
    for e in &program.enums {
        emit_enum(e, &mut out);
        out.push('\n');
    }

    let enums: HashSet<String> = program.enums.iter().map(|e| e.name.clone()).collect();

    // User-defined functions/methods (everything except `Main`, which becomes the
    // Iced `fn main`). Without this a GUI couldn't call its own procedures.
    let modules: HashSet<String> = HashSet::new();
    let fns = resolver::build_fn_table(program);
    let methods = resolver::build_method_table(program);
    let consts = resolver::build_const_map(program);
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
        emit_impl(recv, program, &fns, &methods, &consts, &modules, &enums, diags, &mut out);
        out.push('\n');
    }
    // Paint functions (those that issue drawing verbs) are emitted specially —
    // they take an extra `frame` and their draw commands lower to `frame.*` calls.
    let paint_fns = paint_fn_set(program);

    // Free functions, except `Main`.
    for f in program.functions.iter().filter(|f| f.receiver.is_none() && !is_main(f)) {
        if paint_fns.contains(&to_snake(&f.name)) {
            emit_paint_fn(f, &enums, &paint_fns, diags, &mut out);
        } else {
            emit_fn(f, &fns, &methods, &consts, &modules, &enums, diags, &mut out, 0, None);
        }
        out.push('\n');
    }

    for w in &program.windows {
        out.push_str(&emit_window(w, &enums, &fns, &program.canvases, &paint_fns, diags));
        out.push('\n');
    }
    match find_launched_window(program) {
        Some(w) => out.push_str(&emit_main(w)),
        None => diags.error_once(
            "gui-no-launch",
            "A window is never launched. Add `Function Main()` containing `<Window>.Run`, \
             e.g. `Counter.Run`.",
        ),
    }
    out
}

/// Iced's built-in themes (variant names). Selecting one restyles the whole app.
const KNOWN_THEMES: &[&str] = &[
    "Light", "Dark", "Dracula", "Nord", "SolarizedLight", "SolarizedDark",
    "GruvboxLight", "GruvboxDark", "CatppuccinLatte", "CatppuccinFrappe",
    "CatppuccinMacchiato", "CatppuccinMocha", "TokyoNight", "TokyoNightStorm",
    "TokyoNightLight", "KanagawaWave", "KanagawaDragon", "KanagawaLotus",
    "Moonfly", "Nightfly", "Oxocarbon", "Ferra",
];

/// The canonical (PascalCase) name of a built-in theme, matched case-insensitively.
fn canonical_theme(name: &str) -> Option<&'static str> {
    KNOWN_THEMES.iter().find(|t| t.eq_ignore_ascii_case(name)).copied()
}

/// `fn main` for a GUI: run the window. With a `Theme`, use the `application`
/// builder so we can set `.theme(...)`; otherwise the simpler `iced::run`.
fn emit_main(w: &Window) -> String {
    let title = w.title.clone().unwrap_or_else(|| w.name.clone());
    match &w.theme {
        Some(t) => {
            let theme = canonical_theme(t).unwrap_or(t);
            format!(
                "fn main() -> iced::Result {{\n    \
                 iced::application({:?}, update, view)\n        \
                 .theme(|_| iced::Theme::{})\n        \
                 .run()\n}}\n",
                title, theme
            )
        }
        None => format!(
            "fn main() -> iced::Result {{\n    iced::run({:?}, update, view)\n}}\n",
            title
        ),
    }
}

/// Find the window launched by a `<Window>.Run` statement inside `Function Main()`.
/// Accepts the property form (`Counter.Run`) and the call form (`Counter.Run()`).
fn find_launched_window(program: &Program) -> Option<&Window> {
    let main = program
        .functions
        .iter()
        .find(|f| f.name.eq_ignore_ascii_case("Main"))?;
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
                if let Some(w) = program.windows.iter().find(|w| w.name.eq_ignore_ascii_case(name)) {
                    return Some(w);
                }
            }
        }
    }
    None
}

/// Emit one window's *definition* — the State struct, Message enum, update, and
/// view. (`fn main` is emitted separately, from the launch in `Function Main()`.)
fn emit_window(
    w: &Window,
    enums: &HashSet<String>,
    fns: &resolver::FnTable,
    canvases: &[CanvasDef],
    paint_fns: &HashSet<String>,
    diags: &mut Diagnostics,
) -> String {
    let mut out = String::new();
    let ty = &w.name; // the state struct is named after the window
    let fields: HashSet<String> = w.state.iter().map(|f| to_snake(&f.name)).collect();
    let field_ty: HashMap<String, DeclType> =
        w.state.iter().map(|f| (to_snake(&f.name), f.ty.clone())).collect();

    // Canvases placed in this view: resolve each to its definition, and work out
    // which state fields its `Draw` block reads (snapshotted into the Program).
    let used_canvases = collect_canvases(&w.view);
    let mut canvas_snaps: HashMap<String, Vec<String>> = HashMap::new();
    for cname in &used_canvases {
        match canvases.iter().find(|c| c.name.eq_ignore_ascii_case(cname)) {
            Some(cv) => {
                let idents = canvas_idents(&cv.body);
                let snap: Vec<String> = w
                    .state
                    .iter()
                    .map(|f| to_snake(&f.name))
                    .filter(|f| idents.contains(f))
                    .collect();
                canvas_snaps.insert(cname.clone(), snap);
            }
            None => diags.error_once(
                &format!("unknown-canvas-{}", to_snake(cname)),
                format!("The view places a `Canvas {}`, but there's no `Canvas {}` defined.", cname, cname),
            ),
        }
    }

    let ctx = ViewCtx { fields: &fields, field_ty: &field_ty, enums, canvas_snaps: &canvas_snaps };
    validate_view(&w.view, &field_ty, diags);
    if let Some(t) = &w.theme {
        if canonical_theme(t).is_none() {
            diags.error_once(
                "unknown-theme",
                format!(
                    "Unknown theme `{}`. Built-in themes: {}.",
                    t,
                    KNOWN_THEMES.join(", ")
                ),
            );
        }
    }

    // Analyse each event for `Await`: an async event splits into a kick-off arm
    // (returns a `Task`) and a generated `<Event>Done(...)` continuation arm. If
    // any event is async, the whole `update` returns `Task<Message>`.
    let splits: Vec<Option<AwaitSplit>> = w
        .events
        .iter()
        .map(|e| await_split(e, &field_ty, fns, diags))
        .collect();
    let any_async = splits.iter().any(Option::is_some);

    // A blocking stdlib call in an event must be `Await`ed, or the window freezes.
    for e in &w.events {
        check_blocking_without_await(&e.body, diags);
    }

    // Import only the widgets the view uses, plus Task / stdlib namespaces when
    // events need them.
    let mut widgets: Vec<&'static str> = Vec::new();
    collect_widgets(&w.view, &mut widgets);
    widgets.sort();
    out.push_str(&format!("use iced::widget::{{{}}};\n", widgets.join(", ")));
    out.push_str("use iced::Element;\n");
    if any_async {
        out.push_str("use iced::Task;\n");
    }
    let mut std_used: Vec<String> = Vec::new();
    for e in &w.events {
        collect_event_stdlib(&e.body, &mut std_used, diags);
    }
    std_used.sort();
    std_used.dedup();
    if !std_used.is_empty() {
        out.push_str(&format!("use vbr_stdlib::{{{}}};\n", std_used.join(", ")));
    }
    out.push('\n');

    // The multi-line editor fields (declared `As TextArea`) — held as a stateful
    // `text_editor::Content`, each with an auto-generated edit message.
    let textareas = collect_textareas(&w.view);

    // ── State struct ──
    out.push_str(&format!("struct {} {{\n", ty));
    for f in &w.state {
        let fty = if is_textarea(&f.ty) {
            "iced::widget::text_editor::Content".to_string()
        } else {
            decltype_rust(&f.ty)
        };
        out.push_str(&format!("    {}: {},\n", to_snake(&f.name), fty));
    }
    out.push_str("}\n\n");

    // ── Initial state (the Dim initialisers) ──
    out.push_str(&format!("impl Default for {} {{\n    fn default() -> Self {{\n", ty));
    out.push_str(&format!("        {} {{\n", ty));
    for f in &w.state {
        let init = if is_textarea(&f.ty) {
            let text = f.init.as_ref().map(|e| render_expr(e, None)).unwrap_or_else(|| "\"\"".to_string());
            format!("iced::widget::text_editor::Content::with_text({})", text)
        } else {
            render_init(f.init.as_ref(), &f.ty, enums)
        };
        out.push_str(&format!("            {}: {},\n", to_snake(&f.name), init));
    }
    out.push_str("        }\n    }\n}\n\n");

    // ── Message enum: one variant per event (payload params = its data), plus a
    //    `<Event>Done(result)` continuation variant for each async event. ──
    out.push_str("#[derive(Debug, Clone)]\nenum Message {\n");
    for (e, split) in w.events.iter().zip(&splits) {
        if e.params.is_empty() {
            out.push_str(&format!("    {},\n", e.name));
        } else {
            let types: Vec<String> = e.params.iter().map(|p| decltype_rust(&p.ty)).collect();
            out.push_str(&format!("    {}({}),\n", e.name, types.join(", ")));
        }
        if let Some(s) = split {
            out.push_str(&format!("    {}Done({}),\n", e.name, s.ret_type));
        }
    }
    // Auto-generated edit message for each TextArea (carries an editor Action).
    for field in &textareas {
        out.push_str(&format!(
            "    {}Edited(iced::widget::text_editor::Action),\n",
            to_pascal(field)
        ));
    }
    out.push_str("}\n\n");

    // ── update: state-field idents are rewritten to `state.field`. An async
    //    event's kick-off returns `Task::perform(work, Message::<Event>Done)`;
    //    its continuation runs the post-await handling. ──
    let empty: HashSet<String> = HashSet::new();
    let ret = if any_async { " -> Task<Message>" } else { "" };
    // With no events and no text areas, nothing touches `state` — underscore it
    // so a display-only window (e.g. just an Image) compiles warning-free.
    let state_param = if w.events.is_empty() && textareas.is_empty() {
        "_state"
    } else {
        "state"
    };
    out.push_str(&format!(
        "fn update({}: &mut {}, message: Message){} {{\n",
        state_param, ty, ret
    ));
    out.push_str("    match message {\n");
    for (e, split) in w.events.iter().zip(&splits) {
        // The triggering variant's arm header.
        if e.params.is_empty() {
            out.push_str(&format!("        Message::{} => {{\n", e.name));
        } else {
            let binds: Vec<String> = e.params.iter().map(|p| to_snake(&p.name)).collect();
            out.push_str(&format!("        Message::{}({}) => {{\n", e.name, binds.join(", ")));
        }
        match split {
            // Synchronous event: run the body; async windows need a `Task::none()`.
            None => {
                for stmt in &e.body {
                    let mut rewritten = rewrite_stmt(stmt.clone(), &fields, enums);
                    coerce_state_strings(&mut rewritten, &field_ty);
                    emit_stmt(&rewritten, &empty, &empty, 3, diags, &mut out);
                }
                if any_async {
                    out.push_str("            Task::none()\n");
                }
            }
            // Async kick-off: pre-await body, snapshot state, then return the Task.
            Some(s) => {
                for stmt in &s.pre {
                    let mut rewritten = rewrite_stmt(stmt.clone(), &fields, enums);
                    coerce_state_strings(&mut rewritten, &field_ty);
                    emit_stmt(&rewritten, &empty, &empty, 3, diags, &mut out);
                }
                for snap in &s.snapshots {
                    out.push_str(&format!("            {}\n", snap));
                }
                let work = if s.blocking {
                    // Our stdlib is blocking — run it off the UI thread.
                    format!(
                        "async move {{ tokio::task::spawn_blocking(move || {}).await.unwrap() }}",
                        s.call_src
                    )
                } else {
                    format!("async move {{ {} }}", s.call_src)
                };
                out.push_str(&format!(
                    "            Task::perform({}, Message::{}Done)\n",
                    work, e.name
                ));
            }
        }
        out.push_str("        }\n");
        // The continuation arm for an async event.
        if let Some(s) = split {
            out.push_str(&format!("        Message::{}Done({}) => {{\n", e.name, s.bind));
            for stmt in &s.cont {
                let mut rewritten = rewrite_stmt(stmt.clone(), &fields, enums);
                    coerce_state_strings(&mut rewritten, &field_ty);
                emit_stmt(&rewritten, &empty, &empty, 3, diags, &mut out);
            }
            out.push_str("            Task::none()\n");
            out.push_str("        }\n");
        }
    }
    // Auto-generated arm for each TextArea: apply the edit to its content.
    for field in &textareas {
        out.push_str(&format!("        Message::{}Edited(action) => {{\n", to_pascal(field)));
        out.push_str(&format!("            state.{}.perform(action);\n", field));
        if any_async {
            out.push_str("            Task::none()\n");
        }
        out.push_str("        }\n");
    }
    out.push_str("    }\n}\n\n");

    // ── view ──
    out.push_str(&format!("fn view(state: &{}) -> Element<'_, Message> {{\n", ty));
    out.push_str("    ");
    out.push_str(&render_view(&w.view, &ctx, 1, true));
    out.push('\n');
    out.push_str("}\n");

    // ── canvas Program(s) used by this view ──
    for cname in &used_canvases {
        if let (Some(cv), Some(snap)) = (
            canvases.iter().find(|c| c.name.eq_ignore_ascii_case(cname)),
            canvas_snaps.get(cname),
        ) {
            out.push('\n');
            emit_canvas_program(cv, snap, &fields, &field_ty, enums, paint_fns, diags, &mut out);
        }
    }

    out
}

/// Emit a canvas's Iced `Program` — the snapshot struct plus a `draw` method that
/// runs the `Draw` block against a `frame`. State fields the block reads become
/// `self.field`; calls to paint functions thread the shared `frame` through.
fn emit_canvas_program(
    cv: &CanvasDef,
    snap: &[String],
    fields: &HashSet<String>,
    field_ty: &HashMap<String, DeclType>,
    enums: &HashSet<String>,
    paint_fns: &HashSet<String>,
    diags: &mut Diagnostics,
    out: &mut String,
) {
    let struct_name = format!("{}Canvas", cv.name);
    // The snapshot struct: a copy of just the state the drawing reads.
    out.push_str(&format!("struct {} {{\n", struct_name));
    for f in snap {
        let ty = field_ty.get(f).map(decltype_rust).unwrap_or_else(|| "i32".to_string());
        out.push_str(&format!("    {}: {},\n", f, ty));
    }
    out.push_str("}\n\n");

    out.push_str(&format!(
        "impl<Message> iced::widget::canvas::Program<Message> for {} {{\n",
        struct_name
    ));
    out.push_str("    type State = ();\n");
    out.push_str(
        "    fn draw(\n        &self,\n        _state: &Self::State,\n        \
         renderer: &iced::Renderer,\n        _theme: &iced::Theme,\n        \
         bounds: iced::Rectangle,\n        _cursor: iced::mouse::Cursor,\n    ) \
         -> Vec<iced::widget::canvas::Geometry> {\n",
    );
    out.push_str(
        "        let mut frame = iced::widget::canvas::Frame::new(renderer, bounds.size());\n",
    );
    // A `&mut Frame` shadow, so draw verbs and paint-function calls are uniform.
    out.push_str("        {\n            let frame = &mut frame;\n            let _ = &frame;\n");
    let empty: HashSet<String> = HashSet::new();
    for stmt in &cv.body {
        let mut rewritten = rewrite_canvas_stmt(stmt.clone(), fields, enums, paint_fns);
        coerce_state_strings(&mut rewritten, field_ty);
        emit_stmt(&rewritten, &empty, &empty, 3, diags, out);
    }
    out.push_str("        }\n");
    out.push_str("        vec![frame.into_geometry()]\n");
    out.push_str("    }\n}\n");
}

/// Emit a paint function: an ordinary function that draws, so it gains a leading
/// `frame: &mut Frame` and its draw verbs / nested paint calls lower against it.
fn emit_paint_fn(
    func: &Function,
    enums: &HashSet<String>,
    paint_fns: &HashSet<String>,
    diags: &mut Diagnostics,
    out: &mut String,
) {
    let name = to_snake(&func.name);
    let vis = if func.public { "pub " } else { "" };
    let mut params = vec!["frame: &mut iced::widget::canvas::Frame".to_string()];
    for p in &func.params {
        params.push(render_paint_param(p));
    }
    out.push_str(&format!("{}fn {}({}) {{\n", vis, name, params.join(", ")));
    let empty: HashSet<String> = HashSet::new();
    // Paint functions read no window state (they take values as params); only the
    // paint-call and enum rewrites apply.
    for stmt in &func.body {
        let rewritten = rewrite_canvas_stmt(stmt.clone(), &empty, enums, paint_fns);
        emit_stmt(&rewritten, &empty, &empty, 1, diags, out);
    }
    out.push_str("}\n");
}

/// A paint-function parameter — numeric/bool by value, `&str` for text.
fn render_paint_param(p: &Param) -> String {
    let ty = match &p.ty {
        DeclType::Plain(Type::Text) => "&str".to_string(),
        DeclType::Plain(t) => t.rust().to_string(),
        other => decltype_rust(other),
    };
    format!("{}: {}", to_snake(&p.name), ty)
}

/// A `State` field initialiser: a `String` becomes owned, numbers adapt to type,
/// an enum variant (`Size.Small`) resolves to its path (`Size::Small`), and a
/// `Vec` with no initialiser starts empty.
pub(crate) fn render_init(init: Option<&Expr>, ty: &DeclType, enums: &HashSet<String>) -> String {
    let empty = HashSet::new();
    match (ty, init) {
        (DeclType::Vec(_), None) => "Vec::new()".to_string(),
        (DeclType::Plain(Type::Text), Some(e)) => format!("{}.to_string()", render_expr(e, None)),
        (DeclType::Plain(t), Some(e)) => render_expr(e, Some(*t)),
        // Enum / Vec-with-initialiser / other — rewrite `Size.Small` → `Size::Small`.
        (_, Some(e)) => render_expr(&rewrite_expr(e.clone(), &empty, enums), None),
        // A non-collection field without an initialiser shouldn't reach here (the
        // parser requires one); fall back to Default.
        (_, None) => "Default::default()".to_string(),
    }
}

/// Render a view node to an Iced widget expression, pretty-printed with
/// indentation. `indent` is this node's nesting level; `as_element` appends
/// `.into()` where an `Element` is expected (the root, container children that
/// need it, `Match`/`If` arm bodies). Containers and conditionals break across
/// lines; leaf widgets stay on one line.
fn render_view(node: &ViewNode, ctx: &ViewCtx, indent: usize, as_element: bool) -> String {
    // Containers and conditionals format across multiple lines.
    match node {
        ViewNode::Column { children, spacing, padding } => {
            return render_container("column", children, ctx, indent, as_element, *spacing, *padding)
        }
        ViewNode::Row { children, spacing, padding } => {
            return render_container("row", children, ctx, indent, as_element, *spacing, *padding)
        }
        ViewNode::Match { scrutinee, arms } => return render_view_match(scrutinee, arms, ctx, indent),
        ViewNode::If { branches, else_body } => return render_view_if(branches, else_body, ctx, indent),
        // Layout size constraints are a TUI concept; the GUI ignores them.
        ViewNode::Constrained { child, .. } => return render_view(child, ctx, indent, as_element),
        _ => {}
    }
    // Leaf widgets — a single line.
    let s = match node {
        ViewNode::Text(e) => render_text(e, ctx),
        ViewNode::Button { label, on_click } => {
            let lbl = render_expr(&rewrite_expr(label.clone(), ctx.fields, ctx.enums), None);
            match on_click {
                Some(ev) => format!("button({}).on_press(Message::{})", lbl, ev),
                None => format!("button({})", lbl),
            }
        }
        ViewNode::TextInput { placeholder, value, on_input } => {
            let ph = render_expr(&rewrite_expr(placeholder.clone(), ctx.fields, ctx.enums), None);
            let field = to_snake(value);
            let base = format!("text_input({}, &state.{})", ph, field);
            match on_input {
                Some(ev) => format!("{}.on_input(Message::{})", base, ev),
                None => base,
            }
        }
        ViewNode::Checkbox { label, value, on_toggle } => {
            let lbl = render_expr(&rewrite_expr(label.clone(), ctx.fields, ctx.enums), None);
            let field = to_snake(value);
            // `is_checked` is a `bool` (Copy), so it's passed by value.
            let base = format!("checkbox({}, state.{})", lbl, field);
            match on_toggle {
                Some(ev) => format!("{}.on_toggle(Message::{})", base, ev),
                None => base,
            }
        }
        ViewNode::Slider { min, max, value, on_change } => {
            let lo = render_expr(&rewrite_expr(min.clone(), ctx.fields, ctx.enums), None);
            let hi = render_expr(&rewrite_expr(max.clone(), ctx.fields, ctx.enums), None);
            let field = to_snake(value);
            format!(
                "slider({}..={}, state.{}, Message::{})",
                lo, hi, field, on_change
            )
        }
        ViewNode::Toggler { label, value, on_toggle } => {
            let lbl = render_expr(&rewrite_expr(label.clone(), ctx.fields, ctx.enums), None);
            let field = to_snake(value);
            // `is_toggled` is a `bool` (Copy), passed by value; label via builder.
            let base = format!("toggler(state.{}).label({})", field, lbl);
            match on_toggle {
                Some(ev) => format!("{}.on_toggle(Message::{})", base, ev),
                None => base,
            }
        }
        ViewNode::ProgressBar { min, max, value } => {
            let lo = render_expr(&rewrite_expr(min.clone(), ctx.fields, ctx.enums), None);
            let hi = render_expr(&rewrite_expr(max.clone(), ctx.fields, ctx.enums), None);
            let field = to_snake(value);
            // Iced progress bars are `f32`; cast the bounds and value.
            format!(
                "progress_bar(({} as f32)..=({} as f32), state.{} as f32)",
                lo, hi, field
            )
        }
        ViewNode::Radio { label, value, option, on_select } => {
            let lbl = render_expr(&rewrite_expr(label.clone(), ctx.fields, ctx.enums), None);
            let opt = render_expr(&rewrite_expr(option.clone(), ctx.fields, ctx.enums), None);
            let field = to_snake(value);
            // The selected value is `Some(state.field)` (Copy, so it's a copy).
            format!(
                "radio({}, {}, Some(state.{}), Message::{})",
                lbl, opt, field, on_select
            )
        }
        ViewNode::TextArea { value } => {
            let field = to_snake(value);
            // Edits flow through an Action applied to the editor's Content (the
            // `<Field>Edited` message is generated for you).
            format!(
                "text_editor(&state.{}).on_action(Message::{}Edited)",
                field,
                to_pascal(&field)
            )
        }
        // A blank gap.
        ViewNode::Space { horizontal, amount } => {
            if *horizontal {
                format!("iced::widget::Space::with_width({})", amount)
            } else {
                format!("iced::widget::Space::with_height({})", amount)
            }
        }
        // An image from a path. Fully-qualified so no import is needed; a String
        // path from state is cloned to own it (the handle takes it by value).
        ViewNode::Image { path } => {
            let p = render_expr(&rewrite_expr(path.clone(), ctx.fields, ctx.enums), None);
            match path {
                Expr::Str(_) => format!("iced::widget::image({})", p),
                _ => format!("iced::widget::image({}.clone())", p),
            }
        }
        // A drawing surface: build the canvas Program, snapshotting the state its
        // `Draw` block reads, then apply any fixed size.
        ViewNode::Canvas { name, width, height } => {
            let snap = ctx.canvas_snaps.get(name).cloned().unwrap_or_default();
            let inits: Vec<String> = snap
                .iter()
                .map(|f| {
                    // Copy scalars move freely; anything else (String, Vec, struct)
                    // is cloned so the canvas owns its snapshot.
                    let is_copy = matches!(
                        ctx.field_ty.get(f),
                        Some(DeclType::Plain(t)) if !matches!(t, Type::Text)
                    );
                    if is_copy {
                        format!("{}: state.{}", f, f)
                    } else {
                        format!("{}: state.{}.clone()", f, f)
                    }
                })
                .collect();
            let mut s = format!("iced::widget::Canvas::new({}Canvas {{ {} }})", name, inits.join(", "));
            if let Some(w) = width {
                s.push_str(&format!(".width(iced::Length::Fixed({}.0))", w));
            }
            if let Some(h) = height {
                s.push_str(&format!(".height(iced::Length::Fixed({}.0))", h));
            }
            s
        }
        // `List`/`Table` are Screen (TUI) widgets — invalid in a Window;
        // `validate_view` reports it, so this placeholder is never compiled.
        ViewNode::List { .. } | ViewNode::Table { .. } => {
            "iced::widget::Space::new(iced::Length::Shrink, iced::Length::Shrink)".to_string()
        }
        // Containers/conditionals/constrained returned early above.
        ViewNode::Column { .. } | ViewNode::Row { .. } | ViewNode::Match { .. }
        | ViewNode::If { .. } | ViewNode::Constrained { .. } => {
            unreachable!()
        }
    };
    if as_element {
        format!("{}.into()", s)
    } else {
        s
    }
}

/// A `column!`/`row!` with one child per line, indented, plus optional
/// `.spacing(n)`/`.padding(n)`.
fn render_container(
    kw: &str,
    children: &[ViewNode],
    ctx: &ViewCtx,
    indent: usize,
    as_element: bool,
    spacing: Option<u16>,
    padding: Option<u16>,
) -> String {
    let mut props = String::new();
    if let Some(s) = spacing {
        props.push_str(&format!(".spacing({})", s));
    }
    if let Some(p) = padding {
        props.push_str(&format!(".padding({})", p));
    }
    let tail = if as_element { ".into()" } else { "" };
    if children.is_empty() {
        return format!("{}![]{}{}", kw, props, tail);
    }
    let inner = "    ".repeat(indent + 1);
    let pad = "    ".repeat(indent);
    let mut s = format!("{}![\n", kw);
    for c in children {
        s.push_str(&inner);
        s.push_str(&render_view(c, ctx, indent + 1, false));
        s.push_str(",\n");
    }
    s.push_str(&pad);
    s.push(']');
    s.push_str(&props);
    s.push_str(tail);
    s
}

/// A view `Match` → a typed `{ let el = match … {…}; el }` block, arms on lines.
fn render_view_match(scrutinee: &Expr, arms: &[ViewArm], ctx: &ViewCtx, indent: usize) -> String {
    let subj = render_match_scrutinee(scrutinee, ctx);
    let in1 = "    ".repeat(indent + 1);
    let in2 = "    ".repeat(indent + 2);
    let pad = "    ".repeat(indent);
    let mut s = String::from("{\n");
    s.push_str(&format!("{}let el: Element<'_, Message> = match {} {{\n", in1, subj));
    for arm in arms {
        let guard = match &arm.guard {
            Some(g) => format!(
                " if {}",
                render_expr(&rewrite_expr(g.clone(), ctx.fields, ctx.enums), None)
            ),
            None => String::new(),
        };
        s.push_str(&format!(
            "{}{}{} => {},\n",
            in2,
            arm.pattern,
            guard,
            render_arm_body(&arm.body, ctx, indent + 2)
        ));
    }
    s.push_str(&format!("{}}};\n", in1));
    s.push_str(&format!("{}el\n", in1));
    s.push_str(&format!("{}}}", pad));
    s
}

/// A view `If` → a typed `{ let el = if … {…} else {…}; el }` block.
fn render_view_if(
    branches: &[(Expr, Vec<ViewNode>)],
    else_body: &Option<Vec<ViewNode>>,
    ctx: &ViewCtx,
    indent: usize,
) -> String {
    let in1 = "    ".repeat(indent + 1);
    let in2 = "    ".repeat(indent + 2);
    let pad = "    ".repeat(indent);
    let mut s = String::from("{\n");
    s.push_str(&format!("{}let el: Element<'_, Message> =", in1));
    for (i, (cond, body)) in branches.iter().enumerate() {
        let c = render_expr(&rewrite_expr(cond.clone(), ctx.fields, ctx.enums), None);
        let kw = if i == 0 { " if" } else { " else if" };
        s.push_str(&format!(
            "{} {} {{\n{}{}\n{}}}",
            kw,
            c,
            in2,
            render_arm_body(body, ctx, indent + 2),
            in1
        ));
    }
    let els = match else_body {
        Some(b) => render_arm_body(b, ctx, indent + 2),
        None => "iced::widget::Space::new(iced::Length::Shrink, iced::Length::Shrink).into()"
            .to_string(),
    };
    s.push_str(&format!(" else {{\n{}{}\n{}}};\n", in2, els, in1));
    s.push_str(&format!("{}el\n", in1));
    s.push_str(&format!("{}}}", pad));
    s
}

/// One arm/branch body → a single `Element`: a lone widget, or several wrapped
/// in a `column!`.
fn render_arm_body(body: &[ViewNode], ctx: &ViewCtx, indent: usize) -> String {
    match body {
        [one] => render_view(one, ctx, indent, true),
        many => render_container("column", many, ctx, indent, true, None, None),
    }
}

/// The scrutinee of a view `Match`: a bare `String` state field is matched as a
/// slice (`state.name.as_str()`) so string-literal patterns line up.
fn render_match_scrutinee(scrutinee: &Expr, ctx: &ViewCtx) -> String {
    let rendered = render_expr(&rewrite_expr(scrutinee.clone(), ctx.fields, ctx.enums), None);
    if let Expr::Ident(name) = scrutinee {
        if matches!(ctx.field_ty.get(&to_snake(name)), Some(DeclType::Plain(Type::Text))) {
            return format!("{}.as_str()", rendered);
        }
    }
    rendered
}

/// Walk the view for type mismatches we can explain better than rustc would.
/// Currently: an Iced `Slider`'s value must be convertible to `f64`, which rules
/// out `Long`/`LongLong` (i64) — point the user at `Integer`/`Single`/`Double`.
fn validate_view(node: &ViewNode, field_ty: &HashMap<String, DeclType>, diags: &mut Diagnostics) {
    match node {
        ViewNode::Constrained { child, .. } => validate_view(child, field_ty, diags),
        ViewNode::List { .. } | ViewNode::Table { .. } => diags.error_once(
            "list-in-window",
            "`List`/`Table` are Screen (TUI) widgets — they aren't available in a Window (GUI). \
             For a selectable list in a GUI, compose one from a Column of buttons for now.",
        ),
        ViewNode::Column { children, .. } | ViewNode::Row { children, .. } => {
            children.iter().for_each(|c| validate_view(c, field_ty, diags));
        }
        ViewNode::Match { arms, .. } => {
            for arm in arms {
                arm.body.iter().for_each(|c| validate_view(c, field_ty, diags));
            }
        }
        ViewNode::If { branches, else_body } => {
            for (_, b) in branches {
                b.iter().for_each(|c| validate_view(c, field_ty, diags));
            }
            if let Some(b) = else_body {
                b.iter().for_each(|c| validate_view(c, field_ty, diags));
            }
        }
        ViewNode::Slider { value, .. } => {
            if matches!(field_ty.get(&to_snake(value)), Some(DeclType::Plain(Type::Long | Type::LongLong))) {
                diags.error_once(
                    &format!("slider-i64-{}", to_snake(value)),
                    format!(
                        "A Slider can't bind to `{}` because it's a `Long` (64-bit), which Iced \
                         sliders don't support. Use `Integer`, `Single`, or `Double` for the \
                         bound field.",
                        value
                    ),
                );
            }
        }
        ViewNode::Toggler { value, .. } => {
            if !matches!(field_ty.get(&to_snake(value)), Some(DeclType::Plain(Type::Boolean))) {
                diags.error_once(
                    &format!("toggler-bool-{}", to_snake(value)),
                    format!("A Toggler binds to a `Boolean` state field — `{}` isn't one.", value),
                );
            }
        }
        ViewNode::ProgressBar { value, .. } => {
            let numeric = matches!(
                field_ty.get(&to_snake(value)),
                Some(DeclType::Plain(t)) if !matches!(t, Type::Text | Type::Boolean)
            );
            if !numeric {
                diags.error_once(
                    &format!("progress-num-{}", to_snake(value)),
                    format!("A ProgressBar shows a number — `{}` must be a numeric field.", value),
                );
            }
        }
        ViewNode::TextArea { value } => {
            if !matches!(field_ty.get(&to_snake(value)), Some(DeclType::Named(n)) if n == "TextArea") {
                diags.error_once(
                    &format!("textarea-type-{}", to_snake(value)),
                    format!(
                        "A TextArea binds to a field declared `As TextArea` — `{}` isn't one.",
                        value
                    ),
                );
            }
        }
        ViewNode::Radio { value, .. } => {
            // Iced `radio` values must be `Copy + Eq` — an enum or an integer
            // (floats aren't `Eq`; strings aren't `Copy`).
            let ok = matches!(
                field_ty.get(&to_snake(value)),
                Some(DeclType::Named(_))
                    | Some(DeclType::Plain(
                        Type::Integer | Type::Long | Type::LongLong | Type::Byte
                    ))
            );
            if !ok {
                diags.error_once(
                    &format!("radio-type-{}", to_snake(value)),
                    format!(
                        "A Radio binds to an enum or integer field (the selectable values must be \
                         Copy and comparable) — `{}` isn't one.",
                        value
                    ),
                );
            }
        }
        _ => {}
    }
}

/// Is this state-field type a `TextArea` (multi-line editor)?
fn is_textarea(ty: &DeclType) -> bool {
    matches!(ty, DeclType::Named(n) if n == "TextArea")
}

/// snake_case → PascalCase, for a generated `<Field>Edited` message variant.
fn to_pascal(snake: &str) -> String {
    snake
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// The bound fields of every `TextArea` in the view (snake-cased, deduped).
fn collect_textareas(node: &ViewNode) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(node: &ViewNode, out: &mut Vec<String>) {
        match node {
            ViewNode::Constrained { child, .. } => walk(child, out),
            ViewNode::TextArea { value } => {
                let f = to_snake(value);
                if !out.contains(&f) {
                    out.push(f);
                }
            }
            ViewNode::Column { children, .. } | ViewNode::Row { children, .. } => {
                children.iter().for_each(|c| walk(c, out));
            }
            ViewNode::Match { arms, .. } => {
                for a in arms {
                    a.body.iter().for_each(|c| walk(c, out));
                }
            }
            ViewNode::If { branches, else_body } => {
                for (_, b) in branches {
                    b.iter().for_each(|c| walk(c, out));
                }
                if let Some(b) = else_body {
                    b.iter().for_each(|c| walk(c, out));
                }
            }
            _ => {}
        }
    }
    walk(node, &mut out);
    out
}

/// Which Iced widget functions the view tree references, for the `use` line.
fn collect_widgets(node: &ViewNode, used: &mut Vec<&'static str>) {
    fn add(used: &mut Vec<&'static str>, w: &'static str) {
        if !used.contains(&w) {
            used.push(w);
        }
    }
    match node {
        ViewNode::Constrained { child, .. } => collect_widgets(child, used),
        ViewNode::List { .. } | ViewNode::Table { .. } => {} // TUI-only; rejected by validate_view.
        ViewNode::Column { children, .. } => {
            add(used, "column");
            children.iter().for_each(|c| collect_widgets(c, used));
        }
        ViewNode::Row { children, .. } => {
            add(used, "row");
            children.iter().for_each(|c| collect_widgets(c, used));
        }
        // `Space`/`Image`/`Canvas` use fully-qualified paths, so no import needed.
        ViewNode::Space { .. } | ViewNode::Image { .. } | ViewNode::Canvas { .. } => {}
        ViewNode::Text(_) => add(used, "text"),
        ViewNode::Button { .. } => add(used, "button"),
        ViewNode::TextInput { .. } => add(used, "text_input"),
        ViewNode::Checkbox { .. } => add(used, "checkbox"),
        ViewNode::Slider { .. } => add(used, "slider"),
        ViewNode::Toggler { .. } => add(used, "toggler"),
        ViewNode::ProgressBar { .. } => add(used, "progress_bar"),
        ViewNode::Radio { .. } => add(used, "radio"),
        ViewNode::TextArea { .. } => add(used, "text_editor"),
        ViewNode::Match { arms, .. } => {
            for arm in arms {
                arm.body.iter().for_each(|c| collect_widgets(c, used));
            }
        }
        ViewNode::If { branches, else_body } => {
            for (_, b) in branches {
                b.iter().for_each(|c| collect_widgets(c, used));
            }
            if let Some(b) = else_body {
                b.iter().for_each(|c| collect_widgets(c, used));
            }
        }
    }
}

/// `Text` content: a string literal as-is, a concatenation as its `format!`, and
/// anything else stringified with `format!("{}", …)`.
fn render_text(e: &Expr, ctx: &ViewCtx) -> String {
    let rendered = render_expr(&rewrite_expr(e.clone(), ctx.fields, ctx.enums), None);
    match e {
        Expr::Str(_) => format!("text({})", rendered),
        Expr::Binary { op: BinOp::Concat, .. } => format!("text({})", rendered),
        _ => format!("text(format!(\"{{}}\", {}))", rendered),
    }
}

/// Event bodies skip the resolver, so a string literal assigned to a `String`
/// state field doesn't get its `.to_string()`. Add it here (`status = "x"` →
/// `state.status = "x".to_string()`), recursing through `Match`/`If` bodies.
pub(crate) fn coerce_state_strings(s: &mut Stmt, field_ty: &HashMap<String, DeclType>) {
    match s {
        Stmt::Assign { target: Expr::Field(recv, fname), value, .. }
            if matches!(&**recv, Expr::Ident(n) if n == "state")
                && matches!(field_ty.get(&to_snake(fname)), Some(DeclType::Plain(Type::Text)))
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
                    coerce_state_strings(s2, field_ty);
                }
            }
        }
        Stmt::If { branches, else_body } => {
            for (_, b) in branches {
                for s2 in b {
                    coerce_state_strings(s2, field_ty);
                }
            }
            if let Some(b) = else_body {
                for s2 in b {
                    coerce_state_strings(s2, field_ty);
                }
            }
        }
        _ => {}
    }
}

/// The pieces of an event handler split around an `Await`.
struct AwaitSplit {
    pre: Vec<Stmt>,         // statements before the await (run in the kick-off)
    snapshots: Vec<String>, // `let url = state.url.clone();` for state used in the call
    call_src: String,       // the awaited call, e.g. `Http::get(&url)`
    ret_type: String,       // its result type, e.g. `Result<String, String>`
    blocking: bool,         // wrap the call in `spawn_blocking`
    bind: String,           // continuation binding: `result` (Match) or the Dim name
    cont: Vec<Stmt>,        // continuation statements (run when the result arrives)
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
fn await_split(
    e: &GuiEvent,
    field_ty: &HashMap<String, DeclType>,
    fns: &resolver::FnTable,
    diags: &mut Diagnostics,
) -> Option<AwaitSplit> {
    let idx = e.body.iter().position(stmt_has_await)?;
    match &e.body[idx] {
        Stmt::Match { scrutinee: Expr::Await(call), arms, line } => {
            let info = awaitable_info(call, field_ty, fns, diags)?;
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
            let info = awaitable_info(call, field_ty, fns, diags)?;
            Some(AwaitSplit {
                pre: e.body[..idx].to_vec(),
                snapshots: info.snapshots,
                call_src: info.call_src,
                ret_type: info.ret_type,
                blocking: info.blocking,
                bind: to_snake(name),
                cont: e.body[idx + 1..].to_vec(),
            })
        }
        _ => {
            diags.error_once(
                "await-position",
                "`Await` must be the value of a `Match` (`Match Await Http.Get(url)`) or a \
                 `Dim` (`Dim x = Await …`) inside a Window event.",
            );
            None
        }
    }
}

/// The async task can't borrow `state`, so snapshot (clone) any state fields used
/// as args, and render the call against those owned locals. Returns the `let …`
/// snapshot lines and the rendered argument list.
fn snapshot_args(
    args: &[Expr],
    field_ty: &HashMap<String, DeclType>,
) -> (Vec<String>, Vec<String>) {
    let mut snapshots = Vec::new();
    let mut arg_src = Vec::new();
    for a in args {
        match a {
            Expr::Ident(name) if field_ty.contains_key(&to_snake(name)) => {
                let f = to_snake(name);
                snapshots.push(format!("let {} = state.{}.clone();", f, f));
                if matches!(field_ty.get(&f), Some(DeclType::Plain(Type::Text))) {
                    arg_src.push(format!("&{}", f));
                } else {
                    arg_src.push(f);
                }
            }
            other => arg_src.push(render_expr(other, None)),
        }
    }
    (snapshots, arg_src)
}

/// Resolve an awaited call to its Rust form, result type, and how to run it: a
/// known stdlib call (`Http.Get`), or one of the program's own functions (whose
/// return type the `FnTable` records). Both run off the UI thread.
fn awaitable_info(
    call: &Expr,
    field_ty: &HashMap<String, DeclType>,
    fns: &resolver::FnTable,
    diags: &mut Diagnostics,
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
            let m = to_snake(method);
            let (ret_type, blocking) = match (canon, m.as_str()) {
                ("Http", "get") => ("Result<String, String>".to_string(), true),
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
            let (snapshots, arg_src) = snapshot_args(args, field_ty);
            let call_src = format!("{}::{}({})", canon, m, arg_src.join(", "));
            Some(AwaitInfo { snapshots, call_src, ret_type, blocking })
        }
        // One of the program's own functions — its return type comes from the
        // FnTable; it's synchronous Rust, so run it via `spawn_blocking`.
        Expr::Call { name, args } => {
            let Some(sig) = fns.get(&to_snake(name)) else {
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
            let (snapshots, arg_src) = snapshot_args(args, field_ty);
            let call_src = format!("{}({})", to_snake(name), arg_src.join(", "));
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
                return matches!((c, to_snake(method).as_str()), ("Http", "get"));
            }
        }
    }
    false
}

/// Teaching diagnostic: a blocking stdlib call used in an event *without* `Await`
/// would freeze the window. A call directly under `Await` is fine.
fn check_blocking_without_await(stmts: &[Stmt], diags: &mut Diagnostics) {
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
                 freeze the window until it finishes. Use `Await` so it runs off the UI \
                 thread — e.g. `Match Await Http.Get(url) … End Match`.",
            );
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
fn stmt_has_await(s: &Stmt) -> bool {
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
/// `use vbr_stdlib::{…}` line — and mark them so the dep + feature get added.
fn collect_event_stdlib(stmts: &[Stmt], out: &mut Vec<String>, diags: &mut Diagnostics) {
    fn ex(e: &Expr, out: &mut Vec<String>, diags: &mut Diagnostics) {
        match e {
            Expr::MethodCall { recv, args, .. } => {
                if let Expr::Ident(r) = &**recv {
                    if let Some(c) = stdlib_type(r) {
                        out.push(c.to_string());
                        diags.mark(&format!("stdlib:{}", c));
                    }
                }
                ex(recv, out, diags);
                for a in args {
                    ex(a, out, diags);
                }
            }
            Expr::Await(i) | Expr::Not(i) | Expr::Ref(i) | Expr::MutRef(i) | Expr::Deref(i)
            | Expr::Cast(i, _) | Expr::Try(i) | Expr::Field(i, _) | Expr::TupleIndex(i, _)
            | Expr::Closure { body: i, .. } => ex(i, out, diags),
            Expr::Binary { lhs, rhs, .. } | Expr::Index(lhs, rhs) => {
                ex(lhs, out, diags);
                ex(rhs, out, diags);
            }
            Expr::Call { args, .. } => {
                for a in args {
                    ex(a, out, diags);
                }
            }
            Expr::Tuple(es) => {
                for e2 in es {
                    ex(e2, out, diags);
                }
            }
            Expr::StructLit { fields, .. } => {
                for (_, v) in fields {
                    ex(v, out, diags);
                }
            }
            _ => {}
        }
    }
    fn st(s: &Stmt, out: &mut Vec<String>, diags: &mut Diagnostics) {
        match s {
            Stmt::Assign { target, value, .. } => {
                ex(target, out, diags);
                ex(value, out, diags);
            }
            Stmt::Dim { init: Some(e), .. } => ex(e, out, diags),
            Stmt::Print(e) | Stmt::Expr(e) | Stmt::Return(Some(e)) => ex(e, out, diags),
            Stmt::If { branches, else_body } => {
                for (c, b) in branches {
                    ex(c, out, diags);
                    for s2 in b {
                        st(s2, out, diags);
                    }
                }
                if let Some(b) = else_body {
                    for s2 in b {
                        st(s2, out, diags);
                    }
                }
            }
            Stmt::Match { scrutinee, arms, .. } => {
                ex(scrutinee, out, diags);
                for a in arms {
                    for s2 in &a.body {
                        st(s2, out, diags);
                    }
                }
            }
            Stmt::For { body, .. } | Stmt::ForEach { body, .. } | Stmt::DoLoop { body, .. } => {
                for s2 in body {
                    st(s2, out, diags);
                }
            }
            _ => {}
        }
    }
    for s in stmts {
        st(s, out, diags);
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
fn rewrite_expr_with(
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
        Expr::Ident(name) if fields.contains(&to_snake(&name)) => {
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
        other => other,
    }
}

pub(crate) fn rewrite_stmt(s: Stmt, fields: &HashSet<String>, enums: &HashSet<String>) -> Stmt {
    match s {
        Stmt::Assign { target, value, op } => Stmt::Assign {
            target: rewrite_expr(target, fields, enums),
            value: rewrite_expr(value, fields, enums),
            op,
        },
        Stmt::Print(e) => Stmt::Print(rewrite_expr(e, fields, enums)),
        Stmt::Expr(e) => Stmt::Expr(rewrite_expr(e, fields, enums)),
        Stmt::If { branches, else_body } => Stmt::If {
            branches: branches
                .into_iter()
                .map(|(c, b)| {
                    (
                        rewrite_expr(c, fields, enums),
                        b.into_iter().map(|s| rewrite_stmt(s, fields, enums)).collect(),
                    )
                })
                .collect(),
            else_body: else_body
                .map(|b| b.into_iter().map(|s| rewrite_stmt(s, fields, enums)).collect()),
        },
        Stmt::Match { scrutinee, arms, line } => Stmt::Match {
            scrutinee: rewrite_expr(scrutinee, fields, enums),
            arms: arms
                .into_iter()
                .map(|a| MatchArm {
                    pattern: a.pattern,
                    guard: a.guard.map(|g| rewrite_expr(g, fields, enums)),
                    body: a.body.into_iter().map(|s| rewrite_stmt(s, fields, enums)).collect(),
                })
                .collect(),
            line,
        },
        Stmt::Dim { name, ty, init, line } => Stmt::Dim {
            name,
            ty,
            init: init.map(|e| rewrite_expr(e, fields, enums)),
            line,
        },
        other => other,
    }
}

// ── Canvas support ──────────────────────────────────────────────────────────

/// The set of *paint functions*: those that draw. A function draws if its body
/// contains a drawing verb, or (transitively) calls a function that does. These
/// are emitted with a leading `frame` parameter instead of the normal way.
fn paint_fn_set(program: &Program) -> HashSet<String> {
    let mut set: HashSet<String> = program
        .functions
        .iter()
        .filter(|f| f.receiver.is_none() && body_has_draw(&f.body))
        .map(|f| to_snake(&f.name))
        .collect();
    loop {
        let mut changed = false;
        for f in &program.functions {
            if f.receiver.is_some() {
                continue;
            }
            let n = to_snake(&f.name);
            if !set.contains(&n) && body_calls_any(&f.body, &set) {
                set.insert(n);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    set
}

fn body_has_draw(stmts: &[Stmt]) -> bool {
    stmts.iter().any(stmt_has_draw)
}

fn stmt_has_draw(s: &Stmt) -> bool {
    match s {
        Stmt::Draw(_) => true,
        Stmt::If { branches, else_body } => {
            branches.iter().any(|(_, b)| body_has_draw(b))
                || else_body.as_ref().map_or(false, |b| body_has_draw(b))
        }
        Stmt::For { body, .. } | Stmt::ForEach { body, .. } | Stmt::DoLoop { body, .. } => {
            body_has_draw(body)
        }
        Stmt::Match { arms, .. } => arms.iter().any(|a| body_has_draw(&a.body)),
        _ => false,
    }
}

fn body_calls_any(stmts: &[Stmt], set: &HashSet<String>) -> bool {
    stmts.iter().any(|s| stmt_calls_any(s, set))
}

fn stmt_calls_any(s: &Stmt, set: &HashSet<String>) -> bool {
    match s {
        Stmt::Expr(Expr::Call { name, .. }) => set.contains(&to_snake(name)),
        Stmt::If { branches, else_body } => {
            branches.iter().any(|(_, b)| body_calls_any(b, set))
                || else_body.as_ref().map_or(false, |b| body_calls_any(b, set))
        }
        Stmt::For { body, .. } | Stmt::ForEach { body, .. } | Stmt::DoLoop { body, .. } => {
            body_calls_any(body, set)
        }
        Stmt::Match { arms, .. } => arms.iter().any(|a| body_calls_any(&a.body, set)),
        _ => false,
    }
}

/// The canvas names placed in a view (`Canvas Board`), in first-seen order.
fn collect_canvases(node: &ViewNode) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(node: &ViewNode, out: &mut Vec<String>) {
        match node {
            ViewNode::Constrained { child, .. } => walk(child, out),
            ViewNode::Canvas { name, .. } => {
                if !out.iter().any(|n: &String| n.eq_ignore_ascii_case(name)) {
                    out.push(name.clone());
                }
            }
            ViewNode::Column { children, .. } | ViewNode::Row { children, .. } => {
                children.iter().for_each(|c| walk(c, out));
            }
            ViewNode::Match { arms, .. } => {
                for a in arms {
                    a.body.iter().for_each(|c| walk(c, out));
                }
            }
            ViewNode::If { branches, else_body } => {
                for (_, b) in branches {
                    b.iter().for_each(|c| walk(c, out));
                }
                if let Some(b) = else_body {
                    b.iter().for_each(|c| walk(c, out));
                }
            }
            _ => {}
        }
    }
    walk(node, &mut out);
    out
}

/// The snake-cased identifiers a canvas `Draw` block mentions — used to work out
/// which window state fields to snapshot into the Program.
fn canvas_idents(body: &[Stmt]) -> HashSet<String> {
    let mut out = HashSet::new();
    for s in body {
        collect_stmt_idents(s, &mut out);
    }
    out
}

fn collect_stmt_idents(s: &Stmt, out: &mut HashSet<String>) {
    match s {
        Stmt::Draw(cmd) => collect_drawcmd_idents(cmd, out),
        Stmt::Dim { init: Some(e), .. } => collect_expr_idents(e, out),
        Stmt::Assign { target, value, .. } => {
            collect_expr_idents(target, out);
            collect_expr_idents(value, out);
        }
        Stmt::Print(e) | Stmt::Expr(e) | Stmt::Return(Some(e)) => collect_expr_idents(e, out),
        Stmt::If { branches, else_body } => {
            for (c, b) in branches {
                collect_expr_idents(c, out);
                b.iter().for_each(|s| collect_stmt_idents(s, out));
            }
            if let Some(b) = else_body {
                b.iter().for_each(|s| collect_stmt_idents(s, out));
            }
        }
        Stmt::For { from, to, step, body, .. } => {
            collect_expr_idents(from, out);
            collect_expr_idents(to, out);
            if let Some(st) = step {
                collect_expr_idents(st, out);
            }
            body.iter().for_each(|s| collect_stmt_idents(s, out));
        }
        Stmt::ForEach { iter, body, .. } => {
            collect_expr_idents(iter, out);
            body.iter().for_each(|s| collect_stmt_idents(s, out));
        }
        Stmt::DoLoop { body, .. } => body.iter().for_each(|s| collect_stmt_idents(s, out)),
        Stmt::Match { scrutinee, arms, .. } => {
            collect_expr_idents(scrutinee, out);
            for a in arms {
                a.body.iter().for_each(|s| collect_stmt_idents(s, out));
            }
        }
        _ => {}
    }
}

fn collect_drawcmd_idents(cmd: &DrawCmd, out: &mut HashSet<String>) {
    let shape = |sh: &Shape, out: &mut HashSet<String>| match sh {
        Shape::Circle(a, b, c) => {
            [a, b, c].iter().for_each(|e| collect_expr_idents(e, out));
        }
        Shape::Rect(a, b, c, d) | Shape::Line(a, b, c, d) => {
            [a, b, c, d].iter().for_each(|e| collect_expr_idents(e, out));
        }
    };
    match cmd {
        DrawCmd::Fill { shape: sh, color } => {
            shape(sh, out);
            collect_expr_idents(color, out);
        }
        DrawCmd::Stroke { shape: sh, color, width } => {
            shape(sh, out);
            collect_expr_idents(color, out);
            if let Some(w) = width {
                collect_expr_idents(w, out);
            }
        }
        DrawCmd::Text { text, x, y, color } => {
            collect_expr_idents(text, out);
            collect_expr_idents(x, out);
            collect_expr_idents(y, out);
            if let Some(c) = color {
                collect_expr_idents(c, out);
            }
        }
        DrawCmd::Paint { args, .. } => args.iter().for_each(|e| collect_expr_idents(e, out)),
    }
}

fn collect_expr_idents(e: &Expr, out: &mut HashSet<String>) {
    match e {
        Expr::Ident(n) => {
            out.insert(to_snake(n));
        }
        Expr::Binary { lhs, rhs, .. } | Expr::Index(lhs, rhs) => {
            collect_expr_idents(lhs, out);
            collect_expr_idents(rhs, out);
        }
        Expr::Not(i) | Expr::Ref(i) | Expr::MutRef(i) | Expr::Deref(i) | Expr::Cast(i, _)
        | Expr::Try(i) | Expr::Field(i, _) | Expr::TupleIndex(i, _) | Expr::Closure { body: i, .. } => {
            collect_expr_idents(i, out)
        }
        Expr::MethodCall { recv, args, .. } => {
            collect_expr_idents(recv, out);
            args.iter().for_each(|a| collect_expr_idents(a, out));
        }
        Expr::Call { args, .. } => args.iter().for_each(|a| collect_expr_idents(a, out)),
        Expr::Tuple(es) => es.iter().for_each(|e| collect_expr_idents(e, out)),
        Expr::StructLit { fields, .. } => fields.iter().for_each(|(_, v)| collect_expr_idents(v, out)),
        _ => {}
    }
}

/// Rewrite a statement in a canvas `Draw` block / paint function: state fields
/// become `self.field`, enum variants get their path, and a call to a paint
/// function becomes a `Paint` draw command (so the shared `frame` is threaded).
fn rewrite_canvas_stmt(
    s: Stmt,
    fields: &HashSet<String>,
    enums: &HashSet<String>,
    paint_fns: &HashSet<String>,
) -> Stmt {
    let re = |e: Expr| rewrite_expr_with(e, "self", fields, enums);
    let rec = |s: Stmt| rewrite_canvas_stmt(s, fields, enums, paint_fns);
    match s {
        Stmt::Expr(Expr::Call { name, args }) if paint_fns.contains(&to_snake(&name)) => {
            Stmt::Draw(DrawCmd::Paint { name, args: args.into_iter().map(re).collect() })
        }
        Stmt::Draw(cmd) => Stmt::Draw(rewrite_draw_cmd(cmd, fields, enums)),
        Stmt::Expr(e) => Stmt::Expr(re(e)),
        Stmt::Print(e) => Stmt::Print(re(e)),
        Stmt::Assign { target, value, op } => Stmt::Assign { target: re(target), value: re(value), op },
        Stmt::Dim { name, ty, init, line } => Stmt::Dim { name, ty, init: init.map(re), line },
        Stmt::If { branches, else_body } => Stmt::If {
            branches: branches
                .into_iter()
                .map(|(c, b)| (re(c), b.into_iter().map(rec).collect()))
                .collect(),
            else_body: else_body.map(|b| b.into_iter().map(rec).collect()),
        },
        Stmt::For { var, from, to, step, body } => Stmt::For {
            var,
            from: re(from),
            to: re(to),
            step: step.map(re),
            body: body.into_iter().map(rec).collect(),
        },
        Stmt::ForEach { var1, var2, iter, body } => Stmt::ForEach {
            var1,
            var2,
            iter: re(iter),
            body: body.into_iter().map(rec).collect(),
        },
        Stmt::Match { scrutinee, arms, line } => Stmt::Match {
            scrutinee: re(scrutinee),
            arms: arms
                .into_iter()
                .map(|a| MatchArm {
                    pattern: a.pattern,
                    guard: a.guard.map(re),
                    body: a.body.into_iter().map(rec).collect(),
                })
                .collect(),
            line,
        },
        Stmt::DoLoop { cond, body } => Stmt::DoLoop { cond, body: body.into_iter().map(rec).collect() },
        other => other,
    }
}

fn rewrite_draw_cmd(cmd: DrawCmd, fields: &HashSet<String>, enums: &HashSet<String>) -> DrawCmd {
    let re = |e: Expr| rewrite_expr_with(e, "self", fields, enums);
    let rs = |sh: Shape| rewrite_shape(sh, fields, enums);
    match cmd {
        DrawCmd::Fill { shape, color } => DrawCmd::Fill { shape: rs(shape), color: re(color) },
        DrawCmd::Stroke { shape, color, width } => {
            DrawCmd::Stroke { shape: rs(shape), color: re(color), width: width.map(re) }
        }
        DrawCmd::Text { text, x, y, color } => {
            DrawCmd::Text { text: re(text), x: re(x), y: re(y), color: color.map(re) }
        }
        DrawCmd::Paint { name, args } => DrawCmd::Paint { name, args: args.into_iter().map(re).collect() },
    }
}

fn rewrite_shape(sh: Shape, fields: &HashSet<String>, enums: &HashSet<String>) -> Shape {
    let re = |e: Expr| rewrite_expr_with(e, "self", fields, enums);
    match sh {
        Shape::Circle(a, b, c) => Shape::Circle(re(a), re(b), re(c)),
        Shape::Rect(a, b, c, d) => Shape::Rect(re(a), re(b), re(c), re(d)),
        Shape::Line(a, b, c, d) => Shape::Line(re(a), re(b), re(c), re(d)),
    }
}
