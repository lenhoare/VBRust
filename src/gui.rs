//! GUI codegen — a `Window` becomes an Iced 0.13 application.
//!
//! Slice 1: `Window` / `State` / `View` / `Event` → a `State` struct, a `Message`
//! enum, an `update` function, a `view` function, and `main`. The model is The
//! Elm Architecture, which is what Iced is: state is the source of truth, the
//! view is derived from it, and events update it.

use crate::ast::*;
use crate::diagnostics::Diagnostics;
use crate::surface::{
    self, analyze_events, coerce_state_strings, event_stdlib_imports, launched, match_scrutinee,
    rewrite_expr, rewrite_expr_with, state_maps, AwaitSplit,
};
use crate::transpiler::{collect_mutated, collect_stmt_idents, decltype_rust, elide_for_counter_dims, emit_stmt, render_expr, rust_name};
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
pub fn emit_gui_program(
    program: &Program,
    modules: &[String],
    interfaces: &crate::resolver::ProjectInterfaces,
    is_entry: bool,
    diags: &mut Diagnostics,
) -> String {
    let mut out = String::new();
    // The crate root of a multi-file project declares its sibling modules,
    // exactly as a plain program's entry does.
    surface::emit_mod_decls(modules, is_entry, &mut out);
    let t = surface::build_tables(program, modules, interfaces);
    // Paint functions (those that issue drawing verbs) are emitted specially —
    // they take an extra `frame` and their draw commands lower to `frame.*` calls.
    let paint_fns = paint_fn_set(program);
    surface::emit_shared_items(program, &t, diags, &mut out, &mut |f, diags, out| {
        if paint_fns.contains(&rust_name(&f.name)) {
            emit_paint_fn(f, &t, &paint_fns, diags, out);
            true
        } else {
            false
        }
    });

    if program
        .functions
        .iter()
        .any(|f| crate::transpiler::uses_file_dialog(&f.body))
        || program.windows.iter().any(|w| {
            w.events.iter().any(|e| crate::transpiler::uses_file_dialog(&e.body))
                || w.subs.iter().any(|s| crate::transpiler::uses_file_dialog(&s.body))
        })
        || program.sketches.iter().any(|s| {
            s.events.iter().any(|e| crate::transpiler::uses_file_dialog(&e.body))
                || s.subs.iter().any(|sub| crate::transpiler::uses_file_dialog(&sub.body))
        })
    {
        diags.error_once(
            "tui-file-dialog-window",
            "GetOpenFilename / GetSaveAsFilename are Screen-only (they pop a terminal path \
             prompt). A Window has no terminal overlay — use a Screen, or pick the path \
             another way.",
        );
    }

    // Every non-event stdlib namespace the program names: declared types +
    // `State` inits + call receivers in the helper functions (emit_shared_items
    // above already ran note_builtins on them). Read from the marks so a
    // namespace used only *inside* a helper (e.g. `FileSystem.Read`) is imported
    // too. Event namespaces are merged in per-window below (they mark later).
    crate::transpiler::stdlib_types_declared(program, diags);
    for w in &program.windows {
        surface::state_stdlib(&w.state, diags);
    }
    for s in &program.sketches {
        surface::state_stdlib(&s.state, diags);
    }
    let std_top = crate::transpiler::stdlib_used(diags);

    for w in &program.windows {
        out.push_str(&emit_window(w, &t, &program.canvases, &paint_fns, &std_top, &program.functions, diags));
        out.push('\n');
    }
    for s in &program.sketches {
        out.push_str(&emit_sketch(s, &t, &paint_fns, &std_top, &program.functions, diags));
        out.push('\n');
    }
    let launched_window = launched(program, |name| {
        program.windows.iter().find(|w| w.name.eq_ignore_ascii_case(name))
    });
    let launched_sketch = launched(program, |name| {
        program.sketches.iter().find(|s| s.name.eq_ignore_ascii_case(name))
    });
    match (launched_window, launched_sketch) {
        (Some(w), None) => out.push_str(&emit_main(w, surface::state_fallible(&w.state, &t))),
        (None, Some(s)) => out.push_str(&emit_sketch_main(s, surface::state_fallible(&s.state, &t))),
        (Some(_), Some(_)) => diags.error_once(
            "gui-two-launches",
            "Function Main() launches both a Window and a Sketch. Launch one: `<Name>.Run`.",
        ),
        (None, None) => diags.error_once(
            "gui-no-launch",
            "A window or sketch is never launched. Add `Function Main()` containing `<Name>.Run`, \
             e.g. `Circles.Run`.",
        ),
    }
    out
}

/// Canonical theme names (`Dracula`, `NightOwl`, …). Shared with the Screen and Page.
pub(crate) fn known_theme_names() -> String {
    crate::theme::names().join(", ")
}

/// The canonical (PascalCase) name of a built-in theme, matched case-insensitively.
pub(crate) fn canonical_theme(name: &str) -> Option<&'static str> {
    crate::theme::lookup(name).map(|t| t.name)
}

