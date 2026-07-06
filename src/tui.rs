//! TUI codegen — a `Screen` becomes a ratatui terminal application.
//!
//! The same Elm-style model as the GUI backend (`gui.rs`): State is the source
//! of truth, the View is derived from it, and Events change it. The difference
//! is the renderer (ratatui, not Iced) and the input model — a terminal is
//! keyboard-driven, so a `Screen` binds keys to event handlers with a keymap
//! (`On Key "q" Quit`) rather than attaching events to widgets.
//!
//! Slice 1: `State` + a `View` of `Text` lines + `On Key` + `Event` → a State
//! struct, a `view(state, frame)` that draws a bordered `Paragraph`, and a
//! crossterm event loop that redraws on each keystroke and dispatches the keymap.
//!
//! Web (`vbr runweb`): the same `Screen` renders in a browser through Ratzilla,
//! which draws real ratatui widgets into the DOM. The State struct, `view` fn,
//! and event lowering are shared verbatim — only the shell differs: instead of
//! a blocking crossterm loop, `emit_web_main` wires the state (an
//! `Rc<RefCell<_>>`) into `on_key_event` + a `draw_web` render loop.

use crate::ast::*;
use crate::diagnostics::Diagnostics;
use crate::surface::{
    self, analyze_events, event_stdlib_imports, launched, match_scrutinee, render_init,
    rewrite_expr, state_maps, AwaitSplit,
};
use crate::transpiler::{decltype_rust, render_expr, rust_name};
use std::collections::{HashMap, HashSet};

/// Emit a complete TUI program: shared items (consts/structs/enums/functions),
/// each screen's definition, then `fn main`, which runs the screen launched by
/// `<Screen>.Run` inside `Function Main()`. With `web`, `fn main` is the
/// Ratzilla browser shell instead of the crossterm loop.
pub fn emit_tui_program(program: &Program, web: bool, diags: &mut Diagnostics) -> String {
    let mut out = String::new();
    let t = surface::build_tables(program);
    surface::emit_shared_items(program, &t, diags, &mut out, &mut |_, _, _| false);

    // Struct definitions by name — a `Table` reads its element struct's fields.
    let structs: HashMap<String, &StructDef> =
        program.structs.iter().map(|s| (s.name.clone(), s)).collect();

    for sc in &program.screens {
        out.push_str(&emit_screen(sc, &t, &structs, diags));
        out.push('\n');
    }
    let launched_screen = launched(program, |name| {
        program.screens.iter().find(|s| s.name.eq_ignore_ascii_case(name))
    });
    match launched_screen {
        Some(sc) if web => out.push_str(&emit_web_main(sc, &t, diags)),
        Some(sc) => out.push_str(&emit_main(sc, &t, diags)),
        None => diags.error_once(
            "tui-no-launch",
            "A screen is never launched. Add `Function Main()` containing `<Screen>.Run`, \
             e.g. `Counter.Run`.",
        ),
    }
    if web {
        // The fetch wrapper behind an awaited `Http.Get`, emitted once when used.
        if out.contains("http_get(") {
            out.push('\n');
            out.push_str(surface::HTTP_GET_HELPER);
        }
        // vbr_stdlib doesn't compile to WebAssembly, so the rest of the stdlib
        // is fenced in a browser Screen — `Await Http.Get` (above) is the one
        // door, and a *sync* `Http.Get` gets the blocking-without-Await error.
        let mut std_used: Vec<String> = Vec::new();
        for sc in &program.screens {
            for e in &sc.events {
                surface::collect_event_stdlib(&e.body, &mut std_used);
            }
        }
        std_used.extend(crate::transpiler::stdlib_used(diags));
        std_used.retain(|ns| ns != "Http");
        std_used.sort();
        std_used.dedup();
        if !std_used.is_empty() {
            diags.error_once(
                "tui-web-stdlib",
                format!(
                    "The standard library ({}) isn't available in a browser Screen — it \
                     doesn't compile to WebAssembly. For HTTP, use `Await Http.Get(url)` \
                     inside an event (it runs on the browser's fetch). The terminal version \
                     (`vbr runproject`) has the full stdlib.",
                    std_used.join(", ")
                ),
            );
        }
    }
    out
}

/// Emit one screen: the State struct + Default, the `view` fn, and (later) any
/// helpers. The event loop lives in `fn main` (emitted separately).
fn emit_screen(
    sc: &Screen,
    t: &surface::Tables,
    structs: &HashMap<String, &StructDef>,
    diags: &mut Diagnostics,
) -> String {
    let mut out = String::new();
    let ty = &sc.name;
    let enums = &t.enums;
    let (fields, field_ty) = state_maps(&sc.state);

    // Focusable widgets — Input (types into a String), List/Table (select over a
    // Vec). List/Table need a runtime `ListState`/`TableState`; >1 focusable adds
    // a shared focus index (Tab cycles). Only List/Table make the view `&mut`.
    let focusables = collect_focusables(&sc.view);
    let multi = focusables.len() > 1;
    let has_stateful = focusables.iter().any(Focusable::selectable);
    for fo in &focusables {
        validate_focusable(fo, &field_ty, structs, diags);
    }
    // field → focus index (an Input shows a cursor only when it is focused).
    let focus_map: HashMap<String, usize> =
        focusables.iter().enumerate().map(|(i, f)| (f.field.clone(), i)).collect();

    // Render the view body up front (into `inner`) — this decides which imports
    // are needed and whether the view reads `state`.
    let mut body = String::new();
    let mut counter = 0usize;
    render_view_node(&sc.view, "inner", &fields, &field_ty, enums, structs, &focus_map, multi, &mut counter, 1, &mut body, diags);

    // ── imports (only what the body uses, so it stays warning-free) ──
    let mut widgets = vec!["Block"];
    if body.contains("Paragraph::new(") {
        widgets.push("Paragraph");
    }
    out.push_str(&format!("use ratatui::widgets::{{{}}};\n", widgets.join(", ")));
    if body.contains("Layout::") {
        out.push_str("use ratatui::layout::{Constraint, Layout};\n");
    }
    out.push_str("use ratatui::Frame;\n\n");

    // ── State struct ──
    out.push_str(&format!("struct {} {{\n", ty));
    for f in &sc.state {
        out.push_str(&format!("    {}: {},\n", rust_name(&f.name), decltype_rust(&f.ty)));
    }
    for fo in &focusables {
        if let Some(st) = fo.state_ty() {
            out.push_str(&format!("    {}_state: ratatui::widgets::{},\n", fo.field, st));
        }
    }
    if multi {
        out.push_str("    focus_index: usize,\n");
    }
    out.push_str("}\n\n");

    // ── Default (the Dim initialisers) ──
    out.push_str(&format!("impl Default for {} {{\n    fn default() -> Self {{\n", ty));
    out.push_str(&format!("        {} {{\n", ty));
    for f in &sc.state {
        out.push_str(&format!(
            "            {}: {},\n",
            rust_name(&f.name),
            render_init(f.init.as_ref(), &f.ty, enums)
        ));
    }
    for fo in &focusables {
        if let Some(st) = fo.state_ty() {
            out.push_str(&format!(
                "            {}_state: ratatui::widgets::{}::default().with_selected(Some(0)),\n",
                fo.field, st
            ));
        }
    }
    if multi {
        out.push_str("            focus_index: 0,\n");
    }
    out.push_str("        }\n    }\n}\n\n");

    // ── view ──
    // `state` is `_state` when nothing reads it. A list/table makes the view
    // `&mut` (its state mutates when rendered); an input alone reads immutably
    // (typing happens in the event loop).
    let title = sc.title.clone().unwrap_or_else(|| sc.name.clone());
    let (param_name, param_ty) = if has_stateful {
        ("state".to_string(), format!("&mut {}", ty))
    } else if body.contains("state.") {
        ("state".to_string(), format!("&{}", ty))
    } else {
        ("_state".to_string(), format!("&{}", ty))
    };
    out.push_str(&format!("fn view({}: {}, frame: &mut Frame) {{\n", param_name, param_ty));
    out.push_str(&format!("    let block = Block::bordered().title({:?});\n", title));
    out.push_str("    let area = frame.area();\n");
    out.push_str("    let inner = block.inner(area);\n");
    out.push_str("    frame.render_widget(block, area);\n");
    out.push_str(&body);
    out.push_str("}\n");

    out
}

