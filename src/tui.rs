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

    for sc in &program.screens {
        out.push_str(&emit_screen(sc, &enums, diags));
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
fn emit_screen(sc: &Screen, enums: &HashSet<String>, diags: &mut Diagnostics) -> String {
    let mut out = String::new();
    let ty = &sc.name;
    let field_ty: HashMap<String, DeclType> =
        sc.state.iter().map(|f| (to_snake(&f.name), f.ty.clone())).collect();

    // The View: slice 1 is a Column (or single Text) of Text lines.
    let lines = match collect_lines(&sc.view) {
        Some(l) => l,
        None => {
            diags.error_once(
                "tui-view-shape",
                "A Screen's View currently supports a `Column` of `Text` lines (richer TUI \
                 layout — List, Chart, Row splits — is coming). ",
            );
            Vec::new()
        }
    };

    // ── imports ──
    out.push_str("use ratatui::widgets::{Block, Paragraph};\n");
    out.push_str("use ratatui::text::Line;\n");
    out.push_str("use ratatui::Frame;\n\n");

    // ── State struct ──
    out.push_str(&format!("struct {} {{\n", ty));
    for f in &sc.state {
        out.push_str(&format!("    {}: {},\n", to_snake(&f.name), decltype_rust(&f.ty)));
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
    out.push_str("        }\n    }\n}\n\n");

    // ── view ──
    let fields: HashSet<String> = field_ty.keys().cloned().collect();
    let title = sc.title.clone().unwrap_or_else(|| sc.name.clone());
    // A stateless screen never reads `state` — underscore it so it won't warn.
    let state_param = if sc.state.is_empty() { "_state" } else { "state" };
    out.push_str(&format!("fn view({}: &{}, frame: &mut Frame) {{\n", state_param, ty));
    out.push_str("    let lines: Vec<Line> = vec![\n");
    for e in &lines {
        out.push_str(&format!("        {},\n", render_line(e, &fields, enums)));
    }
    out.push_str("    ];\n");
    out.push_str(&format!(
        "    let block = Block::bordered().title({:?});\n",
        title
    ));
    out.push_str("    frame.render_widget(Paragraph::new(lines).block(block), frame.area());\n");
    out.push_str("}\n");

    out
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

    let mut out = String::new();
    // `state` needs `mut` only if some key runs an event that changes it.
    let mutates = sc.keys.iter().any(|k| events.contains_key(&k.handler.to_ascii_lowercase()));
    let let_state = if mutates { "let mut state" } else { "let state" };
    out.push_str("fn main() -> std::io::Result<()> {\n");
    out.push_str("    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};\n");
    out.push_str(&format!("    {} = {}::default();\n", let_state, ty));
    out.push_str("    let mut terminal = ratatui::init();\n");
    out.push_str("    loop {\n");
    out.push_str("        terminal.draw(|frame| view(&state, frame))?;\n");
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

/// A `Text` node → a `Line::from(...)`: a literal as-is, a concatenation as its
/// `format!`, anything else stringified.
fn render_line(e: &Expr, fields: &HashSet<String>, enums: &HashSet<String>) -> String {
    let rewritten = crate::gui::rewrite_expr(e.clone(), fields, enums);
    let content = match e {
        Expr::Str(_) => render_expr(&rewritten, None),
        Expr::Binary { op: BinOp::Concat, .. } => render_expr(&rewritten, None),
        _ => format!("format!(\"{{}}\", {})", render_expr(&rewritten, None)),
    };
    format!("Line::from({})", content)
}

/// Flatten a slice-1 View into its `Text` lines: a `Column` of `Text`, or a lone
/// `Text`. Anything else → `None` (unsupported for now).
fn collect_lines(view: &ViewNode) -> Option<Vec<Expr>> {
    match view {
        ViewNode::Text(e) => Some(vec![e.clone()]),
        ViewNode::Column { children, .. } | ViewNode::Row { children, .. } => {
            let mut out = Vec::new();
            for c in children {
                match c {
                    ViewNode::Text(e) => out.push(e.clone()),
                    _ => return None,
                }
            }
            Some(out)
        }
        _ => None,
    }
}
