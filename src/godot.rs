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
use crate::resolver;
use crate::surface::{self, rewrite_stmt, Tables};
use crate::transpiler::{collect_mutated, decltype_rust, emit_stmt, render_expr, rust_name};
use std::collections::{HashMap, HashSet};

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
    // Each node's base class + its `I<Base>` interface trait need importing (some
    // are in the prelude too — a redundant `use` of the same item is harmless),
    // as does any Godot singleton class referenced in a body (`Input`). One
    // sorted, de-duplicated list.
    let mut classes: Vec<String> = Vec::new();
    for node in &program.godot_nodes {
        classes.push(node.base.clone());
        classes.push(format!("I{}", node.base));
    }
    classes.extend(referenced_classes(program).into_iter().map(str::to_string));
    classes.sort();
    classes.dedup();
    if !classes.is_empty() {
        out.push_str(&format!("use godot::classes::{{{}}};\n", classes.join(", ")));
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
        emit_node(node, &t, diags, &mut out);
        out.push('\n');
    }
    out
}

/// Emit one node class: the struct (with `#[export]` fields + `base`), then the
/// `#[godot_api] impl I<Base>` with `init` and the lifecycle callbacks.
fn emit_node(node: &GodotNode, t: &Tables, diags: &mut Diagnostics, out: &mut String) {
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

    // The node's member names — bare uses in a body rewrite to `self.<name>` —
    // and their declared types, so the resolver can type-check and coerce a body
    // (its `field_ty`, exactly like a Screen's state fields).
    let fields: HashSet<String> = node.fields.iter().map(|f| rust_name(&f.name)).collect();
    let field_ty: HashMap<String, DeclType> =
        node.fields.iter().map(|f| (rust_name(&f.name), f.ty.clone())).collect();

    for ev in &node.events {
        emit_event(ev, &fields, &field_ty, &t.enums, t, diags, out);
    }

    out.push_str("}\n");

    // --- signals: a second, inherent `#[godot_api] impl` -----------------
    // gdext requires `#[signal]` in the inherent impl, not the trait impl. The
    // typed `self.signals().<name>()` API (used by `Emit`) is generated from
    // these declarations.
    if !node.signals.is_empty() {
        out.push_str(&format!("\n#[godot_api]\nimpl {} {{\n", node.name));
        for sig in &node.signals {
            let params: Vec<String> = sig
                .params
                .iter()
                .map(|p| format!("{}: {}", rust_name(&p.name), decltype_rust(&p.ty)))
                .collect();
            out.push_str("    #[signal]\n");
            out.push_str(&format!("    fn {}({});\n", to_snake(&sig.name), params.join(", ")));
        }
        out.push_str("}\n");
    }
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
    field_ty: &HashMap<String, DeclType>,
    enums: &HashSet<String>,
    t: &Tables,
    diags: &mut Diagnostics,
    out: &mut String,
) {
    let name = to_snake(&ev.name);
    let (sig, rebinds): (String, Vec<String>) = match name.as_str() {
        "ready" | "enter_tree" | "exit_tree" | "draw" => {
            (format!("fn {}(&mut self)", name), vec![])
        }
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
        _ => (format!("fn {}(&mut self)", name), vec![]),
    };

    out.push_str(&format!("    {} {{\n", sig));
    for line in rebinds {
        out.push_str(&format!("        {}\n", line));
    }

    // Resolve first (bare names + `field_ty` in scope, like every other
    // surface): types the arithmetic and applies VB's numeric coercions — an
    // integer literal in a float slot gets its `.0`, so `elapsed = 0` compiles.
    // The Godot-only forms (`Me.Velocity`, `Input`, `Vector2`, `Emit`) resolve to
    // `Unknown` and pass through untouched, ready for the Godot rewrite below.
    let mut body: Vec<Stmt> = ev.body.clone();
    resolver::resolve_event_body(
        &mut body, &ev.params, field_ty, &t.fns, &t.methods, &t.consts, &t.modules,
        &t.interfaces, enums, &t.structs, diags,
    );
    // Then member rewrite (bare `speed` → `self.speed`) and the Godot rewrite
    // (properties / `Input` / `Vector2` / `Emit` → verbatim gdext), then emit.
    let body: Vec<Stmt> =
        body.into_iter().map(|s| godot_stmt(rewrite_stmt(s, "self", fields, enums))).collect();
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

/// PascalCase → snake_case for a Godot method or property name
/// (`GlobalPosition` → `global_position`, `MoveAndSlide` → `move_and_slide`). A
/// `_` is inserted only before an uppercase that follows a lowercase or digit,
/// so an acronym run (`RID`) stays together rather than becoming `r_i_d`.
fn to_snake(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    let chars: Vec<char> = name.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase()
            && i > 0
            && (chars[i - 1].is_ascii_lowercase() || chars[i - 1].is_ascii_digit())
        {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// PascalCase → SCREAMING_SNAKE for a Godot type constant (`Color.CornflowerBlue`
/// → `Color::CORNFLOWER_BLUE`, `Vector2.Up` → `Vector2::UP`).
fn screaming_snake(name: &str) -> String {
    to_snake(name).to_ascii_uppercase()
}

/// The getter for a base-class property `Me.<Name>`. Uniform `get_<snake>`,
/// except the handful of Godot properties whose getter is irregular (a boolean's
/// `is_*`). Setters are uniformly `set_<snake>`, so no companion table.
fn property_getter(name: &str) -> String {
    match name.to_ascii_lowercase().as_str() {
        "visible" => "is_visible".to_string(),
        "insideleaf" | "insidetree" => "is_inside_tree".to_string(),
        _ => format!("get_{}", to_snake(name)),
    }
}

/// A Godot value type usable as a literal/constructor in a body. `Color(r,g,b[,a])`
/// and `Vector2i(x,y)` construct; `Vector2.Up`/`Color.Red` read a constant.
fn value_type(name: &str) -> bool {
    matches!(name, "Vector2" | "Vector2i" | "Vector3" | "Color" | "Rect2" | "Rect2i")
}

/// Construct a Godot value type from a call `Type(args...)`. Components are typed
/// (Vector2 = f32, Vector2i = i32, Color = f32 channels), so each argument is
/// cast; `Color` picks `from_rgb`/`from_rgba` by arity.
fn value_ctor(name: &str, args: &[String]) -> String {
    let cast = |t: &str| -> String {
        args.iter().map(|a| format!("({}) as {}", a, t)).collect::<Vec<_>>().join(", ")
    };
    match name {
        "Vector2" | "Vector3" | "Rect2" => format!("{}::new({})", name, cast("f32")),
        "Vector2i" | "Rect2i" => format!("{}::new({})", name, cast("i32")),
        "Color" if args.len() == 3 => format!("Color::from_rgb({})", cast("f32")),
        "Color" => format!("Color::from_rgba({})", cast("f32")),
        _ => format!("{}::new({})", name, args.join(", ")),
    }
}

/// `Input` methods that need a friendlier name than plain snake_case: the VB-ish
/// `IsPressed("ui_right")` means Godot's *action*-based check.
fn input_method(name: &str) -> String {
    match name.to_ascii_lowercase().as_str() {
        "ispressed" => "is_action_pressed".to_string(),
        "isjustpressed" => "is_action_just_pressed".to_string(),
        "isjustreleased" => "is_action_just_released".to_string(),
        // Snake-case the *original* (cased) name — lowercasing first would erase
        // the word boundaries (`GetAxis` → `get_axis`, not `getaxis`).
        _ => to_snake(name),
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
        // `Me.<Prop> = value` → `{ let __v = value; self.base_mut().set_<prop>(__v); }`.
        // The value is hoisted first because `base_mut()` borrows `self` mutably
        // and the value may read another field.
        Stmt::Assign { target, value, op: None }
            if matches!(&target.kind, ExprKind::Field(recv, _)
                if matches!(&recv.kind, ExprKind::Ident(n) if n == "Me")) =>
        {
            let prop = match &target.kind {
                ExprKind::Field(_, p) => to_snake(p),
                _ => unreachable!(),
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
        // `Emit Sig(a, b)` → hoist the args, then emit — `self.signals()` borrows
        // `self` mutably, so an arg reading a field can't be evaluated inside the
        // call (same reason as a property write).
        Stmt::Expr(Expr { kind: ExprKind::MethodCall { recv, method, args }, .. })
            if matches!(&recv.kind, ExprKind::Ident(n) if n == "__vbr_emit") =>
        {
            let sig = to_snake(&method);
            if args.is_empty() {
                return Stmt::Expr(inline(format!("self.signals().{}().emit()", sig)));
            }
            let lets: Vec<String> = args
                .into_iter()
                .enumerate()
                .map(|(i, a)| format!("let __vbr_a{} = {};", i, render_expr(&godot_expr(a), None)))
                .collect();
            let names: Vec<String> =
                (0..lets.len()).map(|i| format!("__vbr_a{}", i)).collect();
            Stmt::Expr(inline(format!(
                "{{ {} self.signals().{}().emit({}); }}",
                lets.join(" "),
                sig,
                names.join(", ")
            )))
        }
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
        // `Me.<Prop>` (read) → `self.base().get_<prop>()` (any base-class property).
        ExprKind::Field(recv, prop)
            if matches!(&recv.kind, ExprKind::Ident(n) if n == "Me") =>
        {
            inline(format!("self.base().{}()", property_getter(&prop)))
        }
        // `Vector2.Zero` / `Color.Red` → `Vector2::ZERO` / `Color::RED`.
        ExprKind::Field(recv, name)
            if matches!(&recv.kind, ExprKind::Ident(n) if value_type(n)) =>
        {
            let ty = match &recv.kind {
                ExprKind::Ident(n) => n.clone(),
                _ => unreachable!(),
            };
            inline(format!("{}::{}", ty, screaming_snake(&name)))
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
        // `Emit ScoreChanged(10)` (parsed as a call on `__vbr_emit`) →
        // `self.signals().score_changed().emit(10)` — gdext's typed signal API.
        ExprKind::MethodCall { recv, method, args }
            if matches!(&recv.kind, ExprKind::Ident(n) if n == "__vbr_emit") =>
        {
            let rendered: Vec<String> =
                args.into_iter().map(|a| render_expr(&godot_expr(a), None)).collect();
            inline(format!(
                "self.signals().{}().emit({})",
                to_snake(&method),
                rendered.join(", ")
            ))
        }
        // `Me.MoveAndSlide()` (a base-class *method*) → `self.base_mut().move_and_slide()`.
        // A mutable handle serves both `&self` and `&mut self` methods.
        ExprKind::MethodCall { recv, method, args }
            if matches!(&recv.kind, ExprKind::Ident(n) if n == "Me") =>
        {
            let rendered: Vec<String> =
                args.into_iter().map(|a| render_expr(&godot_expr(a), None)).collect();
            inline(format!("self.base_mut().{}({})", to_snake(&method), rendered.join(", ")))
        }
        // `Vector2(x, y)` / `Color(r, g, b)` — construct a Godot value type.
        ExprKind::Call { name, args } if value_type(&name) => {
            let rendered: Vec<String> =
                args.into_iter().map(|a| render_expr(&godot_expr(a), None)).collect();
            inline(value_ctor(&name, &rendered))
        }
        // Unary minus is parsed as `0 - x`; without the resolver's type pass the
        // `0` stays an integer (`0 - f32` won't compile), so fold it into a real
        // negation, which is type-agnostic (`-(f32)`, `-(i64)`).
        ExprKind::Binary { op: BinOp::Sub, lhs, rhs }
            if matches!(&lhs.kind, ExprKind::Int(0)) =>
        {
            inline(format!("-({})", render_expr(&godot_expr(*rhs), None)))
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
        // Wrappers the resolver may insert (a numeric `as` cast, a `&`/`&mut`
        // borrow, a deref) — recurse through so a Godot form tucked inside is
        // still reached.
        ExprKind::Cast(inner, t) => Expr {
            kind: ExprKind::Cast(Box::new(godot_expr(*inner)), t),
            span: e.span,
        },
        ExprKind::Ref(inner) => Expr { kind: ExprKind::Ref(Box::new(godot_expr(*inner))), span: e.span },
        ExprKind::MutRef(inner) => {
            Expr { kind: ExprKind::MutRef(Box::new(godot_expr(*inner))), span: e.span }
        }
        ExprKind::Deref(inner) => {
            Expr { kind: ExprKind::Deref(Box::new(godot_expr(*inner))), span: e.span }
        }
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