/// A focusable widget — Input, List, or Table. All three join the focus ring
/// (Tab cycles); the kind decides the widget, its optional runtime state type,
/// and which keys apply. `handler` is the `On Submit`/`On Select` event, if any.
#[derive(Clone, Copy, PartialEq)]
enum FocusKind {
    Input,
    List,
    Table,
}

struct Focusable {
    field: String,
    handler: Option<String>,
    kind: FocusKind,
}

impl Focusable {
    /// The runtime widget-state type (List/Table); an Input needs none.
    fn state_ty(&self) -> Option<&'static str> {
        match self.kind {
            FocusKind::List => Some("ListState"),
            FocusKind::Table => Some("TableState"),
            FocusKind::Input => None,
        }
    }
    fn selectable(&self) -> bool {
        matches!(self.kind, FocusKind::List | FocusKind::Table)
    }
    fn is_input(&self) -> bool {
        matches!(self.kind, FocusKind::Input)
    }
}

/// The focusable widgets in a view, in first-seen (Tab) order.
fn collect_focusables(view: &ViewNode) -> Vec<Focusable> {
    let mut out = Vec::new();
    fn walk(node: &ViewNode, out: &mut Vec<Focusable>) {
        let push = |out: &mut Vec<Focusable>, field: &str, handler: &Option<String>, kind| {
            out.push(Focusable { field: rust_name(field), handler: handler.clone(), kind });
        };
        match node {
            ViewNode::Input { field, on_submit } => push(out, field, on_submit, FocusKind::Input),
            ViewNode::List { field, on_select } => push(out, field, on_select, FocusKind::List),
            ViewNode::Table { field, on_select } => push(out, field, on_select, FocusKind::Table),
            ViewNode::Constrained { child, .. } => walk(child, out),
            ViewNode::Column { children, .. } | ViewNode::Row { children, .. } => {
                children.iter().for_each(|c| walk(c, out))
            }
            _ => {}
        }
    }
    walk(view, &mut out);
    out
}

/// An Input binds to `String`; a List to `Vec<String>`; a Table to `Vec<Struct>`.
fn validate_focusable(
    fo: &Focusable,
    field_ty: &HashMap<String, DeclType>,
    structs: &HashMap<String, &StructDef>,
    diags: &mut Diagnostics,
) {
    match fo.kind {
        FocusKind::Input => {
            if !matches!(field_ty.get(&fo.field), Some(DeclType::Plain(Type::Text))) {
                diags.error_once(
                    &format!("input-field-{}", fo.field),
                    format!("An Input binds to a `String` state field — `{}` isn't one.", fo.field),
                );
            }
        }
        FocusKind::Table => {
            let ok = matches!(
                field_ty.get(&fo.field),
                Some(DeclType::Vec(inner))
                    if matches!(&**inner, DeclType::Named(n) if structs.get(n).is_some_and(|sd| !sd.fields.is_empty()))
            );
            if !ok {
                diags.error_once(
                    &format!("table-field-{}", fo.field),
                    format!(
                        "A Table binds to a `Vec<Struct>` state field (its columns come from the \
                         struct's fields) — `{}` isn't one.",
                        fo.field
                    ),
                );
            }
        }
        FocusKind::List => {
            let ok = matches!(field_ty.get(&fo.field), Some(DeclType::Vec(inner)) if matches!(**inner, DeclType::Plain(Type::Text)));
            if !ok {
                diags.error_once(
                    &format!("list-field-{}", fo.field),
                    format!("A List binds to a `Vec<String>` state field — `{}` isn't one.", fo.field),
                );
            }
        }
    }
}

