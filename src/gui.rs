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
    // Free functions, except `Main`.
    for f in program.functions.iter().filter(|f| f.receiver.is_none() && !is_main(f)) {
        emit_fn(f, &fns, &methods, &consts, &modules, &enums, diags, &mut out, 0, None);
        out.push('\n');
    }

    for w in &program.windows {
        out.push_str(&emit_window(w, &enums, diags));
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
fn emit_window(w: &Window, enums: &HashSet<String>, diags: &mut Diagnostics) -> String {
    let mut out = String::new();
    let ty = &w.name; // the state struct is named after the window
    let fields: HashSet<String> = w.state.iter().map(|f| to_snake(&f.name)).collect();
    let field_ty: HashMap<String, DeclType> =
        w.state.iter().map(|f| (to_snake(&f.name), f.ty.clone())).collect();
    let ctx = ViewCtx { fields: &fields, field_ty: &field_ty, enums };
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
        .map(|e| await_split(e, &field_ty, diags))
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
            format!(
                "iced::widget::text_editor::Content::with_text({})",
                render_expr(&f.init, None)
            )
        } else {
            render_init(&f.init, &f.ty, enums)
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
    out.push_str(&format!("fn update(state: &mut {}, message: Message){} {{\n", ty, ret));
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

    out
}

/// A `State` field initialiser: a `String` becomes owned, numbers adapt to type,
/// an enum variant (`Size.Small`) resolves to its path (`Size::Small`).
fn render_init(init: &Expr, ty: &DeclType, enums: &HashSet<String>) -> String {
    let empty = HashSet::new();
    match ty {
        DeclType::Plain(Type::Text) => format!("{}.to_string()", render_expr(init, None)),
        DeclType::Plain(t) => render_expr(init, Some(*t)),
        // Enum (or other named) init — rewrite `Size.Small` → `Size::Small`.
        _ => render_expr(&rewrite_expr(init.clone(), &empty, enums), None),
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
        // Containers/conditionals returned early above.
        ViewNode::Column { .. } | ViewNode::Row { .. } | ViewNode::Match { .. } | ViewNode::If { .. } => {
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
        ViewNode::Column { children, .. } => {
            add(used, "column");
            children.iter().for_each(|c| collect_widgets(c, used));
        }
        ViewNode::Row { children, .. } => {
            add(used, "row");
            children.iter().for_each(|c| collect_widgets(c, used));
        }
        // `Space` uses a fully-qualified path, so no import needed.
        ViewNode::Space { .. } => {}
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
fn coerce_state_strings(s: &mut Stmt, field_ty: &HashMap<String, DeclType>) {
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
    diags: &mut Diagnostics,
) -> Option<AwaitSplit> {
    let idx = e.body.iter().position(stmt_has_await)?;
    match &e.body[idx] {
        Stmt::Match { scrutinee: Expr::Await(call), arms, line } => {
            let info = awaitable_info(call, field_ty, diags)?;
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
            let info = awaitable_info(call, field_ty, diags)?;
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

/// Resolve an awaited stdlib call to its Rust form, result type, and how to run
/// it (V1: a known stdlib call such as `Http.Get`).
fn awaitable_info(
    call: &Expr,
    field_ty: &HashMap<String, DeclType>,
    diags: &mut Diagnostics,
) -> Option<AwaitInfo> {
    let (recv, method, args) = match call {
        Expr::MethodCall { recv, method, args } => (recv, method, args),
        _ => {
            diags.error_once(
                "await-not-call",
                "`Await` currently works only on a stdlib call like `Http.Get(url)`.",
            );
            return None;
        }
    };
    let canon = match &**recv {
        Expr::Ident(r) => stdlib_type(r),
        _ => None,
    };
    let canon = match canon {
        Some(c) => c,
        None => {
            diags.error_once(
                "await-not-stdlib",
                "`Await` currently works only on a stdlib call like `Http.Get(url)`.",
            );
            return None;
        }
    };
    let m = to_snake(method);
    let (ret_type, blocking) = match (canon, m.as_str()) {
        ("Http", "get") => ("Result<String, String>", true),
        _ => {
            diags.error_once(
                "await-unsupported",
                format!(
                    "`Await {}.{}` isn't supported yet — V1 awaits stdlib calls like `Http.Get`.",
                    canon, method
                ),
            );
            return None;
        }
    };
    diags.mark(&format!("stdlib:{}", canon));
    // The async task can't borrow `state`, so snapshot (clone) any state fields it
    // uses, and render the call against those owned locals.
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
    let call_src = format!("{}::{}({})", canon, m, arg_src.join(", "));
    Some(AwaitInfo { snapshots, call_src, ret_type: ret_type.to_string(), blocking })
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
fn rewrite_expr(e: Expr, fields: &HashSet<String>, enums: &HashSet<String>) -> Expr {
    match e {
        // `Color.Red` (field on an enum name) → the path `Color::Red`.
        Expr::Field(inner, variant) if matches!(&*inner, Expr::Ident(n) if enums.contains(n)) => {
            match *inner {
                Expr::Ident(n) => Expr::ConstRef(format!("{}::{}", n, variant)),
                _ => unreachable!(),
            }
        }
        Expr::Ident(name) if fields.contains(&to_snake(&name)) => {
            Expr::Field(Box::new(Expr::Ident("state".to_string())), name)
        }
        Expr::Binary { op, lhs, rhs } => Expr::Binary {
            op,
            lhs: Box::new(rewrite_expr(*lhs, fields, enums)),
            rhs: Box::new(rewrite_expr(*rhs, fields, enums)),
        },
        Expr::Not(inner) => Expr::Not(Box::new(rewrite_expr(*inner, fields, enums))),
        Expr::Call { name, args } => Expr::Call {
            name,
            args: args.into_iter().map(|a| rewrite_expr(a, fields, enums)).collect(),
        },
        Expr::MethodCall { recv, method, args } => Expr::MethodCall {
            recv: Box::new(rewrite_expr(*recv, fields, enums)),
            method,
            args: args.into_iter().map(|a| rewrite_expr(a, fields, enums)).collect(),
        },
        Expr::Field(inner, f) => Expr::Field(Box::new(rewrite_expr(*inner, fields, enums)), f),
        Expr::Index(a, b) => Expr::Index(
            Box::new(rewrite_expr(*a, fields, enums)),
            Box::new(rewrite_expr(*b, fields, enums)),
        ),
        Expr::Cast(inner, t) => Expr::Cast(Box::new(rewrite_expr(*inner, fields, enums)), t),
        other => other,
    }
}

fn rewrite_stmt(s: Stmt, fields: &HashSet<String>, enums: &HashSet<String>) -> Stmt {
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