/// `fn main` for a GUI: run the window. With a `Theme`, use the `application`
/// builder so we can set `.theme(...)`; otherwise the simpler `iced::run`.
/// With a fallible `State` initialiser, boot goes through `run_with`, building
/// the state via `init()` and bailing out cleanly before any window opens.
fn emit_main(w: &Window, fallible: bool) -> String {
    let title = w.title.clone().unwrap_or_else(|| w.name.clone());
    let theme_line = w.theme.as_ref().and_then(|t| crate::theme::lookup(t)).map(|spec| {
        format!("        .theme(|_| {})\n", spec.iced_expr())
    });
    if fallible {
        return format!(
            "fn main() -> iced::Result {{\n    \
             iced::application({:?}, update, view)\n\
             {}        \
             .run_with(|| match {}::init() {{\n            \
             Ok(state) => (state, iced::Task::none()),\n            \
             Err(message) => {{\n                \
             eprintln!(\"could not start: {{}}\", message);\n                \
             std::process::exit(1);\n            \
             }}\n        \
             }})\n}}\n",
            title,
            theme_line.as_deref().unwrap_or(""),
            w.name
        );
    }
    match theme_line {
        Some(theme) => format!(
            "fn main() -> iced::Result {{\n    \
             iced::application({:?}, update, view)\n\
             {}        \
             .run()\n}}\n",
            title, theme
        ),
        None => format!(
            "fn main() -> iced::Result {{\n    iced::run({:?}, update, view)\n}}\n",
            title
        ),
    }
}

/// Emit one sketch's definition — State, optional Message/update, a filling
/// canvas view, and the canvas Program. `fn main` is emitted separately.
fn emit_sketch(
    s: &Sketch,
    t: &surface::Tables,
    paint_fns: &HashSet<String>,
    std_top: &[String],
    helpers: &[Function],
    diags: &mut Diagnostics,
) -> String {
    let mut out = String::new();
    let ty = &s.name;
    let t_local = surface::with_subs(t, &s.subs);
    let t = &t_local;
    let (fields, field_ty) = state_maps(&s.state);

    for tm in &s.timers {
        if !s.events.iter().any(|e| e.name.eq_ignore_ascii_case(&tm.handler)) {
            diags.error_once(
                &format!("sketch-timer-{}", rust_name(&tm.handler)),
                format!(
                    "`Every {} {}` needs an `Event {}` on this Sketch to run.",
                    tm.interval_ms, tm.handler, tm.handler
                ),
            );
        }
    }

    let splits: Vec<Option<AwaitSplit>> =
        analyze_events(&s.events, &field_ty, &t.fns, diags, surface::AsyncBackend::Native);
    let any_async = splits.iter().any(Option::is_some);
    let has_messages = !s.events.is_empty();

    out.push_str("use iced::Element;\n");
    if any_async {
        out.push_str("use iced::Task;\n");
    }
    out.push_str(&surface::surface_std_imports(&s.events, helpers));
    let mut std_used = event_stdlib_imports(&s.events, diags);
    for ns in std_top {
        if !std_used.iter().any(|u| u == ns) {
            std_used.push(ns.clone());
        }
    }
    if !std_used.is_empty() {
        out.push_str(&format!("use vbr_stdlib::{{{}}};\n", std_used.join(", ")));
    }
    out.push('\n');

    // ── State struct ──
    if s.state.is_empty() {
        out.push_str(&format!("struct {};\n\n", ty));
    } else {
        out.push_str(&format!("struct {} {{\n", ty));
        for f in &s.state {
            out.push_str(&format!("    {}: {},\n", rust_name(&f.name), decltype_rust(&f.ty)));
        }
        out.push_str("}\n\n");
    }

    let fallible = surface::state_fallible(&s.state, &t);
    if s.state.is_empty() {
        if fallible {
            out.push_str(&format!(
                "impl {} {{\n    fn init() -> Result<{}, String> {{ Ok({} {{}}) }}\n}}\n\n",
                ty, ty, ty
            ));
        } else {
            out.push_str(&format!("impl Default for {} {{\n    fn default() -> Self {{ {} }}\n}}\n\n", ty, ty));
        }
    } else if fallible {
        out.push_str(&format!(
            "impl {} {{\n    fn init() -> Result<{}, String> {{\n",
            ty, ty
        ));
        let names = surface::emit_state_lets(&s.state, &t, 2, diags, &mut out, |_| None);
        out.push_str(&format!("        Ok({} {{\n", ty));
        for name in &names {
            out.push_str(&format!("            {},\n", name));
        }
        out.push_str("        })\n    }\n}\n\n");
    } else {
        out.push_str(&format!("impl Default for {} {{\n    fn default() -> Self {{\n", ty));
        let names = surface::emit_state_lets(&s.state, &t, 2, diags, &mut out, |_| None);
        out.push_str(&format!("        {} {{\n", ty));
        for name in &names {
            out.push_str(&format!("            {},\n", name));
        }
        out.push_str("        }\n    }\n}\n\n");
    }

    surface::emit_subs(&s.subs, ty, &fields, &field_ty, t, diags, &mut out);

    let msg_ty = if has_messages { "Message" } else { "()" };

    if has_messages {
        out.push_str("#[derive(Debug, Clone)]\nenum Message {\n");
        for (e, split) in s.events.iter().zip(&splits) {
            if e.params.is_empty() {
                out.push_str(&format!("    {},\n", e.name));
            } else {
                let types: Vec<String> = e.params.iter().map(|p| decltype_rust(&p.ty)).collect();
                out.push_str(&format!("    {}({}),\n", e.name, types.join(", ")));
            }
            if let Some(sp) = split {
                out.push_str(&format!("    {}Done({}),\n", e.name, sp.ret_type));
            }
        }
        out.push_str("}\n\n");
    }

    let ret = if any_async { " -> Task<Message>" } else { "" };
    let state_param = if s.events.is_empty() { "_state" } else { "state" };
    if has_messages {
        out.push_str(&format!(
            "fn update({}: &mut {}, message: Message){} {{\n",
            state_param, ty, ret
        ));
        out.push_str("    match message {\n");
        for (e, split) in s.events.iter().zip(&splits) {
            if e.params.is_empty() {
                out.push_str(&format!("        Message::{} => {{\n", e.name));
            } else {
                let binds: Vec<String> = e.params.iter().map(|p| rust_name(&p.name)).collect();
                out.push_str(&format!("        Message::{}({}) => {{\n", e.name, binds.join(", ")));
            }
            match split {
                None => {
                    surface::emit_event_stmts_caught(&e.body, &e.params, "state", &fields, &field_ty, t, 3, diags, &mut out);
                    if any_async {
                        out.push_str("            Task::none()\n");
                    }
                }
                Some(sp) => {
                    surface::emit_event_stmts_caught(&sp.pre, &e.params, "state", &fields, &field_ty, t, 3, diags, &mut out);
                    for snap in &sp.snapshots {
                        out.push_str(&format!("            {}\n", snap));
                    }
                    let work = if sp.blocking {
                        format!(
                            "async move {{ tokio::task::spawn_blocking(move || {}).await.unwrap() }}",
                            sp.call_src
                        )
                    } else {
                        format!("async move {{ {} }}", sp.call_src)
                    };
                    out.push_str(&format!(
                        "            Task::perform({}, Message::{}Done)\n",
                        work, e.name
                    ));
                }
            }
            out.push_str("        }\n");
            if let Some(sp) = split {
                out.push_str(&format!("        Message::{}Done({}) => {{\n", e.name, sp.bind));
                surface::emit_event_stmts_caught(&sp.cont, &e.params, "state", &fields, &field_ty, t, 3, diags, &mut out);
                out.push_str("            Task::none()\n");
                out.push_str("        }\n");
            }
        }
        out.push_str("    }\n}\n\n");
    } else {
        out.push_str(&format!("fn update(_state: &mut {}, _message: ()) {{}}\n\n", ty));
    }

    if !s.timers.is_empty() {
        out.push_str(&format!(
            "fn subscription(_state: &{}) -> iced::Subscription<{}> {{\n",
            ty, msg_ty
        ));
        if s.timers.len() == 1 {
            let tm = &s.timers[0];
            out.push_str(&format!(
                "    iced::time::every(std::time::Duration::from_millis({})).map(|_| Message::{})\n",
                tm.interval_ms, tm.handler
            ));
        } else {
            out.push_str("    iced::Subscription::batch([\n");
            for tm in &s.timers {
                out.push_str(&format!(
                    "        iced::time::every(std::time::Duration::from_millis({})).map(|_| Message::{}),\n",
                    tm.interval_ms, tm.handler
                ));
            }
            out.push_str("    ])\n");
        }
        out.push_str("}\n\n");
    }

    let idents = canvas_idents(&s.draw);
    let snap: Vec<String> = s
        .state
        .iter()
        .map(|f| rust_name(&f.name))
        .filter(|f| idents.contains(f))
        .collect();
    let inits: Vec<String> = snap
        .iter()
        .map(|f| {
            let is_copy = matches!(
                field_ty.get(f),
                Some(DeclType::Plain(t)) if !matches!(t, Type::Text)
            );
            if is_copy {
                format!("{}: state.{}", f, f)
            } else {
                format!("{}: state.{}.clone()", f, f)
            }
        })
        .collect();
    let canvas_new = if snap.is_empty() {
        format!("{}Canvas", s.name)
    } else {
        format!("{}Canvas {{ {} }}", s.name, inits.join(", "))
    };
    out.push_str(&format!("fn view(state: &{}) -> Element<'_, {}> {{\n", ty, msg_ty));
    if snap.is_empty() && s.state.is_empty() {
        out.push_str("    let _ = state;\n");
    } else if snap.is_empty() {
        out.push_str("    let _ = state;\n");
    }
    out.push_str(&format!(
        "    iced::widget::Canvas::new({}).width(iced::Length::Fill).height(iced::Length::Fill).into()\n",
        canvas_new
    ));
    out.push_str("}\n\n");

    let bg = match &s.background {
        Some(e) => crate::transpiler::render_color(e, diags),
        None => "iced::Color::BLACK".to_string(),
    };
    let cv = CanvasDef {
        name: s.name.clone(),
        body: s.draw.clone(),
    };
    emit_canvas_program(&cv, &snap, Some(&bg), &fields, &field_ty, t, paint_fns, diags, &mut out);

    out
}