/// Recursively emit the render statements for a view node into `area` (a Rust
/// expression naming the ratatui `Rect` to draw into). Containers split their
/// area with a `Layout`; leaves render a widget.
fn render_view_node(
    node: &ViewNode,
    area: &str,
    fields: &HashSet<String>,
    field_ty: &HashMap<String, DeclType>,
    enums: &HashSet<String>,
    structs: &HashMap<String, &StructDef>,
    focus_map: &HashMap<String, usize>,
    multi: bool,
    counter: &mut usize,
    indent: usize,
    out: &mut String,
    diags: &mut Diagnostics,
) {
    let pad = "    ".repeat(indent);
    match node {
        // A constraint is consumed by the parent container; render the child.
        ViewNode::Constrained { child, .. } => render_view_node(
            child, area, fields, field_ty, enums, structs, focus_map, multi, counter, indent, out,
            diags,
        ),
        ViewNode::Text(e) => {
            out.push_str(&format!(
                "{}frame.render_widget(Paragraph::new({}), {});\n",
                pad,
                text_content(e, fields, enums),
                area
            ));
        }
        ViewNode::Column { children, spacing, padding }
        | ViewNode::Row { children, spacing, padding } => {
            let vertical = matches!(node, ViewNode::Column { .. });
            let id = *counter;
            *counter += 1;
            let ctor = if vertical { "vertical" } else { "horizontal" };
            let cons: Vec<String> = children.iter().map(|c| child_constraint(c)).collect();
            let mut builder = format!("Layout::{}([{}])", ctor, cons.join(", "));
            if let Some(s) = spacing {
                builder.push_str(&format!(".spacing({})", s));
            }
            if let Some(p) = padding {
                builder.push_str(&format!(".margin({})", p));
            }
            out.push_str(&format!("{}let chunks_{} = {}.split({});\n", pad, id, builder, area));
            for (i, child) in children.iter().enumerate() {
                let sub = format!("chunks_{}[{}]", id, i);
                render_view_node(
                    child, &sub, fields, field_ty, enums, structs, focus_map, multi, counter,
                    indent, out, diags,
                );
            }
        }
        ViewNode::Input { field, .. } => {
            let f = rust_name(field);
            out.push_str(&format!(
                "{}frame.render_widget(Paragraph::new(state.{}.as_str())\
                 .block(Block::bordered().title({:?})), {});\n",
                pad, f, field, area
            ));
            // Place the terminal cursor at the end of the text when focused.
            let idx = focus_map.get(&f).copied().unwrap_or(0);
            let set_cursor = format!(
                "frame.set_cursor_position(({}.x + 1 + state.{}.chars().count() as u16, {}.y + 1));",
                area, f, area
            );
            if multi {
                out.push_str(&format!(
                    "{}if state.focus_index == {} {{ {} }}\n",
                    pad, idx, set_cursor
                ));
            } else {
                out.push_str(&format!("{}{}\n", pad, set_cursor));
            }
        }
        ViewNode::List { field, .. } => {
            let f = rust_name(field);
            let id = *counter;
            *counter += 1;
            out.push_str(&format!(
                "{}let items_{}: Vec<ratatui::widgets::ListItem> = \
                 state.{}.iter().map(|s| ratatui::widgets::ListItem::new(s.clone())).collect();\n",
                pad, id, f
            ));
            out.push_str(&format!(
                "{}let list_{} = ratatui::widgets::List::new(items_{}).highlight_symbol(\"\u{bb} \")\
                 .highlight_style(ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED));\n",
                pad, id, id
            ));
            out.push_str(&format!(
                "{}frame.render_stateful_widget(list_{}, {}, &mut state.{}_state);\n",
                pad, id, area, f
            ));
        }
        ViewNode::Table { field, .. } => {
            let f = rust_name(field);
            let id = *counter;
            *counter += 1;
            // Columns come from the element struct's fields (validated earlier).
            let cols: &[Field] = match field_ty.get(&f) {
                Some(DeclType::Vec(inner)) => match &**inner {
                    DeclType::Named(n) => structs.get(n).map(|s| s.fields.as_slice()).unwrap_or(&[]),
                    _ => &[],
                },
                _ => &[],
            };
            // Each row: one cell per struct field (owned String).
            let cells: Vec<String> = cols
                .iter()
                .map(|c| {
                    let acc = format!("row.{}", rust_name(&c.name));
                    match &c.ty {
                        DeclType::Plain(Type::Text) => format!("{}.clone()", acc),
                        _ => format!("{}.to_string()", acc),
                    }
                })
                .collect();
            let headers: Vec<String> = cols.iter().map(|c| format!("{:?}", c.name)).collect();
            let widths: Vec<&str> = cols.iter().map(|_| "Constraint::Fill(1)").collect();
            out.push_str(&format!(
                "{}let rows_{}: Vec<ratatui::widgets::Row> = state.{}.iter()\
                 .map(|row| ratatui::widgets::Row::new(vec![{}])).collect();\n",
                pad, id, f, cells.join(", ")
            ));
            out.push_str(&format!(
                "{}let table_{} = ratatui::widgets::Table::new(rows_{}, [{}])\n",
                pad, id, id, widths.join(", ")
            ));
            out.push_str(&format!(
                "{}    .header(ratatui::widgets::Row::new(vec![{}])\
                 .style(ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::BOLD)))\n",
                pad,
                headers.join(", ")
            ));
            out.push_str(&format!(
                "{}    .row_highlight_style(ratatui::style::Style::new()\
                 .add_modifier(ratatui::style::Modifier::REVERSED)).highlight_symbol(\"\u{bb} \");\n",
                pad
            ));
            out.push_str(&format!(
                "{}frame.render_stateful_widget(table_{}, {}, &mut state.{}_state);\n",
                pad, id, area, f
            ));
        }
        // A progress gauge over min..=max (ratatui `Gauge`, clamped to 0..=1).
        ViewNode::Gauge { min, max, value } => {
            let f = rust_name(value);
            if !matches!(field_ty.get(&f), Some(DeclType::Plain(t)) if !matches!(t, Type::Text | Type::Boolean)) {
                diags.error_once(
                    &format!("gauge-field-{}", f),
                    format!("A Gauge shows a number — `{}` must be a numeric field.", value),
                );
            }
            let lo = render_expr(&rewrite_expr(min.clone(), fields, enums), None);
            let hi = render_expr(&rewrite_expr(max.clone(), fields, enums), None);
            let id = *counter;
            *counter += 1;
            out.push_str(&format!(
                "{}let ratio_{} = ((state.{} as f64 - {} as f64) / ({} as f64 - {} as f64)).clamp(0.0, 1.0);\n",
                pad, id, f, lo, hi, lo
            ));
            out.push_str(&format!(
                "{}frame.render_widget(ratatui::widgets::Gauge::default()\
                 .block(Block::bordered().title({:?})).ratio(ratio_{}), {});\n",
                pad, value, id, area
            ));
        }
        // A trend line over a Vec of numbers (ratatui `Sparkline`).
        ViewNode::Sparkline { field } => {
            let f = rust_name(field);
            if !matches!(field_ty.get(&f), Some(DeclType::Vec(inner)) if matches!(**inner, DeclType::Plain(t) if !matches!(t, Type::Text | Type::Boolean))) {
                diags.error_once(
                    &format!("sparkline-field-{}", f),
                    format!("A Sparkline shows a `Vec` of numbers — `{}` isn't one.", field),
                );
            }
            let id = *counter;
            *counter += 1;
            out.push_str(&format!(
                "{}let spark_{}: Vec<u64> = state.{}.iter().map(|&v| v as u64).collect();\n",
                pad, id, f
            ));
            out.push_str(&format!(
                "{}frame.render_widget(ratatui::widgets::Sparkline::default()\
                 .block(Block::bordered().title({:?})).data(&spark_{}), {});\n",
                pad, field, id, area
            ));
        }
        // Bars over a Vec<Struct>: first String field labels, first number heights.
        ViewNode::BarChart { field } => {
            let f = rust_name(field);
            match barchart_columns(&f, field_ty, structs) {
                Some((label, val)) => {
                    let id = *counter;
                    *counter += 1;
                    out.push_str(&format!(
                        "{}let bars_{}: Vec<(&str, u64)> = state.{}.iter()\
                         .map(|it| (it.{}.as_str(), it.{} as u64)).collect();\n",
                        pad, id, f, label, val
                    ));
                    out.push_str(&format!(
                        "{}frame.render_widget(ratatui::widgets::BarChart::default()\
                         .block(Block::bordered().title({:?})).data(&bars_{}).bar_width(7), {});\n",
                        pad, field, id, area
                    ));
                }
                None => diags.error_once(
                    &format!("barchart-field-{}", f),
                    format!(
                        "A BarChart binds to a `Vec<Struct>` whose struct has a `String` field \
                         (the bar label) and a numeric field (the bar height) — `{}` isn't one.",
                        field
                    ),
                ),
            }
        }
        // An X/Y line/scatter chart (ratatui `Chart`) — one or more series, each
        // its own colour + legend. Axis bounds explicit (`XAxis`/`YAxis`) or
        // auto-computed across all series (fallback 0..1 when empty).
        ViewNode::Chart { fields: series, scatter, x_bounds, y_bounds } => {
            let cols: Option<Vec<(String, String)>> =
                series.iter().map(|f| chart_xy_columns(&rust_name(f), field_ty, structs)).collect();
            let Some(cols) = cols else {
                diags.error_once(
                    "chart-field",
                    "A Chart series binds to a `Vec<Struct>` whose struct has at least two numeric \
                     fields (the x and y of each point).",
                );
                return;
            };
            let id = *counter;
            *counter += 1;
            let graph = if *scatter { "Scatter" } else { "Line" };
            const COLORS: &[&str] = &["Cyan", "Yellow", "Green", "Magenta", "Red", "Blue"];
            // Per-series point vecs.
            for (k, (f, (xf, yf))) in series.iter().zip(&cols).enumerate() {
                out.push_str(&format!(
                    "{}let pts_{}_{}: Vec<(f64, f64)> = state.{}.iter().map(|p| (p.{} as f64, p.{} as f64)).collect();\n",
                    pad, id, k, rust_name(f), xf, yf
                ));
            }
            // The iterator over every series' points (for auto bounds).
            let chain = |sel: usize| -> String {
                let mut s = format!("pts_{}_0.iter()", id);
                for k in 1..series.len() {
                    s = format!("{}.chain(pts_{}_{}.iter())", s, id, k);
                }
                format!("{}.map(|p| p.{})", s, sel)
            };
            // Axis bounds: explicit, or folded over the data.
            for (axis, sel, bounds) in [("x", 0usize, x_bounds), ("y", 1usize, y_bounds)] {
                match bounds {
                    Some((lo, hi)) => {
                        let lo = render_expr(&rewrite_expr(lo.clone(), fields, enums), None);
                        let hi = render_expr(&rewrite_expr(hi.clone(), fields, enums), None);
                        out.push_str(&format!("{}let {}lo_{} = ({}) as f64;\n", pad, axis, id, lo));
                        out.push_str(&format!("{}let {}hi_{} = ({}) as f64;\n", pad, axis, id, hi));
                    }
                    None => {
                        out.push_str(&format!(
                            "{}let {}lo_{} = {}.fold(f64::INFINITY, f64::min);\n",
                            pad, axis, id, chain(sel)
                        ));
                        out.push_str(&format!(
                            "{}let {}hi_{} = {}.fold(f64::NEG_INFINITY, f64::max);\n",
                            pad, axis, id, chain(sel)
                        ));
                        out.push_str(&format!(
                            "{}let ({}lo_{}, {}hi_{}) = if {}lo_{} <= {}hi_{} {{ ({}lo_{}, {}hi_{}) }} else {{ (0.0, 1.0) }};\n",
                            pad, axis, id, axis, id, axis, id, axis, id, axis, id, axis, id
                        ));
                    }
                }
            }
            // Datasets (one per series, cycling colours + a legend name).
            let mut names: Vec<String> = Vec::new();
            for (k, f) in series.iter().enumerate() {
                out.push_str(&format!(
                    "{}let dataset_{}_{} = ratatui::widgets::Dataset::default().name({:?})\
                     .marker(ratatui::symbols::Marker::Braille).graph_type(ratatui::widgets::GraphType::{})\
                     .style(ratatui::style::Style::new().fg(ratatui::style::Color::{})).data(&pts_{}_{});\n",
                    pad, id, k, f, graph, COLORS[k % COLORS.len()], id, k
                ));
                names.push(format!("dataset_{}_{}", id, k));
            }
            let title = series.join(", ");
            out.push_str(&format!(
                "{}let chart_{} = ratatui::widgets::Chart::new(vec![{}]).block(Block::bordered().title({:?}))\n",
                pad, id, names.join(", "), title
            ));
            out.push_str(&format!(
                "{}    .x_axis(ratatui::widgets::Axis::default().bounds([xlo_{}, xhi_{}]).labels(vec![format!(\"{{:.1}}\", xlo_{}), format!(\"{{:.1}}\", xhi_{})]))\n",
                pad, id, id, id, id
            ));
            out.push_str(&format!(
                "{}    .y_axis(ratatui::widgets::Axis::default().bounds([ylo_{}, yhi_{}]).labels(vec![format!(\"{{:.1}}\", ylo_{}), format!(\"{{:.1}}\", yhi_{})]));\n",
                pad, id, id, id, id
            ));
            out.push_str(&format!("{}frame.render_widget(chart_{}, {});\n", pad, id, area));
        }
        // `Match <expr>` in the view → a Rust `match` whose arm renders its
        // widget(s) into the same area.
        ViewNode::Match { scrutinee, arms } => {
            let subj = match_scrutinee(scrutinee, "state", fields, field_ty, enums);
            out.push_str(&format!("{}match {} {{\n", pad, subj));
            for arm in arms {
                let guard = match &arm.guard {
                    Some(g) => format!(
                        " if {}",
                        render_expr(&rewrite_expr(g.clone(), fields, enums), None)
                    ),
                    None => String::new(),
                };
                out.push_str(&format!("{}    {}{} => {{\n", pad, arm.pattern, guard));
                render_body_nodes(
                    &arm.body, area, fields, field_ty, enums, structs, focus_map, multi, counter,
                    indent + 2, out, diags,
                );
                out.push_str(&format!("{}    }}\n", pad));
            }
            out.push_str(&format!("{}}}\n", pad));
        }
        // `If <cond> Then … [ElseIf …] [Else …]` in the view → a Rust `if`/`else`
        // that renders the chosen branch's widget(s) into the area.
        ViewNode::If { branches, else_body } => {
            for (i, (cond, body)) in branches.iter().enumerate() {
                let c = render_expr(&rewrite_expr(cond.clone(), fields, enums), None);
                let kw = if i == 0 { "if" } else { "} else if" };
                out.push_str(&format!("{}{} {} {{\n", pad, kw, c));
                render_body_nodes(
                    body, area, fields, field_ty, enums, structs, focus_map, multi, counter,
                    indent + 1, out, diags,
                );
            }
            if let Some(b) = else_body {
                out.push_str(&format!("{}}} else {{\n", pad));
                render_body_nodes(
                    b, area, fields, field_ty, enums, structs, focus_map, multi, counter,
                    indent + 1, out, diags,
                );
            }
            out.push_str(&format!("{}}}\n", pad));
        }
        other => {
            diags.error_once(
                "tui-widget-unsupported",
                format!(
                    "That widget isn't supported in a Screen yet ({}). A Screen supports Column, \
                     Row, Text, Input, List, Table, Gauge, Sparkline, BarChart, Chart, Match, and If.",
                    tui_node_name(other)
                ),
            );
        }
    }
}

