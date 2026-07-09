//! Web codegen — a `Page` becomes a Yew 0.21 browser application, compiled to
//! WebAssembly and served with trunk (`vbr runweb`).
//!
//! The same Elm-style model as the GUI (`gui.rs`) and TUI (`tui.rs`) backends,
//! over the same shared core (`surface.rs`): State is the source of truth, the
//! View is derived from it, Events update it. A Yew *struct component* is that
//! model verbatim — a state struct, a `Message` enum, an `update` that mutates
//! `self` and returns "re-render", and a `view` built with the `html!` macro —
//! so the generated code reads like the Window/Screen output in a third dress.
//!
//! Slice 1: `Page` / `State` / `View` (`Text`, `Button`, `Column`, `Row`) /
//! `Event` → one component plus `fn main`. Events are synchronous (`Await`
//! arrives in a later slice) and the stdlib is not available in the browser yet.
//! Slice 2: the input round-trip — `TextInput` fires its event per keystroke
//! with the new text, `Checkbox` with its new state; payload events
//! (`Event Rename(value As String)`) become `Message` variants carrying data.
//! Slice 3: view logic and the remaining display widgets — `Match`/`If` in the
//! view (a Rust `match`/`if` choosing an `html!` fragment), `Slider`,
//! `ProgressBar`, `Image`, and `Length`/`Fill` sizing.
//! Slice 4: async — `Await Http.Get(url)` splits the event exactly as in the
//! GUI (kick-off + generated `<Event>Done` continuation), but runs on the
//! browser's own `fetch` (gloo-net) via `ctx.link().send_future` — no threads
//! on wasm, and no vbr_stdlib (ureq doesn't compile there).

use crate::ast::*;
use crate::diagnostics::Diagnostics;
use crate::surface::{
    self, analyze_events, collect_event_stdlib, launched, match_scrutinee, render_init,
    rewrite_expr_with, state_maps, AsyncBackend, AwaitSplit,
};
use crate::transpiler::{decltype_rust, render_expr, rust_name};
use std::collections::{HashMap, HashSet};

/// What the view renderer needs: the state-field names (to rewrite `count` →
/// `self.count`), their types (a `String` match scrutinee gets `.as_str()`, a
/// slider's number gets cast to its field's type), and the enum names
/// (`Size.Small` → `Size::Small`).
struct PageCtx<'a> {
    fields: &'a HashSet<String>,
    field_ty: &'a HashMap<String, DeclType>,
    enums: &'a HashSet<String>,
}

/// Emit a complete web program: shared items, each page's component, then
/// `fn main`, which mounts the page launched by `<Page>.Run` in `Function Main()`.
pub fn emit_web_program(program: &Program, diags: &mut Diagnostics) -> String {
    let mut out = String::new();
    let t = surface::build_tables(program);
    surface::emit_shared_items(program, &t, diags, &mut out, &mut |_, _, _| false);

    for p in &program.pages {
        out.push_str(&emit_page(p, &t, diags));
        out.push('\n');
    }

    // The browser sandbox has no filesystem, and vbr_stdlib doesn't compile on
    // wasm — so no stdlib in a Page, with one door: `Await Http.Get` in an
    // event runs on the browser's own fetch (it never marks `stdlib:Http`, so
    // it doesn't land here).
    let std = crate::transpiler::stdlib_used(diags);
    if !std.is_empty() {
        diags.error_once(
            "page-stdlib",
            format!(
                "The standard library ({}) isn't available in a Page — a browser sandbox has \
                 no filesystem, and its networking is async-only. For HTTP, use \
                 `Await Http.Get(url)` inside an event (it runs on the browser's fetch).",
                std.join(", ")
            ),
        );
    }

    // The fetch wrapper behind an awaited `Http.Get`, emitted once when used.
    if out.contains("http_get(") {
        out.push_str(surface::HTTP_GET_HELPER);
    }

    let launched_page = launched(program, |name| {
        program.pages.iter().find(|p| p.name.eq_ignore_ascii_case(name))
    });
    match launched_page {
        Some(p) => out.push_str(&emit_main(p)),
        None => diags.error_once(
            "web-no-launch",
            "A page is never launched. Add `Function Main()` containing `<Page>.Run`, \
             e.g. `Counter.Run`.",
        ),
    }
    out
}