/// `fn main` for a Sketch: sized Iced window, optional timer subscription.
fn emit_sketch_main(s: &Sketch, fallible: bool) -> String {
    let title = s.title.clone().unwrap_or_else(|| s.name.clone());
    let size = format!(
        "        .window_size(iced::Size::new({}.0, {}.0))\n",
        s.width, s.height
    );
    let sub = if s.timers.is_empty() {
        String::new()
    } else {
        "        .subscription(subscription)\n".to_string()
    };
    if fallible {
        return format!(
            "fn main() -> iced::Result {{\n    \
             iced::application({:?}, update, view)\n\
             {}{}        \
             .run_with(|| match {}::init() {{\n            \
             Ok(state) => (state, iced::Task::none()),\n            \
             Err(message) => {{\n                \
             eprintln!(\"could not start: {{}}\", message);\n                \
             std::process::exit(1);\n            \
             }}\n        \
             }})\n}}\n",
            title, size, sub, s.name
        );
    }
    format!(
        "fn main() -> iced::Result {{\n    \
         iced::application({:?}, update, view)\n\
         {}{}        \
         .run()\n}}\n",
        title, size, sub
    )
}

/// Emit one window's *definition* — the State struct, Message enum, update, and
/// view. (`fn main` is emitted separately, from the launch in `Function Main()`.)
fn emit_window(
    w: &Window,
    t: &surface::Tables,
    canvases: &[CanvasDef],
    paint_fns: &HashSet<String>,
    std_top: &[String],
    helpers: &[Function],
    diags: &mut Diagnostics,
) -> String {
    let mut out = String::new();
    let ty = &w.name; // the state struct is named after the window
    // A per-window view of the tables carrying this window's `Sub` helpers, so a
    // call to one inside an event lowers to a method on the state.
    let t_local = surface::with_subs(t, &w.subs);
    let t = &t_local;
    let enums = &t.enums;
    let (fields, field_ty) = state_maps(&w.state);

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
                    .map(|f| rust_name(&f.name))
                    .filter(|f| idents.contains(f))
                    .collect();
                canvas_snaps.insert(cname.clone(), snap);
            }
            None => diags.error_once(
                &format!("unknown-canvas-{}", rust_name(cname)),
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
                    known_theme_names()
                ),
            );
        }
    }

    // Analyse each event for `Await`: an async event splits into a kick-off arm
    // (returns a `Task`) and a generated `<Event>Done(...)` continuation arm. If
    // any event is async, the whole `update` returns `Task<Message>`.
    let splits: Vec<Option<AwaitSplit>> =
        analyze_events(&w.events, &field_ty, &t.fns, diags, surface::AsyncBackend::Native);
    let any_async = splits.iter().any(Option::is_some);

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
    // `std` types used in event bodies or helper functions (e.g. an `Http.Post`
    // headers HashMap, or one built in a helper like `ChatComplete`).
    out.push_str(&surface::surface_std_imports(&w.events, helpers));
    // vbr_stdlib namespaces: those called in events, plus item-level types /
    // `State` initialisers (`Database` for a db held in state).
    let mut std_used = event_stdlib_imports(&w.events, diags);
    for ns in std_top {
        if !std_used.iter().any(|u| u == ns) {
            std_used.push(ns.clone());
        }
    }
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
        out.push_str(&format!("    {}: {},\n", rust_name(&f.name), fty));
    }
    out.push_str("}\n\n");

    // ── Initial state (the Dim initialisers) ──
    // Infallible initialisers make a plain `Default`; any fallible one (a call
    // returning a Result, like `Database.Open`) switches to
    // `fn init() -> Result<Self, String>`, run at boot with a clean bail-out.
    let fallible = surface::state_fallible(&w.state, &t);
    if fallible {
        out.push_str(&format!(
            "impl {} {{\n    fn init() -> Result<{}, String> {{\n",
            ty, ty
        ));
    } else {
        out.push_str(&format!("impl Default for {} {{\n    fn default() -> Self {{\n", ty));
    }
    // The initialisers as sequential `let`s (so a field can read a sibling),
    // then the struct built from them. A `TextEditor` field wraps its text in
    // a `text_editor::Content` instead of the ordinary `render_init`.
    let names = surface::emit_state_lets(&w.state, &t, 2, diags, &mut out, |f| {
        if is_textarea(&f.ty) {
            let text = f
                .init
                .as_ref()
                .map(|e| render_expr(e, None))
                .unwrap_or_else(|| "\"\"".to_string());
            Some(format!("iced::widget::text_editor::Content::with_text({})", text))
        } else {
            None
        }
    });
    if fallible {
        out.push_str(&format!("        Ok({} {{\n", ty));
    } else {
        out.push_str(&format!("        {} {{\n", ty));
    }
    for name in &names {
        out.push_str(&format!("            {},\n", name));
    }
    if fallible {
        out.push_str("        })\n    }\n}\n\n");
    } else {
        out.push_str("        }\n    }\n}\n\n");
    }

    // ── in-block `Sub` helpers → methods on the state ──
    surface::emit_subs(&w.subs, ty, &fields, &field_ty, t, diags, &mut out);

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
            let binds: Vec<String> = e.params.iter().map(|p| rust_name(&p.name)).collect();
            out.push_str(&format!("        Message::{}({}) => {{\n", e.name, binds.join(", ")));
        }
        match split {
            // Synchronous event: run the body; async windows need a `Task::none()`.
            None => {
                surface::emit_event_stmts_caught(&e.body, &e.params, "state", &fields, &field_ty, t, 3, diags, &mut out);
                if any_async {
                    out.push_str("            Task::none()\n");
                }
            }
            // Async kick-off: pre-await body, snapshot state, then return the Task.
            Some(s) => {
                surface::emit_event_stmts_caught(&s.pre, &e.params, "state", &fields, &field_ty, t, 3, diags, &mut out);
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
            surface::emit_event_stmts_caught(&s.cont, &e.params, "state", &fields, &field_ty, t, 3, diags, &mut out);
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
            emit_canvas_program(cv, snap, None, &fields, &field_ty, t, paint_fns, diags, &mut out);
        }
    }

    out
}