/// Render an arm/branch body (a list of view nodes) into `area`: one node fills
/// the area; several stack vertically (an implicit Column).
#[allow(clippy::too_many_arguments)]
fn render_body_nodes(
    body: &[ViewNode],
    area: &str,
    fields: &HashSet<String>,
    field_ty: &HashMap<String, DeclType>,
    enums: &HashSet<String>,
    structs: &HashMap<String, &StructDef>,
    focus_map: &HashMap<String, usize>,
    multi: bool,
    counter: &mut usize,
    indent: usize,
    out: &mut String,
    diags: &mut Diagnostics,
) {
    match body {
        [] => {}
        [one] => render_view_node(
            one, area, fields, field_ty, enums, structs, focus_map, multi, counter, indent, out,
            diags,
        ),
        many => {
            let col = ViewNode::Column { children: many.to_vec(), spacing: None, padding: None };
            render_view_node(
                &col, area, fields, field_ty, enums, structs, focus_map, multi, counter, indent,
                out, diags,
            );
        }
    }
}

/// The ratatui `Constraint` for a child — its explicit size, or a sensible
/// default: containers/conditionals/scrollables fill leftover space, an input is
/// a bordered line (3 rows), a `Text` takes one line.
fn child_constraint(node: &ViewNode) -> String {
    match node {
        ViewNode::Constrained { size, .. } => constraint_expr(*size),
        ViewNode::Column { .. }
        | ViewNode::Row { .. }
        | ViewNode::Match { .. }
        | ViewNode::If { .. }
        | ViewNode::List { .. }
        | ViewNode::Table { .. } => "Constraint::Fill(1)".to_string(),
        ViewNode::Input { .. } | ViewNode::Gauge { .. } => "Constraint::Length(3)".to_string(),
        ViewNode::Sparkline { .. } | ViewNode::BarChart { .. } | ViewNode::Chart { .. } => {
            "Constraint::Fill(1)".to_string()
        }
        _ => "Constraint::Length(1)".to_string(),
    }
}

