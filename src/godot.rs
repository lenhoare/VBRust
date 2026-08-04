//! Godot codegen — a `Node2D` (or other node) block becomes a **gdext**
//! GDExtension class.
//!
//! Unlike the GUI/TUI/Web surfaces (a whole State/View/Events *app*), a Godot
//! node is *one class* that Godot instantiates and drives: inversion of control.
//! So there is no `fn main` — the program compiles to a **cdylib** the Godot
//! editor loads. VBR contributes the behaviour scripts; Godot owns the scene.
//!
//! Slice 1: a `Node2D "Player"` block with `Export` members and `On Ready` /
//! `On Process(delta)` callbacks lowers to
//! `#[derive(GodotClass)] #[class(base = Node2D)]` + `#[godot_api] impl INode2D`.
//! Two lowering rules the gdext borrow checker forces (learned by probe):
//!   * a Godot *class* (`Input`) needs `use godot::classes::…` — the prelude only
//!     carries the core (`Vector2`, `Base`, the `I…` traits, `godot_print!`);
//!   * a base-class **property** assignment (`Me.Position = …`) hoists its value
//!     into a temp first, because `self.base_mut()` borrows `self` mutably and
//!     the value may read another field.
//!
//! Surface rule: inside an event body a **bare name is a member field**
//! (`speed` → `self.speed`, like a `Screen`'s state field), and **`Me.Property`**
//! is a base-class property (`Me.Position`). `Input`/`Vector2` are Godot's.

use crate::ast::*;
use crate::diagnostics::Diagnostics;
use crate::surface::{self, rewrite_stmt};
use crate::transpiler::{collect_mutated, decltype_rust, emit_stmt, render_expr, rust_name};
use std::collections::HashSet;

/// Emit a complete Godot program: shared items (consts/structs/enums/helper
/// functions) plus one gdext class per node, headed by the one-per-crate
/// `ExtensionLibrary` entry stub.
pub fn emit_godot_program(
    program: &Program,
    modules: &[String],
    interfaces: &crate::resolver::ProjectInterfaces,
    is_entry: bool,
    diags: &mut Diagnostics,
) -> String {
    let mut out = String::new();
    out.push_str("use godot::prelude::*;\n");
    // Godot classes referenced in bodies each need a `use godot::classes::…`.
    for class in referenced_classes(program) {
        out.push_str(&format!("use godot::classes::{};\n", class));
    }
    out.push('\n');

    // The gdext entry point — one per crate, only in the entry module.
    if is_entry {
        out.push_str("struct VbrExtension;\n\n");
        out.push_str("#[gdextension]\n");
        out.push_str("unsafe impl ExtensionLibrary for VbrExtension {}\n\n");
    }

    surface::emit_mod_decls(modules, is_entry, &mut out);
    let t = surface::build_tables(program, modules, interfaces);
    surface::emit_shared_items(program, &t, diags, &mut out, &mut |_, _, _| false);

    for node in &program.godot_nodes {
        emit_node(node, diags, &mut out);
        out.push('\n');
    }
    out
}

/// Emit one node class: the struct (with `#[export]` fields + `base`), then the
/// `#[godot_api] impl I<Base>` with `init` and the lifecycle callbacks.
fn emit_node(node: &GodotNode, diags: &mut Diagnostics, out: &mut String) {
    let base = &node.base;
    // --- the struct ------------------------------------------------------
    out.push_str("#[derive(GodotClass)]\n");
    out.push_str(&format!("#[class(base = {})]\n", base));
    out.push_str(&format!("struct {} {{\n", node.name));
    for f in &node.fields {
        if f.export {
            out.push_str("    #[export]\n");
        }
        out.push_str(&format!("    {}: {},\n", rust_name(&f.name), decltype_rust(&f.ty)));
    }
    out.push_str(&format!("    base: Base<{}>,\n", base));
    out.push_str("}\n\n");

    // --- the impl --------------------------------------------------------
    out.push_str("#[godot_api]\n");
    out.push_str(&format!("impl I{} for {} {{\n", base, node.name));

    // init: seed each field from its default (or Default::default()).
    out.push_str(&format!("    fn init(base: Base<{}>) -> Self {{\n", base));
    out.push_str("        Self {\n");
    for f in &node.fields {
        let val = init_value(f);
        out.push_str(&format!("            {}: {},\n", rust_name(&f.name), val));
    }
    out.push_str("            base,\n");
    out.push_str("        }\n");
    out.push_str("    }\n");

    // The node's member names — bare uses in a body rewrite to `self.<name>`.
    let fields: HashSet<String> = node.fields.iter().map(|f| rust_name(&f.name)).collect();
    let enums: HashSet<String> = HashSet::new();

    for ev in &node.events {
        emit_event(ev, &fields, &enums, diags, out);
    }

    out.push_str("}\n");
}

