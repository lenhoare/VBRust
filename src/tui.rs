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

use crate::ast::*;
use crate::diagnostics::Diagnostics;
use crate::gui::{coerce_state_strings, render_init, rewrite_stmt};
use crate::resolver;
use crate::transpiler::{
    decltype_rust, emit_const, emit_enum, emit_fn, emit_impl, emit_stmt, note_builtins, render_expr,
    emit_struct, to_snake,
};
use std::collections::{HashMap, HashSet};

/// Emit a complete TUI program: shared items (consts/structs/enums/functions),
/// each screen's definition, then `fn main`, which runs the screen launched by
/// `<Screen>.Run` inside `Function Main()`.
pub fn emit_tui_program(program: &Program, diags: &mut Diagnostics) -> String {
    let mut out = String::new();
    for comment in &program.leading_comments {
        out.push_str(&format!("// {}\n", comment));
    }
    if !program.leading_comments.is_empty() {
        out.push('\n');
    }
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

    // User functions/methods (everything except `Main`), so a screen can call them.
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
    for f in program.functions.iter().filter(|f| f.receiver.is_none() && !is_main(f)) {
        emit_fn(f, &fns, &methods, &consts, &modules, &enums, diags, &mut out, 0, None);
        out.push('\n');
    }

    // Struct definitions by name — a `Table` reads its element struct's fields.
    let structs: HashMap<String, &StructDef> =
        program.structs.iter().map(|s| (s.name.clone(), s)).collect();

    for sc in &program.screens {
        out.push_str(&emit_screen(sc, &enums, &structs, diags));
        out.push('\n');
    }
    match find_launched_screen(program) {
        Some(sc) => out.push_str(&emit_main(sc, &enums)),
        None => diags.error_once(
            "tui-no-launch",
            "A screen is never launched. Add `Function Main()` containing `<Screen>.Run`, \
             e.g. `Counter.Run`.",
        ),
    }
    out
}

/// Find the screen launched by `<Screen>.Run` inside `Function Main()`.
fn find_launched_screen(program: &Program) -> Option<&Screen> {
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
                if let Some(sc) = program.screens.iter().find(|s| s.name.eq_ignore_ascii_case(name)) {
                    return Some(sc);
                }
            }
        }
    }
    None
}

/// Emit one screen: the State struct + Default, the `view` fn, and (later) any
/// helpers. The event loop lives in `fn main` (emitted separately).
fn emit_screen(
    sc: &Screen,
    enums: &HashSet<String>,
    structs: &HashMap<String, &StructDef>,
    diags: &mut Diagnostics,
) -> String {
    let mut out = String::new();
    let ty = &sc.name;
    let field_ty: HashMap<String, DeclType> =
        sc.state.iter().map(|f| (to_snake(&f.name), f.ty.clone())).collect();
    let fields: HashSet<String> = field_ty.keys().cloned().collect();

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

    // ── imports ──
    out.push_str("use ratatui::widgets::{Block, Paragraph};\n");
    out.push_str("use ratatui::layout::{Constraint, Layout};\n");
    out.push_str("use ratatui::Frame;\n\n");

    // ── State struct ──
    out.push_str(&format!("struct {} {{\n", ty));
    for f in &sc.state {
        out.push_str(&format!("    {}: {},\n", to_snake(&f.name), decltype_rust(&f.ty)));
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
            to_snake(&f.name),
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
    // Render the body first (into `inner`, the area within the titled border), so
    // we can name the `state` param `_state` when nothing reads it. A list/table
    // makes the view `&mut` (its state mutates when rendered); an input alone
    // reads state immutably (typing happens in the event loop).
    let title = sc.title.clone().unwrap_or_else(|| sc.name.clone());
    let mut body = String::new();
    let mut counter = 0usize;
    render_view_node(&sc.view, "inner", &fields, &field_ty, enums, structs, &focus_map, multi, &mut counter, 1, &mut body, diags);
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
            out.push(Focusable { field: to_snake(field), handler: handler.clone(), kind });
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
            let f = to_snake(field);
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
            let f = to_snake(field);
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
            let f = to_snake(field);
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
                    let acc = format!("row.{}", to_snake(&c.name));
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
        other => {
            diags.error_once(
                "tui-widget-unsupported",
                format!(
                    "That widget isn't supported in a Screen yet ({}). A Screen supports \
                     Column, Row, Text (with layout sizing), List, and Table; Chart is coming.",
                    tui_node_name(other)
                ),
            );
        }
    }
}