/// For a `Chart` field (a `Vec<Struct>`), the struct's first two numeric fields
/// (the x and y of each point), snake-cased.
fn chart_xy_columns(
    field: &str,
    field_ty: &HashMap<String, DeclType>,
    structs: &HashMap<String, &StructDef>,
) -> Option<(String, String)> {
    let struct_name = match field_ty.get(field) {
        Some(DeclType::Vec(inner)) => match &**inner {
            DeclType::Named(n) => n,
            _ => return None,
        },
        _ => return None,
    };
    let sd = structs.get(struct_name)?;
    let mut nums = sd
        .fields
        .iter()
        .filter(|f| matches!(f.ty, DeclType::Plain(t) if !matches!(t, Type::Text | Type::Boolean)))
        .map(|f| rust_name(&f.name));
    Some((nums.next()?, nums.next()?))
}

/// For a `BarChart` field (a `Vec<Struct>`), the struct's first `String` field
/// (the bar label) and first numeric field (the bar height), snake-cased.
fn barchart_columns(
    field: &str,
    field_ty: &HashMap<String, DeclType>,
    structs: &HashMap<String, &StructDef>,
) -> Option<(String, String)> {
    let struct_name = match field_ty.get(field) {
        Some(DeclType::Vec(inner)) => match &**inner {
            DeclType::Named(n) => n,
            _ => return None,
        },
        _ => return None,
    };
    let sd = structs.get(struct_name)?;
    let label = sd
        .fields
        .iter()
        .find(|f| matches!(f.ty, DeclType::Plain(Type::Text)))
        .map(|f| rust_name(&f.name))?;
    let value = sd
        .fields
        .iter()
        .find(|f| matches!(f.ty, DeclType::Plain(t) if !matches!(t, Type::Text | Type::Boolean)))
        .map(|f| rust_name(&f.name))?;
    Some((label, value))
}

fn constraint_expr(size: SizeConstraint) -> String {
    match size {
        SizeConstraint::Length(n) => format!("Constraint::Length({})", n),
        SizeConstraint::Percent(n) => format!("Constraint::Percentage({})", n),
        SizeConstraint::Fill(n) => format!("Constraint::Fill({})", n),
        SizeConstraint::Min(n) => format!("Constraint::Min({})", n),
    }
}

/// A short name for an unsupported node, for the diagnostic.
fn tui_node_name(node: &ViewNode) -> &'static str {
    match node {
        ViewNode::Button { .. } => "Button",
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
        _ => "widget",
    }
}