/// Emit a canvas's Iced `Program` — the snapshot struct plus a `draw` method that
/// runs the `Draw` block against a `frame`. State fields the block reads become
/// `self.field`; calls to paint functions thread the shared `frame` through.
/// `background` is an already-rendered `iced::Color` expression; when set, the
/// paper is filled before the `Draw` verbs (a Sketch's `Background`).
fn emit_canvas_program(
    cv: &CanvasDef,
    snap: &[String],
    background: Option<&str>,
    fields: &HashSet<String>,
    field_ty: &HashMap<String, DeclType>,
    t: &surface::Tables,
    paint_fns: &HashSet<String>,
    diags: &mut Diagnostics,
    out: &mut String,
) {
    let enums = &t.enums;
    let struct_name = format!("{}Canvas", cv.name);
    // The snapshot struct: a copy of just the state the drawing reads.
    if snap.is_empty() {
        out.push_str(&format!("struct {};\n\n", struct_name));
    } else {
        out.push_str(&format!("struct {} {{\n", struct_name));
        for f in snap {
            let ty = field_ty.get(f).map(decltype_rust).unwrap_or_else(|| "i32".to_string());
            out.push_str(&format!("    {}: {},\n", f, ty));
        }
        out.push_str("}\n\n");
    }

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
    if let Some(color) = background {
        out.push_str(&format!(
            "            frame.fill(&iced::widget::canvas::Path::rectangle(iced::Point::ORIGIN, bounds.size()), {});\n",
            color
        ));
    }
    let pixels = body_has_pixel(&cv.body);
    if pixels {
        out.push_str("            let pw = bounds.size().width.max(1.0).round() as u32;\n");
        out.push_str("            let ph = bounds.size().height.max(1.0).round() as u32;\n");
        out.push_str("            let mut pix = vec![0u8; pw as usize * ph as usize * 4];\n");
        out.push_str("            let mut pix_dirty = false;\n");
        out.push_str("            let width = pw as i32;\n");
        out.push_str("            let height = ph as i32;\n");
        out.push_str("            let _ = (width, height);\n");
    }
    // Draw is the same language as an Event: resolve first (coercions, `?` on
    // helpers), then rewrite state fields to `self.field`. Iced's `draw` is not
    // a `Result`, so the body runs in a catching closure — a failed helper
    // prints and the last picture stays up.
    let mut env_ty = field_ty.clone();
    if pixels {
        env_ty.insert("width".to_string(), DeclType::Plain(Type::Integer));
        env_ty.insert("height".to_string(), DeclType::Plain(Type::Integer));
    }
    let mut body = cv.body.clone();
    let passed_by_ref = crate::resolver::resolve_event_body(
        &mut body, &[], &env_ty, &t.fns, &t.methods, &t.consts, &t.modules, &t.interfaces,
        enums, &t.structs, diags,
    );
    elide_for_counter_dims(&mut body);
    let mut rewrite_fields = fields.clone();
    if pixels {
        // The injected canvas-size locals, not a State field of the same name.
        rewrite_fields.remove("width");
        rewrite_fields.remove("height");
    }
    let rewritten: Vec<Stmt> = body
        .into_iter()
        .map(|stmt| {
            let mut s = rewrite_canvas_stmt(stmt, &rewrite_fields, enums, paint_fns);
            coerce_state_strings(&mut s, "self", field_ty);
            s
        })
        .collect();
    let mut mutated = HashSet::new();
    collect_mutated(&rewritten, &mut mutated);
    mutated.extend(passed_by_ref);
    let empty_byref: HashSet<String> = HashSet::new();
    let emit_body = |diags: &mut Diagnostics, out: &mut String| {
        out.push_str("            let __vbr_draw: Result<(), String> = (|| {\n");
        for stmt in &rewritten {
            emit_stmt(stmt, &mutated, &empty_byref, 4, diags, out);
        }
        out.push_str("                Ok(())\n");
        out.push_str("            })();\n");
        out.push_str("            if let Err(__e) = __vbr_draw {\n");
        out.push_str("                eprintln!(\"Error: {}\", __e);\n");
        out.push_str("            }\n");
    };
    if pixels {
        crate::transpiler::with_pixel_flush(|| emit_body(diags, out));
        out.push_str(
            "            if pix_dirty { let __h = iced::widget::image::Handle::from_rgba(pw, ph, pix); frame.draw_image(iced::Rectangle::new(iced::Point::ORIGIN, bounds.size()), iced::widget::canvas::Image::new(__h).filter_method(iced::widget::image::FilterMethod::Nearest).snap(true)); }\n",
        );
    } else {
        emit_body(diags, out);
    }
    out.push_str("        }\n");
    out.push_str("        vec![frame.into_geometry()]\n");
    out.push_str("    }\n}\n");
}