/// A field initialiser for `init`: a literal-ish default rendered in the field's
/// type, or `Default::default()` when none was written.
fn init_value(f: &GodotField) -> String {
    match &f.default {
        Some(e) => match &f.ty {
            DeclType::Plain(t) => render_expr(e, Some(*t)),
            _ => render_expr(e, None),
        },
        None => "Default::default()".to_string(),
    }
}

/// Emit one lifecycle callback as the matching gdext virtual method.
fn emit_event(
    ev: &GodotEvent,
    fields: &HashSet<String>,
    enums: &HashSet<String>,
    diags: &mut Diagnostics,
    out: &mut String,
) {
    let name = ev.name.to_ascii_lowercase();
    let (sig, rebinds): (String, Vec<String>) = match name.as_str() {
        "ready" => ("fn ready(&mut self)".to_string(), vec![]),
        // Godot hands `process`/`physics_process` an f64 delta; VBR sees it in
        // the declared type (default `Single`), so rebind: `let delta = … as f32`.
        "process" | "physics_process" => {
            let p = ev.params.first();
            let pname = p.map(|p| rust_name(&p.name)).unwrap_or_else(|| "delta".to_string());
            let ty = p.map(|p| decltype_rust(&p.ty)).unwrap_or_else(|| "f32".to_string());
            let rebind = if ty == "f64" {
                vec![]
            } else {
                vec![format!("let {} = {} as {};", pname, pname, ty)]
            };
            (format!("fn {}(&mut self, {}: f64)", name, pname), rebind)
        }
        // An unknown event name: emit it snake-cased and let gdext validate that
        // it's a real virtual method (translated back to the .vbr line if not).
        _ => (format!("fn {}(&mut self)", rust_name(&ev.name)), vec![]),
    };

    out.push_str(&format!("    {} {{\n", sig));
    for line in rebinds {
        out.push_str(&format!("        {}\n", line));
    }

    // Member rewrite (bare `speed` → `self.speed`), then the Godot rewrite
    // (properties / `Input` / `Vector2` → verbatim gdext), then emit.
    let body: Vec<Stmt> = ev
        .body
        .iter()
        .cloned()
        .map(|s| godot_stmt(rewrite_stmt(s, "self", fields, enums)))
        .collect();
    let mut mutated: HashSet<String> = HashSet::new();
    collect_mutated(&body, &mut mutated);
    let byref: HashSet<String> = HashSet::new();
    for stmt in &body {
        emit_stmt(stmt, &mutated, &byref, 2, diags, out);
    }
    out.push_str("    }\n");
}

// ---------------------------------------------------------------------------
// The Godot rewrite: turn Godot-specific expressions into verbatim gdext code
// (an `InlineRust` node the emitter passes through), and property assignments
// into a hoisted `set_…`. Kept AST→AST so the normal emitter does the rest.
// ---------------------------------------------------------------------------

/// Base-class transform properties (get_/set_ pairs) recognised on `Me` for
/// slice 1. `Visible` (whose getter is `is_visible`) and the wider set arrive
/// with slice 2's general property rule.
fn base_property(name: &str) -> Option<&'static str> {
    match name.to_ascii_lowercase().as_str() {
        "position" => Some("position"),
        "globalposition" | "global_position" => Some("global_position"),
        "rotation" => Some("rotation"),
        "scale" => Some("scale"),
        "skew" => Some("skew"),
        _ => None,
    }
}