/// `fn main`: the crossterm event loop. Redraw from state, read a key, dispatch
/// the keymap (a handler event's body, or `Quit` → break), repeat.
fn emit_main(sc: &Screen, t: &surface::Tables, diags: &mut Diagnostics) -> String {
    let ty = &sc.name;
    let (fields, field_ty) = state_maps(&sc.state);
    let events: HashMap<String, &GuiEvent> =
        sc.events.iter().map(|e| (e.name.to_ascii_lowercase(), e)).collect();

    // Async: split each event around an `Await`. An async event kicks its blocking
    // work onto a background thread and delivers the result over a channel.
    // (`analyze_events` also checks nothing blocking runs un-`Await`ed.)
    let splits: Vec<Option<AwaitSplit>> =
        analyze_events(&sc.events, &field_ty, &t.fns, diags, surface::AsyncBackend::Native);
    let any_async = splits.iter().any(Option::is_some);
    let async_by_name: HashMap<String, &AwaitSplit> = sc
        .events
        .iter()
        .zip(&splits)
        .filter_map(|(e, s)| s.as_ref().map(|s| (e.name.to_ascii_lowercase(), s)))
        .collect();

    let focusables = collect_focusables(&sc.view);
    let has_focus = !focusables.is_empty();
    let has_sel = focusables.iter().any(Focusable::selectable);
    let has_input = focusables.iter().any(Focusable::is_input);
    let multi = focusables.len() > 1;
    // Keys the user bound explicitly — their bindings win over the built-in
    // navigation (so we skip a built-in arm whose key they've taken).
    let user_keys: HashSet<String> = sc.keys.iter().map(|k| key_pattern(&k.key)).collect();

    let mut out = String::new();
    let mut dummy = Diagnostics::new();

    // Async preamble: the stdlib import + a `Message` enum of continuations.
    if any_async {
        let std_used = event_stdlib_imports(&sc.events, diags);
        if !std_used.is_empty() {
            out.push_str(&format!("use vbr_stdlib::{{{}}};\n\n", std_used.join(", ")));
        }
        out.push_str("enum Message {\n");
        for (e, split) in sc.events.iter().zip(&splits) {
            if let Some(s) = split {
                out.push_str(&format!("    {}Done({}),\n", e.name, s.ret_type));
            }
        }
        out.push_str("}\n\n");
    }

    let has_timer = !sc.timers.is_empty();
    // A timer or an async result means the loop can't just block on a keystroke —
    // it must tick (poll input briefly, then loop) so timers fire / results land.
    let poll_loop = any_async || has_timer;

    // `state` needs `mut` if a key/timer runs an event that changes it, or a
    // focusable widget mutates it (typing into an input, moving a selection).
    let mutates = has_focus
        || has_timer
        || sc.keys.iter().any(|k| events.contains_key(&k.handler.to_ascii_lowercase()));
    let let_state = if mutates { "let mut state" } else { "let state" };
    // Only a list/table makes the view `&mut` (its state mutates on render).
    let draw_arg = if has_sel { "&mut state" } else { "&state" };
    out.push_str("fn main() -> std::io::Result<()> {\n");
    out.push_str("    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};\n");
    out.push_str(&format!("    {} = {}::default();\n", let_state, ty));
    out.push_str("    let mut terminal = ratatui::init();\n");
    if any_async {
        out.push_str("    let (tx, rx) = std::sync::mpsc::channel::<Message>();\n");
    }
    for i in 0..sc.timers.len() {
        out.push_str(&format!("    let mut last_tick_{} = std::time::Instant::now();\n", i));
    }
    out.push_str("    loop {\n");
    out.push_str(&format!("        terminal.draw(|frame| view({}, frame))?;\n", draw_arg));
    if any_async {
        // Deliver any completed background results.
        out.push_str("        while let Ok(msg) = rx.try_recv() {\n");
        out.push_str("            match msg {\n");
        for (e, split) in sc.events.iter().zip(&splits) {
            if let Some(s) = split {
                out.push_str(&format!("                Message::{}Done({}) => {{\n", e.name, s.bind));
                surface::emit_event_stmts(&s.cont, &e.params, "state", &fields, &field_ty, t, 5, &mut dummy, &mut out);
                out.push_str("                }\n");
            }
        }
        out.push_str("            }\n");
        out.push_str("        }\n");
    }
    // Fire any due timers.
    for (i, tm) in sc.timers.iter().enumerate() {
        out.push_str(&format!(
            "        if last_tick_{}.elapsed().as_millis() >= {} {{\n",
            i, tm.interval_ms
        ));
        let handler = tm.handler.to_ascii_lowercase();
        if tm.handler.eq_ignore_ascii_case("Quit") {
            out.push_str("            break;\n");
        } else if let Some(ev) = events.get(&handler) {
            emit_event_run(
                ev, async_by_name.get(&handler).copied(), 3, &fields, &field_ty, t, &mut out,
                &mut dummy,
            );
        }
        out.push_str(&format!("            last_tick_{} = std::time::Instant::now();\n", i));
        out.push_str("        }\n");
    }
    if poll_loop {
        // Only block briefly on input so the loop keeps ticking.
        out.push_str("        if !event::poll(std::time::Duration::from_millis(50))? {\n");
        out.push_str("            continue;\n");
        out.push_str("        }\n");
    }
    out.push_str("        if let Event::Key(key) = event::read()? {\n");
    out.push_str("            if key.kind == KeyEventKind::Press {\n");
    out.push_str("                match key.code {\n");
    for k in &sc.keys {
        out.push_str(&format!("                    {} => {{\n", key_pattern(&k.key)));
        let handler = k.handler.to_ascii_lowercase();
        if k.handler.eq_ignore_ascii_case("Quit") {
            out.push_str("                        break;\n");
        } else if let Some(ev) = events.get(&handler) {
            emit_event_run(
                ev, async_by_name.get(&handler).copied(), 6, &fields, &field_ty, t, &mut out,
                &mut dummy,
            );
        }
        out.push_str("                    }\n");
    }
    // Built-in focus navigation (only for keys the user hasn't bound). Order
    // matters: specific user `Char('x')` arms above, general `Char(c)` typing last.
    if has_sel && !user_keys.contains("KeyCode::Down") {
        out.push_str("                    KeyCode::Down => {\n");
        out.push_str(&nav_dispatch(&focusables, multi, "select_next", 6));
        out.push_str("                    }\n");
    }
    if has_sel && !user_keys.contains("KeyCode::Up") {
        out.push_str("                    KeyCode::Up => {\n");
        out.push_str(&nav_dispatch(&focusables, multi, "select_previous", 6));
        out.push_str("                    }\n");
    }
    if multi && !user_keys.contains("KeyCode::Tab") {
        out.push_str(&format!(
            "                    KeyCode::Tab => {{\n                        \
             state.focus_index = (state.focus_index + 1) % {};\n                    }}\n",
            focusables.len()
        ));
    }
    if has_focus && !user_keys.contains("KeyCode::Enter") {
        out.push_str("                    KeyCode::Enter => {\n");
        out.push_str(&enter_dispatch(&focusables, multi, &events, &fields, &field_ty, t, 6));
        out.push_str("                    }\n");
    }
    if has_input && !user_keys.contains("KeyCode::Backspace") {
        out.push_str("                    KeyCode::Backspace => {\n");
        out.push_str(&input_dispatch(&focusables, multi, &|f| format!("state.{}.pop();", f), 6));
        out.push_str("                    }\n");
    }
    if has_input {
        out.push_str("                    KeyCode::Char(c) => {\n");
        out.push_str(&input_dispatch(&focusables, multi, &|f| format!("state.{}.push(c);", f), 6));
        out.push_str("                    }\n");
    }
    out.push_str("                    _ => {}\n");
    out.push_str("                }\n");
    out.push_str("            }\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("    ratatui::restore();\n");
    out.push_str("    Ok(())\n");
    out.push_str("}\n");
    out
}

/// `fn main` for the browser (`vbr runweb`): the Ratzilla shell. The terminal
/// is drawn into the DOM; the state lives in an `Rc<RefCell<_>>` shared by the
/// key handler (which dispatches the same keymap as the native loop) and the
/// `draw_web` render loop. The State struct, `view`, and event bodies are the
/// exact ones the native shell uses.
fn emit_web_main(sc: &Screen, t: &surface::Tables, diags: &mut Diagnostics) -> String {
    let ty = &sc.name;
    let (fields, field_ty) = state_maps(&sc.state);
    let events: HashMap<String, &GuiEvent> =
        sc.events.iter().map(|e| (e.name.to_ascii_lowercase(), e)).collect();

    // Async: split each event around an `Await`. In the browser the awaited
    // work is the browser's own fetch, and the continuation runs in a spawned
    // future that re-borrows the state when the result lands — no channel, no
    // extra thread. (`analyze_events` also checks nothing blocking runs
    // un-`Await`ed.)
    let splits: Vec<Option<AwaitSplit>> =
        analyze_events(&sc.events, &field_ty, &t.fns, diags, surface::AsyncBackend::WebScreen);
    let async_by_name: HashMap<String, &AwaitSplit> = sc
        .events
        .iter()
        .zip(&splits)
        .filter_map(|(e, s)| s.as_ref().map(|s| (e.name.to_ascii_lowercase(), s)))
        .collect();

    // The focusable widgets and their built-in navigation — the same dispatch
    // the native loop wires up, inside the browser key handler.
    let focusables = collect_focusables(&sc.view);
    let has_focus = !focusables.is_empty();
    let has_sel = focusables.iter().any(Focusable::selectable);
    let has_input = focusables.iter().any(Focusable::is_input);
    let multi = focusables.len() > 1;
    let user_keys: HashSet<String> = sc.keys.iter().map(|k| key_pattern(&k.key)).collect();

    // A browser page can't quit itself, so `Quit` bindings (key or timer) are
    // dropped on the web — say so rather than diverge silently.
    if sc.keys.iter().any(|k| k.handler.eq_ignore_ascii_case("Quit"))
        || sc.timers.iter().any(|tm| tm.handler.eq_ignore_ascii_case("Quit"))
    {
        diags.note(
            "tui-web-quit",
            "A `Quit` key has no meaning in a browser (close the tab instead), so that \
             binding is left out of the web build.",
        );
    }
    let arms: Vec<&KeyBinding> =
        sc.keys.iter().filter(|k| !k.handler.eq_ignore_ascii_case("Quit")).collect();

    let mut out = String::new();
    let mut dummy = Diagnostics::new();
    let handle_keys = !arms.is_empty() || has_focus;
    out.push_str("fn main() -> std::io::Result<()> {\n");
    out.push_str("    use ratzilla::{DomBackend, WebRenderer};\n");
    if handle_keys {
        out.push_str("    use ratzilla::event::KeyCode;\n");
    }
    out.push_str(&format!(
        "    let state = std::rc::Rc::new(std::cell::RefCell::new({}::default()));\n",
        ty
    ));
    out.push_str("    let backend = DomBackend::new()?;\n");
    // `on_key_event` needs `&mut terminal`; with no bindings, skip it (and the
    // `mut`) so an output-only screen stays warning-free.
    let let_term = if handle_keys { "let mut terminal" } else { "let terminal" };
    out.push_str(&format!("    {} = ratzilla::ratatui::Terminal::new(backend)?;\n", let_term));
    if handle_keys {
        // An async handler's spawned future needs the shareable handle, which
        // the reborrow below shadows — keep it reachable as `rc`.
        let key_async =
            arms.iter().any(|k| async_by_name.contains_key(&k.handler.to_ascii_lowercase()));
        out.push_str("    terminal.on_key_event({\n");
        out.push_str("        let state = state.clone();\n");
        out.push_str("        move |key| {\n");
        if key_async {
            out.push_str("            let rc = state.clone();\n");
        }
        // Reborrow the RefCell guard into a plain `&mut`, so one statement can
        // touch two state fields (`state.history.push(state.level)`) — through
        // the guard itself, the borrow checker can't split fields across the
        // deref, which a native `&mut state` allows.
        out.push_str("            let mut guard = state.borrow_mut();\n");
        out.push_str("            let state = &mut *guard;\n");
        out.push_str("            match key.code {\n");
        for k in &arms {
            out.push_str(&format!("                {} => {{\n", key_pattern(&k.key)));
            let handler = k.handler.to_ascii_lowercase();
            if let Some(ev) = events.get(&handler) {
                emit_web_event_run(
                    ev,
                    async_by_name.get(&handler).copied(),
                    5,
                    &fields,
                    &field_ty,
                    t,
                    &mut out,
                    &mut dummy,
                );
            }
            out.push_str("                }\n");
        }
        // Built-in focus navigation — same rules as the native loop: a key the
        // user bound wins; the general `Char(c)` typing arm comes last.
        if has_sel && !user_keys.contains("KeyCode::Down") {
            out.push_str("                KeyCode::Down => {\n");
            out.push_str(&nav_dispatch(&focusables, multi, "select_next", 5));
            out.push_str("                }\n");
        }
        if has_sel && !user_keys.contains("KeyCode::Up") {
            out.push_str("                KeyCode::Up => {\n");
            out.push_str(&nav_dispatch(&focusables, multi, "select_previous", 5));
            out.push_str("                }\n");
        }
        if multi && !user_keys.contains("KeyCode::Tab") {
            out.push_str(&format!(
                "                KeyCode::Tab => {{\n                    \
                 state.focus_index = (state.focus_index + 1) % {};\n                }}\n",
                focusables.len()
            ));
        }
        if has_focus && !user_keys.contains("KeyCode::Enter") {
            out.push_str("                KeyCode::Enter => {\n");
            out.push_str(&enter_dispatch(&focusables, multi, &events, &fields, &field_ty, t, 5));
            out.push_str("                }\n");
        }
        if has_input && !user_keys.contains("KeyCode::Backspace") {
            out.push_str("                KeyCode::Backspace => {\n");
            out.push_str(&input_dispatch(&focusables, multi, &|f| format!("state.{}.pop();", f), 5));
            out.push_str("                }\n");
        }
        if has_input {
            out.push_str("                KeyCode::Char(c) => {\n");
            out.push_str(&input_dispatch(&focusables, multi, &|f| format!("state.{}.push(c);", f), 5));
            out.push_str("                }\n");
        }
        out.push_str("                _ => {}\n");
        out.push_str("            }\n");
        out.push_str("        }\n");
        out.push_str("    })?;\n");
    }
    // Each `Every` becomes a browser interval timer running the same handler
    // body; `.forget()` keeps it ticking for the life of the page. (Ratzilla's
    // render loop redraws continuously, so the state change just shows up.)
    for tm in &sc.timers {
        let handler = tm.handler.to_ascii_lowercase();
        let Some(ev) = events.get(&handler) else {
            continue; // `Quit` (noted above) or an unknown handler
        };
        out.push_str(&format!(
            "    gloo_timers::callback::Interval::new({}, {{\n",
            tm.interval_ms
        ));
        out.push_str("        let state = state.clone();\n");
        out.push_str("        move || {\n");
        if async_by_name.contains_key(&handler) {
            out.push_str("            let rc = state.clone();\n");
        }
        out.push_str("            let mut guard = state.borrow_mut();\n");
        out.push_str("            let state = &mut *guard;\n");
        emit_web_event_run(
            ev,
            async_by_name.get(&handler).copied(),
            3,
            &fields,
            &field_ty,
            t,
            &mut out,
            &mut dummy,
        );
        out.push_str("        }\n");
        out.push_str("    })\n");
        out.push_str("    .forget();\n");
    }
    // Only a list/table makes the render borrow mutable (its selection state
    // mutates as it draws), matching the native `draw` call.
    if has_sel {
        out.push_str("    terminal.draw_web(move |frame| view(&mut state.borrow_mut(), frame));\n");
    } else {
        out.push_str("    terminal.draw_web(move |frame| view(&state.borrow(), frame));\n");
    }
    out.push_str("    Ok(())\n");
    out.push_str("}\n");
    out
}

/// Run a Screen event in the browser: a sync event runs inline; an async one
/// runs its pre-await body, snapshots the state it needs, and spawns the rest
/// as a browser future (`spawn_local`) whose continuation re-borrows the state
/// when the result arrives. The future captures `rc` — the un-shadowed
/// `Rc<RefCell<_>>` handle cloned at the top of the enclosing closure.
#[allow(clippy::too_many_arguments)]
fn emit_web_event_run(
    ev: &GuiEvent,
    split: Option<&AwaitSplit>,
    indent: usize,
    fields: &HashSet<String>,
    field_ty: &HashMap<String, DeclType>,
    t: &surface::Tables,
    out: &mut String,
    dummy: &mut Diagnostics,
) {
    let pad = "    ".repeat(indent);
    match split {
        Some(s) => {
            surface::emit_event_stmts(
                &s.pre, &ev.params, "state", fields, field_ty, t, indent, dummy, out,
            );
            for snap in &s.snapshots {
                out.push_str(&format!("{}{}\n", pad, snap));
            }
            out.push_str(&format!("{}wasm_bindgen_futures::spawn_local({{\n", pad));
            out.push_str(&format!("{}    let state = rc.clone();\n", pad));
            out.push_str(&format!("{}    async move {{\n", pad));
            out.push_str(&format!("{}        let {} = {}.await;\n", pad, s.bind, s.call_src));
            out.push_str(&format!("{}        let mut guard = state.borrow_mut();\n", pad));
            out.push_str(&format!("{}        let state = &mut *guard;\n", pad));
            surface::emit_event_stmts(
                &s.cont, &ev.params, "state", fields, field_ty, t, indent + 2, dummy, out,
            );
            out.push_str(&format!("{}    }}\n", pad));
            out.push_str(&format!("{}}});\n", pad));
        }
        None => surface::emit_event_stmts(
            &ev.body, &ev.params, "state", fields, field_ty, t, indent, dummy, out,
        ),
    }
}

/// Run a Screen event: an async event (with a split) kicks its work onto a
/// thread and posts the result over the channel; a sync event runs inline. Used
/// by both key handlers and timers.
#[allow(clippy::too_many_arguments)]
fn emit_event_run(
    ev: &GuiEvent,
    split: Option<&AwaitSplit>,
    indent: usize,
    fields: &HashSet<String>,
    field_ty: &HashMap<String, DeclType>,
    t: &surface::Tables,
    out: &mut String,
    dummy: &mut Diagnostics,
) {
    let pad = "    ".repeat(indent);
    let emit_body = |body: &[Stmt], out: &mut String, dummy: &mut Diagnostics| {
        surface::emit_event_stmts(body, &ev.params, "state", fields, field_ty, t, indent, dummy, out);
    };
    match split {
        // Kick-off: pre-await body (main thread), snapshot state, spawn the
        // blocking work, and post the result back over the channel.
        Some(s) => {
            emit_body(&s.pre, out, dummy);
            for snap in &s.snapshots {
                out.push_str(&format!("{}{}\n", pad, snap));
            }
            out.push_str(&format!("{}let tx = tx.clone();\n", pad));
            out.push_str(&format!("{}std::thread::spawn(move || {{\n", pad));
            out.push_str(&format!(
                "{}    let _ = tx.send(Message::{}Done({}));\n",
                pad, ev.name, s.call_src
            ));
            out.push_str(&format!("{}}});\n", pad));
        }
        None => emit_body(&ev.body, out, dummy),
    }
}

/// Up/Down on the focused list/table — direct for one focusable, routed by
/// `focus_index` for several (inputs fall through to `_`). `base` is the
/// indent level of the emitted body (the shells nest differently).
fn nav_dispatch(focusables: &[Focusable], multi: bool, method: &str, base: usize) -> String {
    let pad = "    ".repeat(base);
    let sel: Vec<(usize, &Focusable)> =
        focusables.iter().enumerate().filter(|(_, f)| f.selectable()).collect();
    if !multi {
        return format!("{}state.{}_state.{}();\n", pad, sel[0].1.field, method);
    }
    let mut out = format!("{}match state.focus_index {{\n", pad);
    for (i, f) in &sel {
        out.push_str(&format!("{}    {} => state.{}_state.{}(),\n", pad, i, f.field, method));
    }
    out.push_str(&format!("{}    _ => {{}}\n", pad));
    out.push_str(&format!("{}}}\n", pad));
    out
}

/// Enter on the focused widget → its handler: an Input's `On Submit` (reading the
/// bound field from state), or a List/Table's `On Select` (with the selected row).
/// `base` is the indent level of the emitted body.
#[allow(clippy::too_many_arguments)]
fn enter_dispatch(
    focusables: &[Focusable],
    multi: bool,
    events: &HashMap<String, &GuiEvent>,
    fields: &HashSet<String>,
    field_ty: &HashMap<String, DeclType>,
    t: &surface::Tables,
    base: usize,
) -> String {
    let body = |fo: &Focusable, indent: usize| -> String {
        let pad = "    ".repeat(indent);
        let mut s = String::new();
        let Some(ev) = fo.handler.as_ref().and_then(|h| events.get(&h.to_ascii_lowercase())) else {
            return s;
        };
        let mut dummy = Diagnostics::new();
        let mut emit_body = |s: &mut String, extra: usize| {
            surface::emit_event_stmts(&ev.body, &ev.params, "state", fields, field_ty, t, indent + extra, &mut dummy, s);
        };
        if fo.is_input() {
            // Submit: bind the handler's parameter to a clone of the typed text
            // (so `list.Push(text)` moves the local, not the state field).
            if let Some(p) = ev.params.first() {
                s.push_str(&format!("{}let {} = state.{}.clone();\n", pad, rust_name(&p.name), fo.field));
            }
            emit_body(&mut s, 0);
        } else {
            s.push_str(&format!("{}if let Some(i) = state.{}_state.selected() {{\n", pad, fo.field));
            match ev.params.first() {
                Some(p) => s.push_str(&format!(
                    "{}    let {} = state.{}[i].clone();\n",
                    pad,
                    rust_name(&p.name),
                    fo.field
                )),
                None => s.push_str(&format!("{}    let _ = i;\n", pad)),
            }
            emit_body(&mut s, 1);
            s.push_str(&format!("{}}}\n", pad));
        }
        s
    };
    if !multi {
        return body(&focusables[0], base);
    }
    let pad = "    ".repeat(base);
    let mut out = format!("{}match state.focus_index {{\n", pad);
    for (i, fo) in focusables.iter().enumerate() {
        let arm = body(fo, base + 2);
        if arm.is_empty() {
            continue;
        }
        out.push_str(&format!("{}    {} => {{\n", pad, i));
        out.push_str(&arm);
        out.push_str(&format!("{}    }}\n", pad));
    }
    out.push_str(&format!("{}    _ => {{}}\n", pad));
    out.push_str(&format!("{}}}\n", pad));
    out
}

/// A key that edits the focused input (Backspace/typing) → `action(field)` on the
/// focused input; direct for one focusable, routed by `focus_index` otherwise.
/// `base` is the indent level of the emitted body.
fn input_dispatch(
    focusables: &[Focusable],
    multi: bool,
    action: &dyn Fn(&str) -> String,
    base: usize,
) -> String {
    let pad = "    ".repeat(base);
    let inputs: Vec<(usize, &Focusable)> =
        focusables.iter().enumerate().filter(|(_, f)| f.is_input()).collect();
    if !multi {
        return format!("{}{}\n", pad, action(&inputs[0].1.field));
    }
    let mut out = format!("{}match state.focus_index {{\n", pad);
    for (i, f) in &inputs {
        out.push_str(&format!("{}    {} => {{ {} }}\n", pad, i, action(&f.field)));
    }
    out.push_str(&format!("{}    _ => {{}}\n", pad));
    out.push_str(&format!("{}}}\n", pad));
    out
}

/// A key spec → the matching `KeyCode` pattern.
fn key_pattern(key: &str) -> String {
    // A single character → `KeyCode::Char('x')`.
    let chars: Vec<char> = key.chars().collect();
    if chars.len() == 1 {
        return format!("KeyCode::Char({:?})", chars[0]);
    }
    // A named key.
    match key.to_ascii_lowercase().as_str() {
        "up" => "KeyCode::Up".to_string(),
        "down" => "KeyCode::Down".to_string(),
        "left" => "KeyCode::Left".to_string(),
        "right" => "KeyCode::Right".to_string(),
        "enter" => "KeyCode::Enter".to_string(),
        "esc" | "escape" => "KeyCode::Esc".to_string(),
        "tab" => "KeyCode::Tab".to_string(),
        "space" => "KeyCode::Char(' ')".to_string(),
        "backspace" => "KeyCode::Backspace".to_string(),
        // Fallback: treat the first char as the key.
        _ => format!("KeyCode::Char({:?})", chars.first().copied().unwrap_or(' ')),
    }
}

/// A `Text` node → the `Paragraph` content: a literal as-is, a concatenation as
/// its `format!`, anything else stringified. State fields become `state.field`.
fn text_content(e: &Expr, fields: &HashSet<String>, enums: &HashSet<String>) -> String {
    let rewritten = rewrite_expr(e.clone(), fields, enums);
    match e {
        Expr::Str(_) => render_expr(&rewritten, None),
        Expr::Binary { op: BinOp::Concat, .. } => render_expr(&rewritten, None),
        _ => format!("format!(\"{{}}\", {})", render_expr(&rewritten, None)),
    }
}
