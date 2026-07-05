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

use crate::ast::*;
use crate::diagnostics::Diagnostics;
use crate::surface::{
    self, event_stdlib_imports, launched, render_init, rewrite_expr_with, state_maps,
    stmt_has_await,
};
use crate::transpiler::{decltype_rust, render_expr, rust_name};
use std::collections::HashSet;

/// What the view renderer needs: the state-field names (to rewrite `count` →
/// `self.count`) and the enum names (`Size.Small` → `Size::Small`).
struct PageCtx<'a> {
    fields: &'a HashSet<String>,
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

    // The browser sandbox has no filesystem, and our stdlib's blocking calls
    // can't run on the UI thread — so no stdlib in a Page yet (a web-friendly
    // `Http` over the browser's fetch arrives with `Await` in a later slice).
    let std = crate::transpiler::stdlib_used(diags);
    if !std.is_empty() {
        diags.error_once(
            "page-stdlib",
            format!(
                "The standard library ({}) isn't available in a Page yet — a browser has no \
                 filesystem, and blocking calls would freeze the page. A web-friendly `Http` \
                 arrives in a later slice.",
                std.join(", ")
            ),
        );
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
    let ctx = PageCtx { fields: &fields, enums: &t.enums };

    validate_page(p, diags);
    // Trigger the stdlib marks for event bodies (checked program-wide by the caller).
    let _ = event_stdlib_imports(&p.events, diags);

    out.push_str("use yew::prelude::*;\n\n");

    // ── State struct: a Yew component holds its state directly ──
    out.push_str(&format!("struct {} {{\n", ty));
    for f in &p.state {
        out.push_str(&format!("    {}: {},\n", rust_name(&f.name), decltype_rust(&f.ty)));
    }
    out.push_str("}\n\n");

    // ── Message enum: one variant per event ──
    if !p.events.is_empty() {
        out.push_str("enum Message {\n");
        for e in &p.events {
            out.push_str(&format!("    {},\n", e.name));
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
    // `true` tells Yew the state changed, so the view re-renders.
    if !p.events.is_empty() {
        out.push_str(
            "    fn update(&mut self, _ctx: &Context<Self>, message: Self::Message) -> bool {\n",
        );
        out.push_str("        match message {\n");
        for e in &p.events {
            out.push_str(&format!("            Message::{} => {{\n", e.name));
            surface::emit_event_stmts(
                &e.body, &e.params, "self", &fields, &field_ty, t, 4, diags, &mut out,
            );
            out.push_str("            }\n");
        }
        out.push_str("        }\n");
        out.push_str("        true // state changed — re-render the view\n");
        out.push_str("    }\n\n");
    }

    // view — the html! body decides whether `ctx` is needed (button callbacks).
    let mut body = String::new();
    render_node(&p.view, &ctx, 3, &mut body, diags);
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

/// Slice-1 fences, each a teaching error: no `Await` yet (events are
/// synchronous), no event parameters yet (they arrive with input controls).
fn validate_page(p: &Window, diags: &mut Diagnostics) {
    if p.events.iter().any(|e| e.body.iter().any(stmt_has_await)) {
        diags.error_once(
            "page-await",
            "`Await` isn't available in a Page yet — Page events are synchronous for now \
             (browser async arrives in a later slice).",
        );
    }
    if p.events.iter().any(|e| !e.params.is_empty()) {
        diags.error_once(
            "page-event-params",
            "A Page event can't take parameters yet — they arrive with the input controls \
             (`TextInput`, `Checkbox`) in a later slice.",
        );
    }
}

/// Render a view node as `html!` markup into `out`, one element per line,
/// indented. Containers become flexbox `<div>`s; leaves become HTML elements.
fn render_node(
    node: &ViewNode,
    ctx: &PageCtx,
    indent: usize,
    out: &mut String,
    diags: &mut Diagnostics,
) {
    let pad = "    ".repeat(indent);
    match node {
        ViewNode::Column { children, spacing, padding } => {
            render_flex("column", children, *spacing, *padding, ctx, indent, out, diags);
        }
        ViewNode::Row { children, spacing, padding } => {
            render_flex("row", children, *spacing, *padding, ctx, indent, out, diags);
        }
        ViewNode::Text(e) => {
            out.push_str(&format!("{}<p>{{ {} }}</p>\n", pad, text_content(e, ctx)));
        }
        ViewNode::Button { label, on_click } => {
            let lbl = text_content(label, ctx);
            match on_click {
                Some(ev) => out.push_str(&format!(
                    "{}<button onclick={{ctx.link().callback(|_| Message::{})}}>{{ {} }}</button>\n",
                    pad, ev, lbl
                )),
                None => out.push_str(&format!("{}<button>{{ {} }}</button>\n", pad, lbl)),
            }
        }
        // Layout sizing is not styled yet — render the child and say so.
        ViewNode::Constrained { child, .. } => {
            diags.error_once(
                "page-size",
                "`Length`/`Fill` sizing isn't supported in a Page yet — a browser lays out \
                 with CSS, which arrives in a later slice.",
            );
            render_node(child, ctx, indent, out, diags);
        }
        other => {
            diags.error_once(
                "page-widget",
                format!(
                    "That widget isn't supported in a Page yet ({}). Slice 1 supports Column, \
                     Row, Text, and Button.",
                    web_node_name(other)
                ),
            );
        }
    }
}

/// A `Column`/`Row` → a flexbox `<div>`, children one per line inside.
#[allow(clippy::too_many_arguments)]
fn render_flex(
    direction: &str,
    children: &[ViewNode],
    spacing: Option<u16>,
    padding: Option<u16>,
    ctx: &PageCtx,
    indent: usize,
    out: &mut String,
    diags: &mut Diagnostics,
) {
    let pad = "    ".repeat(indent);
    let mut style = format!("display: flex; flex-direction: {};", direction);
    if let Some(s) = spacing {
        style.push_str(&format!(" gap: {}px;", s));
    }
    if let Some(p) = padding {
        style.push_str(&format!(" padding: {}px;", p));
    }
    out.push_str(&format!("{}<div style=\"{}\">\n", pad, style));
    for c in children {
        render_node(c, ctx, indent + 1, out, diags);
    }
    out.push_str(&format!("{}</div>\n", pad));
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
        ViewNode::TextInput { .. } => "TextInput",
        ViewNode::TextArea { .. } => "TextArea",
        ViewNode::Checkbox { .. } => "Checkbox",
        ViewNode::Slider { .. } => "Slider",
        ViewNode::Toggler { .. } => "Toggler",
        ViewNode::ProgressBar { .. } => "ProgressBar",
        ViewNode::Radio { .. } => "Radio",
        ViewNode::Image { .. } => "Image",
        ViewNode::Canvas { .. } => "Canvas",
        ViewNode::Space { .. } => "Space",
        ViewNode::Match { .. } => "Match",
        ViewNode::If { .. } => "If",
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