/// `Input` methods that need a friendlier name than plain snake_case: the VB-ish
/// `IsPressed("ui_right")` means Godot's *action*-based check.
fn input_method(name: &str) -> String {
    match name.to_ascii_lowercase().as_str() {
        "ispressed" => "is_action_pressed".to_string(),
        "isjustpressed" => "is_action_just_pressed".to_string(),
        "isjustreleased" => "is_action_just_released".to_string(),
        other => rust_name(other),
    }
}

fn inline(s: String) -> Expr {
    Expr { kind: ExprKind::InlineRust(s), span: crate::span::Span::none() }
}

/// Rewrite one statement. The only structural case is a property assignment
/// (`Me.Position = v`), which becomes a hoisted `set_…` block; everything else
/// just has its expressions Godot-rewritten (recursing into nested bodies).
fn godot_stmt(s: Stmt) -> Stmt {
    match s {
        // `Me.Position = value` → `{ let __v = value; self.base_mut().set_position(__v); }`.
        Stmt::Assign { target, value, op: None }
            if matches!(&target.kind, ExprKind::Field(recv, p)
                if matches!(&recv.kind, ExprKind::Ident(n) if n == "Me") && base_property(p).is_some()) =>
        {
            let prop = if let ExprKind::Field(_, p) = &target.kind {
                base_property(p).unwrap()
            } else {
                unreachable!()
            };
            let v = render_expr(&godot_expr(value), None);
            Stmt::Expr(inline(format!(
                "{{ let __vbr_v = {}; self.base_mut().set_{}(__vbr_v); }}",
                v, prop
            )))
        }
        Stmt::Assign { target, value, op } => Stmt::Assign {
            target: godot_expr(target),
            value: godot_expr(value),
            op,
        },
        Stmt::Dim { name, name_span, ty, init, line } => Stmt::Dim {
            name,
            name_span,
            ty,
            init: init.map(godot_expr),
            line,
        },
        Stmt::Expr(e) => Stmt::Expr(godot_expr(e)),
        Stmt::Print(e) => Stmt::Print(godot_expr(e)),
        Stmt::Return(e) => Stmt::Return(e.map(godot_expr)),
        Stmt::If { branches, else_body } => Stmt::If {
            branches: branches
                .into_iter()
                .map(|(c, b)| (godot_expr(c), b.into_iter().map(godot_stmt).collect()))
                .collect(),
            else_body: else_body.map(|b| b.into_iter().map(godot_stmt).collect()),
        },
        Stmt::For { var, from, to, step, body } => Stmt::For {
            var,
            from: godot_expr(from),
            to: godot_expr(to),
            step: step.map(godot_expr),
            body: body.into_iter().map(godot_stmt).collect(),
        },
        other => other,
    }
}