/// Emit a paint function: an ordinary function that draws, so it gains a leading
/// `frame: &mut Frame` and its draw verbs / nested paint calls lower against it.
/// Same resolver as a Function; returns `Result` so Draw can `?` it.
fn emit_paint_fn(
    func: &Function,
    t: &surface::Tables,
    paint_fns: &HashSet<String>,
    diags: &mut Diagnostics,
    out: &mut String,
) {
    let name = rust_name(&func.name);
    let vis = if func.public { "pub " } else { "" };
    let mut params = vec!["frame: &mut iced::widget::canvas::Frame".to_string()];
    for p in &func.params {
        params.push(render_paint_param(p));
    }
    out.push_str(&format!("{}fn {}({}) -> Result<(), String> {{\n", vis, name, params.join(", ")));
    if body_has_pixel(&func.body) {
        diags.error_once(
            "pixel-in-paint",
            "`Set Pixel` belongs in the `Draw` block — it writes the picture buffer. \
             A paint function is for `Fill` / `Stroke` / `Text`. Compute a colour in a \
             normal Function and `Set Pixel` in Draw.",
        );
    }
    let mut body = func.body.clone();
    let passed_by_ref = crate::resolver::resolve_body(
        &mut body,
        &func.params,
        &t.fns,
        &t.methods,
        &t.consts,
        &t.modules,
        &t.interfaces,
        &t.enums,
        &t.structs,
        None,
        None,
        None,
        Some(crate::resolver::FailShape::Result),
        diags,
    );
    elide_for_counter_dims(&mut body);
    let empty_fields: HashSet<String> = HashSet::new();
    let rewritten: Vec<Stmt> = body
        .into_iter()
        .map(|stmt| rewrite_canvas_stmt(stmt, &empty_fields, &t.enums, paint_fns))
        .collect();
    let mut mutated = HashSet::new();
    collect_mutated(&rewritten, &mut mutated);
    mutated.extend(passed_by_ref);
    let byref: HashSet<String> = func
        .params
        .iter()
        .filter(|p| p.mode == ParamMode::ByRef)
        .map(|p| rust_name(&p.name))
        .collect();
    for stmt in &rewritten {
        emit_stmt(stmt, &mutated, &byref, 1, diags, out);
    }
    out.push_str("    Ok(())\n");
    out.push_str("}\n");
}

