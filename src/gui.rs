//! GUI codegen — a `Window` becomes an Iced 0.13 application.
//!
//! Slice 1: `Window` / `State` / `View` / `Event` → a `State` struct, a `Message`
//! enum, an `update` function, a `view` function, and `main`. The model is The
//! Elm Architecture, which is what Iced is: state is the source of truth, the
//! view is derived from it, and events update it.

use crate::ast::*;
use crate::diagnostics::Diagnostics;
use crate::transpiler::{emit_stmt, render_expr, to_snake};
use std::collections::HashSet;

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
    for w in &program.windows {
        out.push_str(&emit_window(w, diags));
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

/// `fn main` for a GUI: run the window. `iced::run` returns `iced::Result`, so
/// `main` returns it.
fn emit_main(w: &Window) -> String {
    let title = w.title.clone().unwrap_or_else(|| w.name.clone());
    format!(
        "fn main() -> iced::Result {{\n    iced::run({:?}, update, view)\n}}\n",
        title
    )
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
fn emit_window(w: &Window, diags: &mut Diagnostics) -> String {
    let mut out = String::new();
    let ty = &w.name; // the state struct is named after the window
    let fields: HashSet<String> = w.state.iter().map(|f| to_snake(&f.name)).collect();

    // Import only the widgets the view actually uses (no dead imports).
    let mut widgets: Vec<&'static str> = Vec::new();
    collect_widgets(&w.view, &mut widgets);
    widgets.sort();
    out.push_str(&format!("use iced::widget::{{{}}};\n", widgets.join(", ")));
    out.push_str("use iced::Element;\n\n");

    // ── State struct ──
    out.push_str(&format!("struct {} {{\n", ty));
    for f in &w.state {
        out.push_str(&format!("    {}: {},\n", to_snake(&f.name), f.ty.rust()));
    }
    out.push_str("}\n\n");

    // ── Initial state (the Dim initialisers) ──
    out.push_str(&format!("impl Default for {} {{\n    fn default() -> Self {{\n", ty));
    out.push_str(&format!("        {} {{\n", ty));
    for f in &w.state {
        out.push_str(&format!("            {}: {},\n", to_snake(&f.name), render_init(&f.init, f.ty)));
    }
    out.push_str("        }\n    }\n}\n\n");

    // ── Message enum (one variant per event) ──
    out.push_str("#[derive(Debug, Clone)]\nenum Message {\n");
    for e in &w.events {
        out.push_str(&format!("    {},\n", e.name));
    }
    out.push_str("}\n\n");

    // ── update: each event body, with state fields rewritten to `state.field` ──
    let empty: HashSet<String> = HashSet::new();
    out.push_str(&format!("fn update(state: &mut {}, message: Message) {{\n", ty));
    out.push_str("    match message {\n");
    for e in &w.events {
        out.push_str(&format!("        Message::{} => {{\n", e.name));
        for stmt in &e.body {
            let rewritten = rewrite_stmt(stmt.clone(), &fields);
            emit_stmt(&rewritten, &empty, &empty, 3, diags, &mut out);
        }
        out.push_str("        }\n");
    }
    out.push_str("    }\n}\n\n");

    // ── view ──
    out.push_str(&format!("fn view(state: &{}) -> Element<'_, Message> {{\n", ty));
    out.push_str(&format!("    {}\n", render_view(&w.view, &fields, true)));
    out.push_str("}\n");

    out
}

/// A `State` field initialiser: a `String` becomes owned, numbers adapt to type.
fn render_init(init: &Expr, ty: Type) -> String {
    if ty == Type::Text {
        return format!("{}.to_string()", render_expr(init, None));
    }
    render_expr(init, Some(ty))
}

/// Render a view node to an Iced widget expression. The root gets `.into()` so
/// the `view` function returns an `Element`.
fn render_view(node: &ViewNode, fields: &HashSet<String>, root: bool) -> String {
    let s = match node {
        ViewNode::Column(children) => format!("column![{}]", render_children(children, fields)),
        ViewNode::Row(children) => format!("row![{}]", render_children(children, fields)),
        ViewNode::Text(e) => render_text(e, fields),
        ViewNode::Button { label, on_click } => {
            let lbl = render_expr(&rewrite_expr(label.clone(), fields), None);
            match on_click {
                Some(ev) => format!("button({}).on_press(Message::{})", lbl, ev),
                None => format!("button({})", lbl),
            }
        }
    };
    if root {
        format!("{}.into()", s)
    } else {
        s
    }
}

/// Which Iced widget functions the view tree references, for the `use` line.
fn collect_widgets(node: &ViewNode, used: &mut Vec<&'static str>) {
    fn add(used: &mut Vec<&'static str>, w: &'static str) {
        if !used.contains(&w) {
            used.push(w);
        }
    }
    match node {
        ViewNode::Column(children) => {
            add(used, "column");
            children.iter().for_each(|c| collect_widgets(c, used));
        }
        ViewNode::Row(children) => {
            add(used, "row");
            children.iter().for_each(|c| collect_widgets(c, used));
        }
        ViewNode::Text(_) => add(used, "text"),
        ViewNode::Button { .. } => add(used, "button"),
    }
}

fn render_children(children: &[ViewNode], fields: &HashSet<String>) -> String {
    children
        .iter()
        .map(|c| render_view(c, fields, false))
        .collect::<Vec<_>>()
        .join(", ")
}

/// `Text` content: a string literal as-is, a concatenation as its `format!`, and
/// anything else stringified with `format!("{}", …)`.
fn render_text(e: &Expr, fields: &HashSet<String>) -> String {
    let rendered = render_expr(&rewrite_expr(e.clone(), fields), None);
    match e {
        Expr::Str(_) => format!("text({})", rendered),
        Expr::Binary { op: BinOp::Concat, .. } => format!("text({})", rendered),
        _ => format!("text(format!(\"{{}}\", {}))", rendered),
    }
}

/// Replace a bare reference to a state field with `state.field`, so an event /
/// view expression reaches the window's state struct.
fn rewrite_expr(e: Expr, fields: &HashSet<String>) -> Expr {
    match e {
        Expr::Ident(name) if fields.contains(&to_snake(&name)) => {
            Expr::Field(Box::new(Expr::Ident("state".to_string())), name)
        }
        Expr::Binary { op, lhs, rhs } => Expr::Binary {
            op,
            lhs: Box::new(rewrite_expr(*lhs, fields)),
            rhs: Box::new(rewrite_expr(*rhs, fields)),
        },
        Expr::Not(inner) => Expr::Not(Box::new(rewrite_expr(*inner, fields))),
        Expr::Call { name, args } => Expr::Call {
            name,
            args: args.into_iter().map(|a| rewrite_expr(a, fields)).collect(),
        },
        Expr::MethodCall { recv, method, args } => Expr::MethodCall {
            recv: Box::new(rewrite_expr(*recv, fields)),
            method,
            args: args.into_iter().map(|a| rewrite_expr(a, fields)).collect(),
        },
        Expr::Field(inner, f) => Expr::Field(Box::new(rewrite_expr(*inner, fields)), f),
        Expr::Index(a, b) => Expr::Index(
            Box::new(rewrite_expr(*a, fields)),
            Box::new(rewrite_expr(*b, fields)),
        ),
        Expr::Cast(inner, t) => Expr::Cast(Box::new(rewrite_expr(*inner, fields)), t),
        other => other,
    }
}

fn rewrite_stmt(s: Stmt, fields: &HashSet<String>) -> Stmt {
    match s {
        Stmt::Assign { target, value, op } => Stmt::Assign {
            target: rewrite_expr(target, fields),
            value: rewrite_expr(value, fields),
            op,
        },
        Stmt::Print(e) => Stmt::Print(rewrite_expr(e, fields)),
        Stmt::Expr(e) => Stmt::Expr(rewrite_expr(e, fields)),
        Stmt::If { branches, else_body } => Stmt::If {
            branches: branches
                .into_iter()
                .map(|(c, b)| {
                    (
                        rewrite_expr(c, fields),
                        b.into_iter().map(|s| rewrite_stmt(s, fields)).collect(),
                    )
                })
                .collect(),
            else_body: else_body.map(|b| b.into_iter().map(|s| rewrite_stmt(s, fields)).collect()),
        },
        other => other,
    }
}