/// Rewrite one expression, converting the Godot-specific forms to verbatim gdext
/// and recursing through the containers a slice-1 body uses.
fn godot_expr(e: Expr) -> Expr {
    match e.kind {
        // `Me.Position` (read) → `self.base().get_position()`.
        ExprKind::Field(recv, prop)
            if matches!(&recv.kind, ExprKind::Ident(n) if n == "Me")
                && base_property(&prop).is_some() =>
        {
            inline(format!("self.base().get_{}()", base_property(&prop).unwrap()))
        }
        // `Vector2.Zero` → `Vector2::ZERO` (a named constant on a Godot type).
        ExprKind::Field(recv, name)
            if matches!(&recv.kind, ExprKind::Ident(n) if n == "Vector2") =>
        {
            inline(format!("Vector2::{}", name.to_ascii_uppercase()))
        }
        // `Input.IsPressed("ui_right")` → `Input::singleton().is_action_pressed("ui_right")`.
        ExprKind::MethodCall { recv, method, args }
            if matches!(&recv.kind, ExprKind::Ident(n) if n == "Input") =>
        {
            let rendered: Vec<String> =
                args.into_iter().map(|a| render_expr(&godot_expr(a), None)).collect();
            inline(format!(
                "Input::singleton().{}({})",
                input_method(&method),
                rendered.join(", ")
            ))
        }
        // `Vector2(x, y)` → `Vector2::new(x as f32, y as f32)` (components are f32).
        ExprKind::Call { name, args } if name == "Vector2" => {
            let rendered: Vec<String> = args
                .into_iter()
                .map(|a| format!("({}) as f32", render_expr(&godot_expr(a), None)))
                .collect();
            inline(format!("Vector2::new({})", rendered.join(", ")))
        }
        // Recurse through the ordinary containers so a Godot form nested inside
        // arithmetic (`velocity * Speed * delta`) is still reached.
        ExprKind::Binary { op, lhs, rhs } => Expr {
            kind: ExprKind::Binary {
                op,
                lhs: Box::new(godot_expr(*lhs)),
                rhs: Box::new(godot_expr(*rhs)),
            },
            span: e.span,
        },
        ExprKind::Not(inner) => Expr {
            kind: ExprKind::Not(Box::new(godot_expr(*inner))),
            span: e.span,
        },
        ExprKind::Field(recv, name) => Expr {
            kind: ExprKind::Field(Box::new(godot_expr(*recv)), name),
            span: e.span,
        },
        ExprKind::Index(recv, idx) => Expr {
            kind: ExprKind::Index(Box::new(godot_expr(*recv)), Box::new(godot_expr(*idx))),
            span: e.span,
        },
        ExprKind::MethodCall { recv, method, args } => Expr {
            kind: ExprKind::MethodCall {
                recv: Box::new(godot_expr(*recv)),
                method,
                args: args.into_iter().map(godot_expr).collect(),
            },
            span: e.span,
        },
        ExprKind::Call { name, args } => Expr {
            kind: ExprKind::Call { name, args: args.into_iter().map(godot_expr).collect() },
            span: e.span,
        },
        _ => e,
    }
}

/// The Godot classes referenced anywhere in the program's node bodies — each
/// needs its own `use godot::classes::…`. Slice 1 only surfaces `Input`.
fn referenced_classes(program: &Program) -> Vec<&'static str> {
    let mut used: Vec<&'static str> = Vec::new();
    for node in &program.godot_nodes {
        for ev in &node.events {
            if body_uses_input(&ev.body) && !used.contains(&"Input") {
                used.push("Input");
            }
        }
    }
    used
}

fn body_uses_input(stmts: &[Stmt]) -> bool {
    stmts.iter().any(stmt_uses_input)
}

fn stmt_uses_input(s: &Stmt) -> bool {
    match s {
        Stmt::Dim { init, .. } => init.as_ref().is_some_and(expr_uses_input),
        Stmt::Assign { target, value, .. } => expr_uses_input(target) || expr_uses_input(value),
        Stmt::Expr(e) | Stmt::Print(e) => expr_uses_input(e),
        Stmt::Return(e) => e.as_ref().is_some_and(expr_uses_input),
        Stmt::If { branches, else_body } => {
            branches.iter().any(|(c, b)| expr_uses_input(c) || body_uses_input(b))
                || else_body.as_ref().is_some_and(|b| body_uses_input(b))
        }
        Stmt::For { body, .. } => body_uses_input(body),
        _ => false,
    }
}

fn expr_uses_input(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::MethodCall { recv, args, .. } => {
            matches!(&recv.kind, ExprKind::Ident(n) if n == "Input")
                || expr_uses_input(recv)
                || args.iter().any(expr_uses_input)
        }
        ExprKind::Binary { lhs, rhs, .. } => expr_uses_input(lhs) || expr_uses_input(rhs),
        ExprKind::Not(i) | ExprKind::Field(i, _) => expr_uses_input(i),
        ExprKind::Index(a, b) => expr_uses_input(a) || expr_uses_input(b),
        ExprKind::Call { args, .. } => args.iter().any(expr_uses_input),
        _ => false,
    }
}