/// A paint-function parameter — numeric/bool by value, `&str` for text.
fn render_paint_param(p: &Param) -> String {
    let ty = match &p.ty {
        DeclType::Plain(Type::Text) => "&str".to_string(),
        DeclType::Plain(t) => t.rust().to_string(),
        other => decltype_rust(other),
    };
    format!("{}: {}", rust_name(&p.name), ty)
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
        ViewNode::Frame { children, spacing, padding, .. } => {
            // TUI-only; validate_view rejects it. Render children so the tree still walks.
            return render_container("column", children, ctx, indent, as_element, *spacing, *padding)
        }
        ViewNode::Tabs { tabs, .. } => {
            let children: Vec<ViewNode> = tabs.iter().flat_map(|p| p.children.clone()).collect();
            return render_container("column", &children, ctx, indent, as_element, None, None)
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
            let field = rust_name(value);
            let base = format!("text_input({}, &state.{})", ph, field);
            match on_input {
                Some(ev) => format!("{}.on_input(Message::{})", base, ev),
                None => base,
            }
        }
        ViewNode::Checkbox { label, value, on_toggle } => {
            let lbl = render_expr(&rewrite_expr(label.clone(), ctx.fields, ctx.enums), None);
            let field = rust_name(value);
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
            let field = rust_name(value);
            format!(
                "slider({}..={}, state.{}, Message::{})",
                lo, hi, field, on_change
            )
        }
        ViewNode::Toggler { label, value, on_toggle } => {
            let lbl = render_expr(&rewrite_expr(label.clone(), ctx.fields, ctx.enums), None);
            let field = rust_name(value);
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
            let field = rust_name(value);
            // Iced progress bars are `f32`; cast the bounds and value.
            format!(
                "progress_bar(({} as f32)..=({} as f32), state.{} as f32)",
                lo, hi, field
            )
        }
        ViewNode::Radio { label, value, option, on_select } => {
            let lbl = render_expr(&rewrite_expr(label.clone(), ctx.fields, ctx.enums), None);
            let opt = render_expr(&rewrite_expr(option.clone(), ctx.fields, ctx.enums), None);
            let field = rust_name(value);
            // The selected value is `Some(state.field)` (Copy, so it's a copy).
            format!(
                "radio({}, {}, Some(state.{}), Message::{})",
                lbl, opt, field, on_select
            )
        }
        ViewNode::TextArea { value } => {
            let field = rust_name(value);
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
            match &path.kind {
                ExprKind::Str(_) => format!("iced::widget::image({})", p),
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
        // Input/List/Table/Gauge/Sparkline/BarChart are Screen (TUI) widgets —
        // invalid in a Window; `validate_view` reports it, so this placeholder is
        // never compiled.
        ViewNode::Input { .. }
        | ViewNode::List { .. }
        | ViewNode::Table { .. }
        | ViewNode::Gauge { .. }
        | ViewNode::Sparkline { .. }
        | ViewNode::BarChart { .. }
        | ViewNode::Chart { .. }
        | ViewNode::Memo { .. } => {
            "iced::widget::Space::new(iced::Length::Shrink, iced::Length::Shrink)".to_string()
        }
        // Containers/conditionals/constrained returned early above.
        ViewNode::Column { .. } | ViewNode::Row { .. } | ViewNode::Frame { .. } | ViewNode::Tabs { .. }
        | ViewNode::Match { .. }
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
        s.push_str(&render_sized_child(c, ctx, indent + 1, kw));
        s.push_str(",\n");
    }
    s.push_str(&pad);
    s.push(']');
    s.push_str(&props);
    s.push_str(tail);
    s
}

/// A container child, honoring a `Length`/`Fill` size line: the child is wrapped
/// in an Iced `container` sized on the container's main axis (height in a Column,
/// width in a Row). An unsized child renders as-is.
fn render_sized_child(c: &ViewNode, ctx: &ViewCtx, indent: usize, kw: &str) -> String {
    if let ViewNode::Constrained { size, child } = c {
        let inner = render_view(child, ctx, indent, false);
        let axis = if kw == "column" { "height" } else { "width" };
        format!("iced::widget::container({}).{}({})", inner, axis, gui_length(*size))
    } else {
        render_view(c, ctx, indent, false)
    }
}

/// A layout size → the Iced `Length` it maps to. `Percent`/`Min` are terminal-only
/// (validated elsewhere); they fall back to a portion / fixed size here.
fn gui_length(size: SizeConstraint) -> String {
    match size {
        SizeConstraint::Length(n) => format!("iced::Length::Fixed({}.0)", n),
        SizeConstraint::Fill(1) => "iced::Length::Fill".to_string(),
        SizeConstraint::Fill(n) => format!("iced::Length::FillPortion({})", n),
        SizeConstraint::Percent(n) => format!("iced::Length::FillPortion({})", n),
        SizeConstraint::Min(n) => format!("iced::Length::Fixed({}.0)", n),
    }
}

/// A view `Match` → a typed `{ let el = match … {…}; el }` block, arms on lines.
fn render_view_match(scrutinee: &Expr, arms: &[ViewArm], ctx: &ViewCtx, indent: usize) -> String {
    let subj = match_scrutinee(scrutinee, "state", ctx.fields, ctx.field_ty, ctx.enums);
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

/// Walk the view for type mismatches we can explain better than rustc would.
/// Currently: an Iced `Slider`'s value must be convertible to `f64`, which rules
/// out `Long`/`LongLong` (i64) — point the user at `Integer`/`Single`/`Double`.
fn validate_view(node: &ViewNode, field_ty: &HashMap<String, DeclType>, diags: &mut Diagnostics) {
    match node {
        ViewNode::Constrained { size, child } => {
            if matches!(size, SizeConstraint::Percent(_) | SizeConstraint::Min(_)) {
                diags.error_once(
                    "gui-size",
                    "GUI layout sizing supports `Length N` (fixed pixels) and `Fill` — `Percent` \
                     and `Min` are Screen (TUI) only.",
                );
            }
            validate_view(child, field_ty, diags);
        }
        ViewNode::Input { .. }
        | ViewNode::List { .. }
        | ViewNode::Table { .. }
        | ViewNode::Gauge { .. }
        | ViewNode::Sparkline { .. }
        | ViewNode::BarChart { .. }
        | ViewNode::Chart { .. }
        | ViewNode::Memo { .. } => diags.error_once(
            "tui-widget-in-window",
            "That's a Screen (TUI) widget — it isn't available in a Window (GUI). In a GUI use \
             `TextInput` for text entry, `TextArea` for multi-line edit, and `ProgressBar`/`Canvas` \
             for charts.",
        ),
        ViewNode::Frame { children, .. } => {
            diags.error_once(
                "tui-widget-in-window",
                "A Frame is a Screen (TUI) panel — it isn't available in a Window. In a GUI use \
                 `Column`/`Row` (and a window title) instead.",
            );
            children.iter().for_each(|c| validate_view(c, field_ty, diags));
        }
        ViewNode::Tabs { tabs, .. } => {
            diags.error_once(
                "tui-widget-in-window",
                "Tabs is a Screen (TUI) widget — it isn't available in a Window.",
            );
            tabs.iter()
                .for_each(|p| p.children.iter().for_each(|c| validate_view(c, field_ty, diags)));
        }
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
            if matches!(field_ty.get(&rust_name(value)), Some(DeclType::Plain(Type::Long | Type::LongLong))) {
                diags.error_once(
                    &format!("slider-i64-{}", rust_name(value)),
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
            if !matches!(field_ty.get(&rust_name(value)), Some(DeclType::Plain(Type::Boolean))) {
                diags.error_once(
                    &format!("toggler-bool-{}", rust_name(value)),
                    format!("A Toggler binds to a `Boolean` state field — `{}` isn't one.", value),
                );
            }
        }
        ViewNode::ProgressBar { value, .. } => {
            let numeric = matches!(
                field_ty.get(&rust_name(value)),
                Some(DeclType::Plain(t)) if !matches!(t, Type::Text | Type::Boolean)
            );
            if !numeric {
                diags.error_once(
                    &format!("progress-num-{}", rust_name(value)),
                    format!("A ProgressBar shows a number — `{}` must be a numeric field.", value),
                );
            }
        }
        ViewNode::TextArea { value } => {
            if !matches!(field_ty.get(&rust_name(value)), Some(DeclType::Named(n)) if n == "TextArea") {
                diags.error_once(
                    &format!("textarea-type-{}", rust_name(value)),
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
                field_ty.get(&rust_name(value)),
                Some(DeclType::Named(_))
                    | Some(DeclType::Plain(
                        Type::Integer | Type::Long | Type::LongLong | Type::Byte
                    ))
            );
            if !ok {
                diags.error_once(
                    &format!("radio-type-{}", rust_name(value)),
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
                let f = rust_name(value);
                if !out.contains(&f) {
                    out.push(f);
                }
            }
            ViewNode::Column { children, .. } | ViewNode::Row { children, .. } => {
                children.iter().for_each(|c| walk(c, out));
            }
            ViewNode::Frame { children, .. } => {
                children.iter().for_each(|c| walk(c, out));
            }
            ViewNode::Tabs { tabs, .. } => {
                tabs.iter()
                    .for_each(|p| p.children.iter().for_each(|c| walk(c, out)));
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
        // TUI-only; rejected by validate_view.
        ViewNode::Input { .. }
        | ViewNode::List { .. }
        | ViewNode::Table { .. }
        | ViewNode::Gauge { .. }
        | ViewNode::Sparkline { .. }
        | ViewNode::BarChart { .. }
        | ViewNode::Chart { .. }
        | ViewNode::Memo { .. } => {}
        ViewNode::Column { children, .. } => {
            add(used, "column");
            children.iter().for_each(|c| collect_widgets(c, used));
        }
        ViewNode::Row { children, .. } => {
            add(used, "row");
            children.iter().for_each(|c| collect_widgets(c, used));
        }
        ViewNode::Frame { children, .. } => {
            children.iter().for_each(|c| collect_widgets(c, used));
        }
        ViewNode::Tabs { tabs, .. } => {
            tabs.iter()
                .for_each(|p| p.children.iter().for_each(|c| collect_widgets(c, used)));
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
    match &e.kind {
        ExprKind::Str(_) => format!("text({})", rendered),
        ExprKind::Binary { op: BinOp::Concat, .. } => format!("text({})", rendered),
        _ => format!("text(format!(\"{{}}\", {}))", rendered),
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
        .map(|f| rust_name(&f.name))
        .collect();
    loop {
        let mut changed = false;
        for f in &program.functions {
            if f.receiver.is_some() {
                continue;
            }
            let n = rust_name(&f.name);
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

fn body_has_pixel(stmts: &[Stmt]) -> bool {
    stmts.iter().any(stmt_has_pixel)
}

fn stmt_has_pixel(s: &Stmt) -> bool {
    match s {
        Stmt::Draw(DrawCmd::Pixel { .. }) => true,
        Stmt::If { branches, else_body } => {
            branches.iter().any(|(_, b)| body_has_pixel(b))
                || else_body.as_ref().map_or(false, |b| body_has_pixel(b))
        }
        Stmt::For { body, .. } | Stmt::ForEach { body, .. } | Stmt::DoLoop { body, .. } => {
            body_has_pixel(body)
        }
        Stmt::Match { arms, .. } => arms.iter().any(|a| body_has_pixel(&a.body)),
        _ => false,
    }
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
        Stmt::Expr(Expr { kind: ExprKind::Call { name, .. }, .. }) => set.contains(&rust_name(name)),
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
            ViewNode::Frame { children, .. } => {
                children.iter().for_each(|c| walk(c, out));
            }
            ViewNode::Tabs { tabs, .. } => {
                tabs.iter()
                    .for_each(|p| p.children.iter().for_each(|c| walk(c, out)));
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
        Stmt::Expr(e) => {
            // Resolver wraps fallible calls in `?`. A paint function is rewritten
            // to `Paint` (which already emits `?`); peel that wrapper so the call
            // still matches. Any other `?` stays — Draw's catching closure is the
            // `Result` context.
            let is_paint = {
                let call = match &e.kind {
                    ExprKind::Try(inner) => inner.as_ref(),
                    _ => &e,
                };
                matches!(&call.kind, ExprKind::Call { name, .. } if paint_fns.contains(&rust_name(name)))
            };
            if is_paint {
                let inner = match e.kind {
                    ExprKind::Try(inner) => *inner,
                    kind => Expr { kind, span: e.span },
                };
                let ExprKind::Call { name, args } = inner.kind else {
                    unreachable!("is_paint implies Call")
                };
                Stmt::Draw(DrawCmd::Paint {
                    name,
                    args: args.into_iter().map(re).collect(),
                })
            } else {
                Stmt::Expr(re(e))
            }
        }
        Stmt::Draw(cmd) => Stmt::Draw(rewrite_draw_cmd(cmd, fields, enums)),
        Stmt::Print(e) => Stmt::Print(re(e)),
        Stmt::Log(level, e) => Stmt::Log(level, re(e)),
        Stmt::Assign { target, value, op } => Stmt::Assign { target: re(target), value: re(value), op },
        Stmt::Dim { name, name_span, ty, init, deferred, line } => Stmt::Dim { name, name_span, ty, init: init.map(re), deferred, line },
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
        Stmt::Match { scrutinee, arms, line, if_let } => Stmt::Match {
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
            if_let,
        },
        Stmt::DoLoop { cond, body } => Stmt::DoLoop {
            cond: cond.map(|c| match c {
                DoCond::PreWhile(e) => DoCond::PreWhile(re(e)),
                DoCond::PreUntil(e) => DoCond::PreUntil(re(e)),
                DoCond::PostWhile(e) => DoCond::PostWhile(re(e)),
                DoCond::PostUntil(e) => DoCond::PostUntil(re(e)),
            }),
            body: body.into_iter().map(rec).collect(),
        },
        Stmt::Set { name, mutable, value } => Stmt::Set { name, mutable, value: re(value) },
        Stmt::DestructureDim { names, ty, value } => Stmt::DestructureDim { names, ty, value: re(value) },
        Stmt::Return(e) => Stmt::Return(e.map(re)),
        Stmt::RaiseError(e) => Stmt::RaiseError(re(e)),
        Stmt::HandleErr { target, call, err_name, body, line } => Stmt::HandleErr {
            target: target.map(re),
            call: re(call),
            err_name,
            body: body.into_iter().map(rec).collect(),
            line,
        },
        // Leaves — no `self`-field expression to rewrite. Listed explicitly (no
        // `_`) so a new statement carrying an expression must be handled here.
        leaf @ (Stmt::HandleDim { .. }
        | Stmt::Break
        | Stmt::Continue
        // `x = Nothing` names a plain local — no `self`-field to rewrite.
        | Stmt::Destroy { .. }
        // `Assert` only appears in a `Test` block, never a canvas `Draw` body.
        | Stmt::Assert(_)
        | Stmt::Comment(_)
        | Stmt::LineMark(_)) => leaf,
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
        DrawCmd::Pixel { x, y, color } => DrawCmd::Pixel { x: re(x), y: re(y), color: re(color) },
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