/// `fn main` for a web app: mount the launched page's component onto the
/// document body. (The browser-tab title comes from `Title`, written into the
/// generated `index.html` by the project build.)
fn emit_main(p: &Window) -> String {
    format!(
        "fn main() {{\n    yew::Renderer::<{}>::new().render();\n}}\n",
        p.name
    )
}

/// Emit one page as a Yew struct component: the state struct, the `Message`
/// enum, and `impl Component` (create / update / view).
fn emit_page(p: &Window, t: &surface::Tables, diags: &mut Diagnostics) -> String {
    let mut out = String::new();
    let ty = &p.name; // the component struct is named after the page
    let (fields, field_ty) = state_maps(&p.state);
    let ctx = PageCtx { fields: &fields, field_ty: &field_ty, enums: &t.enums };

    validate_page(p, &field_ty, diags);
    // A fallible `State` initialiser needs a startup moment to fail cleanly in;
    // a browser component has none (and vbr_stdlib isn't on wasm anyway).
    if surface::state_fallible(&p.state, &t.fns) {
        diags.error_once(
            "page-fallible-init",
            "A fallible `State` initialiser (a call returning a Result, like \
             `Database.Open`) isn't available in a Page — give the field a plain \
             initial value.",
        );
    }
    // The same theme names as the GUI — `Theme "Dracula"` colors a Page like
    // it colors a Window (the palette becomes CSS in the generated index.html).
    if let Some(th) = &p.theme {
        if crate::gui::canonical_theme(th).is_none() {
            diags.error_once(
                "unknown-theme",
                format!(
                    "Unknown theme `{}`. Built-in themes: {}.",
                    th,
                    crate::gui::KNOWN_THEMES.join(", ")
                ),
            );
        }
    }

    // Analyse each event for `Await`: an async event splits into a kick-off arm
    // (sends a future to the component) and a generated `<Event>Done(...)`
    // continuation arm. Also checks nothing blocking runs un-`Await`ed.
    let splits: Vec<Option<AwaitSplit>> =
        analyze_events(&p.events, &field_ty, &t.fns, diags, AsyncBackend::Web);
    let any_async = splits.iter().any(Option::is_some);

    // Mark stdlib namespaces used in events so the program-wide fence catches
    // them — except `Http`, whose awaited form runs on the browser's fetch
    // (a sync `Http.Get` is caught by the blocking-without-`Await` check).
    let mut std_used: Vec<String> = Vec::new();
    for e in &p.events {
        collect_event_stdlib(&e.body, &mut std_used);
    }
    for ns in std_used.iter().filter(|ns| ns.as_str() != "Http") {
        diags.mark(&format!("stdlib:{}", ns));
    }

    out.push_str("use yew::prelude::*;\n");
    // `std` types used in event bodies (e.g. a HashMap built in an event).
    out.push_str(&surface::event_std_imports(&p.events));
    out.push('\n');

    // ── State struct: a Yew component holds its state directly ──
    out.push_str(&format!("struct {} {{\n", ty));
    for f in &p.state {
        out.push_str(&format!("    {}: {},\n", rust_name(&f.name), decltype_rust(&f.ty)));
    }
    out.push_str("}\n\n");

    // ── Message enum: one variant per event (payload params = its data), plus
    //    a `<Event>Done(result)` continuation variant for each async event ──
    if !p.events.is_empty() {
        out.push_str("enum Message {\n");
        for (e, split) in p.events.iter().zip(&splits) {
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
        out.push_str("}\n\n");
    }

    // ── The component: create (initial state), update (events), view ──
    out.push_str(&format!("impl Component for {} {{\n", ty));
    if p.events.is_empty() {
        out.push_str("    type Message = ();\n");
    } else {
        out.push_str("    type Message = Message;\n");
    }
    out.push_str("    type Properties = ();\n\n");

    // create — the Dim initialisers.
    out.push_str("    fn create(_ctx: &Context<Self>) -> Self {\n");
    out.push_str(&format!("        {} {{\n", ty));
    for f in &p.state {
        out.push_str(&format!(
            "            {}: {},\n",
            rust_name(&f.name),
            render_init(f.init.as_ref(), &f.ty, &t.enums)
        ));
    }
    out.push_str("        }\n");
    out.push_str("    }\n\n");

    // update — state-field idents are rewritten to `self.field`; returning
    // `true` tells Yew the state changed, so the view re-renders. An async
    // event's kick-off sends its future to the component with
    // `ctx.link().send_future` (the result comes back as `<Event>Done`), and
    // the `true` shows the pre-await state (e.g. "loading…") right away.
    if !p.events.is_empty() {
        let update_ctx = if any_async { "ctx" } else { "_ctx" };
        out.push_str(&format!(
            "    fn update(&mut self, {}: &Context<Self>, message: Self::Message) -> bool {{\n",
            update_ctx
        ));
        out.push_str("        match message {\n");
        for (e, split) in p.events.iter().zip(&splits) {
            if e.params.is_empty() {
                out.push_str(&format!("            Message::{} => {{\n", e.name));
            } else {
                let binds: Vec<String> = e.params.iter().map(|p| rust_name(&p.name)).collect();
                out.push_str(&format!(
                    "            Message::{}({}) => {{\n",
                    e.name,
                    binds.join(", ")
                ));
            }
            match split {
                // Synchronous event: run the whole body.
                None => surface::emit_event_stmts(
                    &e.body, &e.params, "self", &fields, &field_ty, t, 4, diags, &mut out,
                ),
                // Async kick-off: pre-await body, snapshot state, send the future.
                Some(s) => {
                    surface::emit_event_stmts(
                        &s.pre, &e.params, "self", &fields, &field_ty, t, 4, diags, &mut out,
                    );
                    for snap in &s.snapshots {
                        out.push_str(&format!("                {}\n", snap));
                    }
                    out.push_str(&format!(
                        "                ctx.link().send_future(async move {{ \
                         Message::{}Done({}.await) }});\n",
                        e.name, s.call_src
                    ));
                }
            }
            out.push_str("            }\n");
            // The continuation arm for an async event.
            if let Some(s) = split {
                out.push_str(&format!("            Message::{}Done({}) => {{\n", e.name, s.bind));
                surface::emit_event_stmts(
                    &s.cont, &e.params, "self", &fields, &field_ty, t, 4, diags, &mut out,
                );
                out.push_str("            }\n");
            }
        }
        out.push_str("        }\n");
        out.push_str("        true // state changed — re-render the view\n");
        out.push_str("    }\n\n");
    }

    // view — the html! body decides whether `ctx` is needed (button callbacks).
    // The root container carries the page's own name as a class alongside its
    // vbr-* kind, so CSS can say ".counter .vbr-button" as well as ".vbr-button".
    let root_class = rust_name(&p.name);
    let mut body = String::new();
    match &p.view {
        ViewNode::Column { children, spacing, padding } => render_flex(
            "column", children, *spacing, *padding, Some(&root_class), &ctx, 3, &mut body, diags,
        ),
        ViewNode::Row { children, spacing, padding } => render_flex(
            "row", children, *spacing, *padding, Some(&root_class), &ctx, 3, &mut body, diags,
        ),
        other => render_node(other, &ctx, "column", 3, &mut body, diags),
    }
    let ctx_param = if body.contains("ctx.link()") { "ctx" } else { "_ctx" };
    out.push_str(&format!(
        "    fn view(&self, {}: &Context<Self>) -> Html {{\n",
        ctx_param
    ));
    out.push_str("        html! {\n");
    out.push_str(&body);
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    out
}

/// Page binding checks, each a teaching error: input widgets must bind fields
/// of the right type (a `TextInput` types into a `String`, a `Checkbox`
/// toggles a `Boolean`).
fn validate_page(p: &Window, field_ty: &HashMap<String, DeclType>, diags: &mut Diagnostics) {
    validate_view(&p.view, field_ty, diags);
}

fn validate_view(node: &ViewNode, field_ty: &HashMap<String, DeclType>, diags: &mut Diagnostics) {
    match node {
        ViewNode::TextInput { value, .. } => {
            if !matches!(field_ty.get(&rust_name(value)), Some(DeclType::Plain(Type::Text))) {
                diags.error_once(
                    &format!("textinput-field-{}", rust_name(value)),
                    format!("A TextInput binds to a `String` state field — `{}` isn't one.", value),
                );
            }
        }
        ViewNode::Checkbox { value, .. } => {
            if !matches!(field_ty.get(&rust_name(value)), Some(DeclType::Plain(Type::Boolean))) {
                diags.error_once(
                    &format!("checkbox-field-{}", rust_name(value)),
                    format!("A Checkbox binds to a `Boolean` state field — `{}` isn't one.", value),
                );
            }
        }
        ViewNode::Slider { value, .. } => {
            let numeric = matches!(
                field_ty.get(&rust_name(value)),
                Some(DeclType::Plain(t)) if !matches!(t, Type::Text | Type::Boolean)
            );
            if !numeric {
                diags.error_once(
                    &format!("slider-field-{}", rust_name(value)),
                    format!("A Slider binds to a numeric state field — `{}` isn't one.", value),
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
                    &format!("progress-field-{}", rust_name(value)),
                    format!("A ProgressBar shows a number — `{}` must be a numeric field.", value),
                );
            }
        }
        ViewNode::Constrained { child, .. } => validate_view(child, field_ty, diags),
        ViewNode::Column { children, .. } | ViewNode::Row { children, .. } => {
            children.iter().for_each(|c| validate_view(c, field_ty, diags));
        }
        ViewNode::Match { arms, .. } => {
            for a in arms {
                a.body.iter().for_each(|c| validate_view(c, field_ty, diags));
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
        _ => {}
    }
}

/// Render a view node as `html!` markup into `out`, one element per line,
/// indented. Containers become flexbox `<div>`s; leaves become HTML elements.
/// `axis` is the enclosing container's direction, so a `Length` constraint
/// knows whether it sizes height (in a Column) or width (in a Row).
fn render_node(
    node: &ViewNode,
    ctx: &PageCtx,
    axis: &str,
    indent: usize,
    out: &mut String,
    diags: &mut Diagnostics,
) {
    let pad = "    ".repeat(indent);
    match node {
        ViewNode::Column { children, spacing, padding } => {
            render_flex("column", children, *spacing, *padding, None, ctx, indent, out, diags);
        }
        ViewNode::Row { children, spacing, padding } => {
            render_flex("row", children, *spacing, *padding, None, ctx, indent, out, diags);
        }
        ViewNode::Text(e) => {
            out.push_str(&format!("{}<p class=\"vbr-text\">{{ {} }}</p>\n", pad, text_content(e, ctx)));
        }
        ViewNode::Button { label, on_click } => {
            let lbl = text_content(label, ctx);
            match on_click {
                Some(ev) => out.push_str(&format!(
                    "{}<button class=\"vbr-button\" onclick={{ctx.link().callback(|_| Message::{})}}>{{ {} }}</button>\n",
                    pad, ev, lbl
                )),
                None => out.push_str(&format!(
                    "{}<button class=\"vbr-button\">{{ {} }}</button>\n",
                    pad, lbl
                )),
            }
        }
        // A controlled text input: the value always comes from state, and each
        // keystroke sends the input's new text to the bound event.
        ViewNode::TextInput { placeholder, value, on_input } => {
            let ph = render_expr(
                &rewrite_expr_with(placeholder.clone(), "self", ctx.fields, ctx.enums),
                None,
            );
            let field = rust_name(value);
            let inner = "    ".repeat(indent + 1);
            out.push_str(&format!("{}<input\n", pad));
            out.push_str(&format!("{}class=\"vbr-textinput\"\n", inner));
            out.push_str(&format!("{}placeholder={{{}}}\n", inner, ph));
            out.push_str(&format!("{}value={{self.{}.clone()}}\n", inner, field));
            if let Some(ev) = on_input {
                out.push_str(&format!(
                    "{}oninput={{ctx.link().callback(|e: InputEvent| \
                     Message::{}(e.target_unchecked_into::<web_sys::HtmlInputElement>().value()))}}\n",
                    inner, ev
                ));
            }
            out.push_str(&format!("{}/>\n", pad));
        }
        // A checkbox inside its <label>, so clicking the text toggles it too.
        ViewNode::Checkbox { label, value, on_toggle } => {
            let lbl = text_content(label, ctx);
            let field = rust_name(value);
            let inner = "    ".repeat(indent + 1);
            let in2 = "    ".repeat(indent + 2);
            out.push_str(&format!("{}<label class=\"vbr-checkbox\">\n", pad));
            out.push_str(&format!("{}<input\n", inner));
            out.push_str(&format!("{}type=\"checkbox\"\n", in2));
            out.push_str(&format!("{}checked={{self.{}}}\n", in2, field));
            if let Some(ev) = on_toggle {
                out.push_str(&format!(
                    "{}onchange={{ctx.link().callback(|e: Event| \
                     Message::{}(e.target_unchecked_into::<web_sys::HtmlInputElement>().checked()))}}\n",
                    in2, ev
                ));
            }
            out.push_str(&format!("{}/>\n", inner));
            out.push_str(&format!("{}{{ {} }}\n", inner, lbl));
            out.push_str(&format!("{}</label>\n", pad));
        }
        // A numeric slider over min..=max — each drag sends the new value,
        // cast to the bound field's type (the DOM reports it as a float).
        ViewNode::Slider { min, max, value, on_change } => {
            let field = rust_name(value);
            let cast = match ctx.field_ty.get(&field) {
                Some(DeclType::Plain(t)) => t.rust(),
                _ => "i64",
            };
            let inner = "    ".repeat(indent + 1);
            out.push_str(&format!("{}<input\n", pad));
            out.push_str(&format!("{}class=\"vbr-slider\"\n", inner));
            out.push_str(&format!("{}type=\"range\"\n", inner));
            out.push_str(&format!("{}min={}\n", inner, attr_value(min, ctx)));
            out.push_str(&format!("{}max={}\n", inner, attr_value(max, ctx)));
            out.push_str(&format!("{}value={{self.{}.to_string()}}\n", inner, field));
            out.push_str(&format!(
                "{}oninput={{ctx.link().callback(|e: InputEvent| Message::{}(\
                 e.target_unchecked_into::<web_sys::HtmlInputElement>().value_as_number() as {}))}}\n",
                inner, on_change, cast
            ));
            out.push_str(&format!("{}/>\n", pad));
        }
        // A read-only progress bar. HTML's <progress> always starts at 0, so a
        // non-zero `min` shifts both the value and the max.
        ViewNode::ProgressBar { min, max, value } => {
            let field = rust_name(value);
            match (min, max) {
                (Expr::Int(0), Expr::Int(hi)) => {
                    out.push_str(&format!(
                        "{}<progress class=\"vbr-progressbar\" max=\"{}\" value={{self.{}.to_string()}}></progress>\n",
                        pad, hi, field
                    ));
                }
                _ => {
                    let lo = render_rewritten(min, ctx);
                    let hi = render_rewritten(max, ctx);
                    out.push_str(&format!(
                        "{}<progress class=\"vbr-progressbar\" max={{(({}) as f64 - ({}) as f64).to_string()}} \
                         value={{(self.{} as f64 - ({}) as f64).to_string()}}></progress>\n",
                        pad, hi, lo, field, lo
                    ));
                }
            }
        }
        // An image. The src is a URL: absolute (https://…) always works; a bare
        // file name resolves against the served site (the asset story is a
        // later slice). A String state field as the path is cloned to own it.
        ViewNode::Image { path } => {
            let src = match path {
                Expr::Str(_) => render_expr(path, None),
                _ => format!("{{{}.clone()}}", render_rewritten(path, ctx)),
            };
            out.push_str(&format!("{}<img class=\"vbr-image\" src={} />\n", pad, src));
        }
        // `Match <expr>` in the view → a Rust `match` choosing an html! fragment.
        ViewNode::Match { scrutinee, arms } => {
            let subj = match_scrutinee(scrutinee, "self", ctx.fields, ctx.field_ty, ctx.enums);
            let in1 = "    ".repeat(indent + 1);
            let in2 = "    ".repeat(indent + 2);
            out.push_str(&format!("{}{{\n", pad));
            out.push_str(&format!("{}match {} {{\n", in1, subj));
            for arm in arms {
                let guard = match &arm.guard {
                    Some(g) => format!(" if {}", render_rewritten(g, ctx)),
                    None => String::new(),
                };
                out.push_str(&format!("{}{}{} => html! {{\n", in2, arm.pattern, guard));
                render_body(&arm.body, ctx, indent + 3, out, diags);
                out.push_str(&format!("{}}},\n", in2));
            }
            out.push_str(&format!("{}}}\n", in1));
            out.push_str(&format!("{}}}\n", pad));
        }
        // `If <cond> Then … [ElseIf …] [Else …]` → a Rust `if` choosing an html!
        // fragment; with no `Else`, the false case renders nothing (`html! {}`).
        ViewNode::If { branches, else_body } => {
            let in1 = "    ".repeat(indent + 1);
            let in2 = "    ".repeat(indent + 2);
            out.push_str(&format!("{}{{\n", pad));
            for (i, (cond, body)) in branches.iter().enumerate() {
                let kw = if i == 0 { "if" } else { "} else if" };
                out.push_str(&format!("{}{} {} {{\n", in1, kw, render_rewritten(cond, ctx)));
                out.push_str(&format!("{}html! {{\n", in2));
                render_body(body, ctx, indent + 3, out, diags);
                out.push_str(&format!("{}}}\n", in2));
            }
            out.push_str(&format!("{}}} else {{\n", in1));
            match else_body {
                Some(body) => {
                    out.push_str(&format!("{}html! {{\n", in2));
                    render_body(body, ctx, indent + 3, out, diags);
                    out.push_str(&format!("{}}}\n", in2));
                }
                None => out.push_str(&format!("{}html! {{}}\n", in2)),
            }
            out.push_str(&format!("{}}}\n", in1));
            out.push_str(&format!("{}}}\n", pad));
        }
        // A sized child: `Length N` fixes the container axis in pixels; `Fill`
        // takes a share of the leftover space (CSS flex).
        ViewNode::Constrained { size, child } => {
            let style = match size {
                SizeConstraint::Length(n) => {
                    let dim = if axis == "row" { "width" } else { "height" };
                    format!("{}: {}px;", dim, n)
                }
                SizeConstraint::Fill(w) => format!("flex: {};", w),
                SizeConstraint::Percent(_) | SizeConstraint::Min(_) => {
                    diags.error_once(
                        "page-size",
                        "Page layout sizing supports `Length N` (pixels) and `Fill` — \
                         `Percent` and `Min` are Screen (TUI) only.",
                    );
                    render_node(child, ctx, axis, indent, out, diags);
                    return;
                }
            };
            out.push_str(&format!("{}<div style=\"{}\">\n", pad, style));
            render_node(child, ctx, axis, indent + 1, out, diags);
            out.push_str(&format!("{}</div>\n", pad));
        }
        other => {
            diags.error_once(
                "page-widget",
                format!(
                    "That widget isn't supported in a Page yet ({}). So far a Page supports \
                     Column, Row, Text, Button, TextInput, Checkbox, Slider, ProgressBar, \
                     Image, Match, and If.",
                    web_node_name(other)
                ),
            );
        }
    }
}

/// A `Column`/`Row` → a flexbox `<div>`, children one per line inside.
/// `extra` adds a second CSS class — the page's own name on its root
/// container, so a stylesheet can target one page's controls.
#[allow(clippy::too_many_arguments)]
fn render_flex(
    direction: &str,
    children: &[ViewNode],
    spacing: Option<u16>,
    padding: Option<u16>,
    extra: Option<&str>,
    ctx: &PageCtx,
    indent: usize,
    out: &mut String,
    diags: &mut Diagnostics,
) {
    let pad = "    ".repeat(indent);
    let class = match extra {
        Some(e) => format!("vbr-{} {}", direction, e),
        None => format!("vbr-{}", direction),
    };
    let mut style = format!("display: flex; flex-direction: {};", direction);
    if let Some(s) = spacing {
        style.push_str(&format!(" gap: {}px;", s));
    }
    if let Some(p) = padding {
        style.push_str(&format!(" padding: {}px;", p));
    }
    out.push_str(&format!("{}<div class=\"{}\" style=\"{}\">\n", pad, class, style));
    for c in children {
        render_node(c, ctx, direction, indent + 1, out, diags);
    }
    out.push_str(&format!("{}</div>\n", pad));
}

/// An arm/branch body → its markup: one widget renders as-is; several stack
/// vertically (an implicit Column).
fn render_body(
    body: &[ViewNode],
    ctx: &PageCtx,
    indent: usize,
    out: &mut String,
    diags: &mut Diagnostics,
) {
    match body {
        [] => {}
        [one] => render_node(one, ctx, "column", indent, out, diags),
        many => {
            let col = ViewNode::Column { children: many.to_vec(), spacing: None, padding: None };
            render_node(&col, ctx, "column", indent, out, diags);
        }
    }
}

/// An expression rewritten for the component (`count` → `self.count`,
/// `Size.Small` → `Size::Small`) and rendered to Rust.
fn render_rewritten(e: &Expr, ctx: &PageCtx) -> String {
    render_expr(&rewrite_expr_with(e.clone(), "self", ctx.fields, ctx.enums), None)
}

/// An HTML attribute value: a literal number as a plain quoted attribute
/// (`min="0"`), anything else as a braced expression stringified.
fn attr_value(e: &Expr, ctx: &PageCtx) -> String {
    match e {
        Expr::Int(n) => format!("\"{}\"", n),
        _ => format!("{{({}).to_string()}}", render_rewritten(e, ctx)),
    }
}

/// Text/label content inside `{ … }`: a string literal as-is, a concatenation
/// as its `format!`, anything else stringified. State fields become `self.field`.
fn text_content(e: &Expr, ctx: &PageCtx) -> String {
    let rewritten = rewrite_expr_with(e.clone(), "self", ctx.fields, ctx.enums);
    match e {
        Expr::Str(_) => render_expr(&rewritten, None),
        Expr::Binary { op: BinOp::Concat, .. } => render_expr(&rewritten, None),
        _ => format!("format!(\"{{}}\", {})", render_expr(&rewritten, None)),
    }
}

/// A short name for an unsupported node, for the diagnostic.
fn web_node_name(node: &ViewNode) -> &'static str {
    match node {
        ViewNode::TextArea { .. } => "TextArea",
        ViewNode::Toggler { .. } => "Toggler",
        ViewNode::Radio { .. } => "Radio",
        ViewNode::Canvas { .. } => "Canvas",
        ViewNode::Space { .. } => "Space",
        ViewNode::Input { .. } => "Input",
        ViewNode::List { .. } => "List",
        ViewNode::Table { .. } => "Table",
        ViewNode::Gauge { .. } => "Gauge",
        ViewNode::Sparkline { .. } => "Sparkline",
        ViewNode::BarChart { .. } => "BarChart",
        ViewNode::Chart { .. } => "Chart",
        _ => "widget",
    }
}

/// The stylesheet for a web program's `index.html`: the launched page's
/// `Theme` (the same palette as the Iced theme of that name, as CSS custom
/// properties plus base rules), followed by any `Css … End Css` blocks,
/// verbatim. `None` when the program styles nothing (the index stays bare).
pub fn page_style(program: &Program) -> Option<String> {
    if program.pages.is_empty() {
        return None;
    }
    let page = launched(program, |name| {
        program.pages.iter().find(|p| p.name.eq_ignore_ascii_case(name))
    })
    .or_else(|| program.pages.first());
    let mut css = String::new();
    if let Some(th) = page.and_then(|p| p.theme.as_ref()) {
        if let Some(t) = theme_css(th) {
            css.push_str(&t);
        }
    }
    for block in &program.css {
        if !css.is_empty() {
            css.push('\n');
        }
        css.push_str(block.trim_matches('\n'));
        css.push('\n');
    }
    (!css.is_empty()).then_some(css)
}

/// Local files the pages' views reference (`Image "logo.png"`) — trunk copies
/// each into the served site. An absolute URL isn't an asset, and a computed
/// path can't be scanned (the spec says to keep asset files next to the .vbr).
pub fn page_assets(program: &Program) -> Vec<String> {
    fn walk(node: &ViewNode, out: &mut Vec<String>) {
        match node {
            ViewNode::Image { path: Expr::Str(s) }
                if !s.starts_with("http://") && !s.starts_with("https://") =>
            {
                out.push(s.clone());
            }
            ViewNode::Column { children, .. } | ViewNode::Row { children, .. } => {
                children.iter().for_each(|c| walk(c, out));
            }
            ViewNode::Constrained { child, .. } => walk(child, out),
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
    let mut assets = Vec::new();
    for p in &program.pages {
        walk(&p.view, &mut assets);
    }
    assets.sort();
    assets.dedup();
    assets
}

/// A built-in theme as CSS: the palette (matching Iced 0.13's of the same
/// name) as custom properties, plus base rules for the page body and the
/// generated `vbr-*` controls. A `Css` block can override any of it — it comes
/// later in the stylesheet.
fn theme_css(name: &str) -> Option<String> {
    let (bg, text, primary) = theme_palette(crate::gui::canonical_theme(name)?)?;
    Some(format!(
        ":root {{\n  --vbr-background: {bg};\n  --vbr-text: {text};\n  --vbr-primary: {primary};\n}}\n\
         body {{\n  margin: 0;\n  min-height: 100vh;\n  background: var(--vbr-background);\n  \
         color: var(--vbr-text);\n  font-family: system-ui, sans-serif;\n}}\n\
         .vbr-button {{\n  width: fit-content;\n  padding: 6px 14px;\n  border: none;\n  \
         border-radius: 4px;\n  background: var(--vbr-primary);\n  color: var(--vbr-background);\n  \
         cursor: pointer;\n}}\n\
         .vbr-textinput {{\n  padding: 6px 8px;\n  border: 1px solid var(--vbr-primary);\n  \
         border-radius: 4px;\n  background: transparent;\n  color: var(--vbr-text);\n}}\n\
         .vbr-checkbox, .vbr-slider, .vbr-progressbar {{\n  accent-color: var(--vbr-primary);\n}}\n"
    ))
}

/// (background, text, primary) for each built-in theme — the hex values of
/// Iced 0.13's palettes (core/src/theme/palette.rs), so the browser Dracula
/// is the desktop Dracula.
fn theme_palette(canon: &str) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match canon {
        "Light" => ("#ffffff", "#000000", "#5e7ce2"),
        "Dark" => ("#202225", "#e6e6e6", "#5e7ce2"),
        "Dracula" => ("#282a36", "#f8f8f2", "#bd93f9"),
        "Nord" => ("#2e3440", "#eceff4", "#8fbcbb"),
        "SolarizedLight" => ("#fdf6e3", "#657b83", "#2aa198"),
        "SolarizedDark" => ("#002b36", "#839496", "#2aa198"),
        "GruvboxLight" => ("#fbf1c7", "#282828", "#458588"),
        "GruvboxDark" => ("#282828", "#fbf1c7", "#458588"),
        "CatppuccinLatte" => ("#eff1f5", "#4c4f69", "#1e66f5"),
        "CatppuccinFrappe" => ("#303446", "#c6d0f5", "#8caaee"),
        "CatppuccinMacchiato" => ("#24273a", "#cad3f5", "#8aadf4"),
        "CatppuccinMocha" => ("#1e1e2e", "#cdd6f4", "#89b4fa"),
        "TokyoNight" => ("#1a1b26", "#9aa5ce", "#2ac3de"),
        "TokyoNightStorm" => ("#24283b", "#9aa5ce", "#2ac3de"),
        "TokyoNightLight" => ("#d5d6db", "#565a6e", "#166775"),
        "KanagawaWave" => ("#363646", "#dcd7ba", "#2d4f67"),
        "KanagawaDragon" => ("#181616", "#c5c9c5", "#223249"),
        "KanagawaLotus" => ("#f2ecbc", "#545464", "#c9cbd1"),
        "Moonfly" => ("#080808", "#bdbdbd", "#80a0ff"),
        "Nightfly" => ("#011627", "#bdc1c6", "#82aaff"),
        "Oxocarbon" => ("#232323", "#d0d0d0", "#00b4ff"),
        "Ferra" => ("#2b292d", "#fecdb2", "#d1d1e0"),
        _ => return None,
    })
}