/// The ratatui `Constraint` for a child — its explicit size, or a sensible
/// default (a nested container fills leftover space; a leaf takes one line).
fn child_constraint(node: &ViewNode) -> String {
    match node {
        ViewNode::Constrained { size, .. } => constraint_expr(*size),
        ViewNode::Column { .. } | ViewNode::Row { .. } => "Constraint::Fill(1)".to_string(),
        _ => "Constraint::Length(1)".to_string(),
    }
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
fn emit_main(sc: &Screen, enums: &HashSet<String>) -> String {
    let ty = &sc.name;
    let field_ty: HashMap<String, DeclType> =
        sc.state.iter().map(|f| (to_snake(&f.name), f.ty.clone())).collect();
    let fields: HashSet<String> = field_ty.keys().cloned().collect();
    let events: HashMap<String, &GuiEvent> =
        sc.events.iter().map(|e| (e.name.to_ascii_lowercase(), e)).collect();

    let focusables = collect_focusables(&sc.view);
    let has_focus = !focusables.is_empty();
    let has_sel = focusables.iter().any(Focusable::selectable);
    let has_input = focusables.iter().any(Focusable::is_input);
    let multi = focusables.len() > 1;
    // Keys the user bound explicitly — their bindings win over the built-in
    // navigation (so we skip a built-in arm whose key they've taken).
    let user_keys: HashSet<String> = sc.keys.iter().map(|k| key_pattern(&k.key)).collect();

    let mut out = String::new();
    // `state` needs `mut` if a key runs an event that changes it, or a focusable
    // widget mutates it (typing into an input, moving a list selection).
    let mutates =
        has_focus || sc.keys.iter().any(|k| events.contains_key(&k.handler.to_ascii_lowercase()));
    let let_state = if mutates { "let mut state" } else { "let state" };
    // Only a list/table makes the view `&mut` (its state mutates on render).
    let draw_arg = if has_sel { "&mut state" } else { "&state" };
    out.push_str("fn main() -> std::io::Result<()> {\n");
    out.push_str("    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};\n");
    out.push_str(&format!("    {} = {}::default();\n", let_state, ty));
    out.push_str("    let mut terminal = ratatui::init();\n");
    out.push_str("    loop {\n");
    out.push_str(&format!("        terminal.draw(|frame| view({}, frame))?;\n", draw_arg));
    out.push_str("        if let Event::Key(key) = event::read()? {\n");
    out.push_str("            if key.kind == KeyEventKind::Press {\n");
    out.push_str("                match key.code {\n");
    let mut dummy = Diagnostics::new();
    for k in &sc.keys {
        out.push_str(&format!("                    {} => {{\n", key_pattern(&k.key)));
        if k.handler.eq_ignore_ascii_case("Quit") {
            out.push_str("                        break;\n");
        } else if let Some(ev) = events.get(&k.handler.to_ascii_lowercase()) {
            for stmt in &ev.body {
                let mut rewritten = rewrite_stmt(stmt.clone(), &fields, enums);
                coerce_state_strings(&mut rewritten, &field_ty);
                emit_stmt(&rewritten, &HashSet::new(), &HashSet::new(), 6, &mut dummy, &mut out);
            }
        }
        out.push_str("                    }\n");
    }
    // Built-in focus navigation (only for keys the user hasn't bound). Order
    // matters: specific user `Char('x')` arms above, general `Char(c)` typing last.
    if has_sel && !user_keys.contains("KeyCode::Down") {
        out.push_str("                    KeyCode::Down => {\n");
        out.push_str(&nav_dispatch(&focusables, multi, "select_next"));
        out.push_str("                    }\n");
    }
    if has_sel && !user_keys.contains("KeyCode::Up") {
        out.push_str("                    KeyCode::Up => {\n");
        out.push_str(&nav_dispatch(&focusables, multi, "select_previous"));
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
        out.push_str(&enter_dispatch(&focusables, multi, &events, &fields, &field_ty, enums));
        out.push_str("                    }\n");
    }
    if has_input && !user_keys.contains("KeyCode::Backspace") {
        out.push_str("                    KeyCode::Backspace => {\n");
        out.push_str(&input_dispatch(&focusables, multi, &|f| format!("state.{}.pop();", f)));
        out.push_str("                    }\n");
    }
    if has_input {
        out.push_str("                    KeyCode::Char(c) => {\n");
        out.push_str(&input_dispatch(&focusables, multi, &|f| format!("state.{}.push(c);", f)));
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

/// Up/Down on the focused list/table — direct for one focusable, routed by
/// `focus_index` for several (inputs fall through to `_`).
fn nav_dispatch(focusables: &[Focusable], multi: bool, method: &str) -> String {
    let sel: Vec<(usize, &Focusable)> =
        focusables.iter().enumerate().filter(|(_, f)| f.selectable()).collect();
    if !multi {
        return format!("                        state.{}_state.{}();\n", sel[0].1.field, method);
    }
    let mut out = String::from("                        match state.focus_index {\n");
    for (i, f) in &sel {
        out.push_str(&format!(
            "                            {} => state.{}_state.{}(),\n",
            i, f.field, method
        ));
    }
    out.push_str("                            _ => {}\n");
    out.push_str("                        }\n");
    out
}

/// Enter on the focused widget → its handler: an Input's `On Submit` (reading the
/// bound field from state), or a List/Table's `On Select` (with the selected row).
fn enter_dispatch(
    focusables: &[Focusable],
    multi: bool,
    events: &HashMap<String, &GuiEvent>,
    fields: &HashSet<String>,
    field_ty: &HashMap<String, DeclType>,
    enums: &HashSet<String>,
) -> String {
    let body = |fo: &Focusable, indent: usize| -> String {
        let pad = "    ".repeat(indent);
        let mut s = String::new();
        let Some(ev) = fo.handler.as_ref().and_then(|h| events.get(&h.to_ascii_lowercase())) else {
            return s;
        };
        let mut dummy = Diagnostics::new();
        let mut emit_body = |s: &mut String, extra: usize| {
            for stmt in &ev.body {
                let mut rewritten = rewrite_stmt(stmt.clone(), fields, enums);
                coerce_state_strings(&mut rewritten, field_ty);
                emit_stmt(&rewritten, &HashSet::new(), &HashSet::new(), indent + extra, &mut dummy, s);
            }
        };
        if fo.is_input() {
            // Submit: bind the handler's parameter to a clone of the typed text
            // (so `list.Push(text)` moves the local, not the state field).
            if let Some(p) = ev.params.first() {
                s.push_str(&format!("{}let {} = state.{}.clone();\n", pad, to_snake(&p.name), fo.field));
            }
            emit_body(&mut s, 0);
        } else {
            s.push_str(&format!("{}if let Some(i) = state.{}_state.selected() {{\n", pad, fo.field));
            match ev.params.first() {
                Some(p) => s.push_str(&format!(
                    "{}    let {} = state.{}[i].clone();\n",
                    pad,
                    to_snake(&p.name),
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
        return body(&focusables[0], 6);
    }
    let mut out = String::from("                        match state.focus_index {\n");
    for (i, fo) in focusables.iter().enumerate() {
        let arm = body(fo, 8);
        if arm.is_empty() {
            continue;
        }
        out.push_str(&format!("                            {} => {{\n", i));
        out.push_str(&arm);
        out.push_str("                            }\n");
    }
    out.push_str("                            _ => {}\n");
    out.push_str("                        }\n");
    out
}

/// A key that edits the focused input (Backspace/typing) → `action(field)` on the
/// focused input; direct for one focusable, routed by `focus_index` otherwise.
fn input_dispatch(focusables: &[Focusable], multi: bool, action: &dyn Fn(&str) -> String) -> String {
    let inputs: Vec<(usize, &Focusable)> =
        focusables.iter().enumerate().filter(|(_, f)| f.is_input()).collect();
    if !multi {
        return format!("                        {}\n", action(&inputs[0].1.field));
    }
    let mut out = String::from("                        match state.focus_index {\n");
    for (i, f) in &inputs {
        out.push_str(&format!("                            {} => {{ {} }}\n", i, action(&f.field)));
    }
    out.push_str("                            _ => {}\n");
    out.push_str("                        }\n");
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
    let rewritten = crate::gui::rewrite_expr(e.clone(), fields, enums);
    match e {
        Expr::Str(_) => render_expr(&rewritten, None),
        Expr::Binary { op: BinOp::Concat, .. } => render_expr(&rewritten, None),
        _ => format!("format!(\"{{}}\", {})", render_expr(&rewritten, None)),
    }
}
