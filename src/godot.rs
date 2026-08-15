//! Godot codegen — a `Node2D` (or other node) block becomes a **gdext**
//! GDExtension class.
//!
//! Unlike the GUI/TUI/Web surfaces (a whole State/View/Events *app*), a Godot
//! node is *one class* that Godot instantiates and drives: inversion of control.
//! So there is no `fn main` — the program compiles to a **cdylib** the Godot
//! editor loads. Bust contributes the behaviour scripts; Godot owns the scene.
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
        // Node types fetched via `GetNode` (`Dim h As Label = …`) need importing.
        for ev in &node.events {
            collect_handle_types(&ev.body, &mut classes);
            // `On Input`/`On UnhandledInput` take an `InputEvent` object param.
            if matches!(to_snake(&ev.name).as_str(), "input" | "unhandled_input") {
                classes.push("InputEvent".to_string());
            }
        }
        for h in &node.handlers {
            collect_handle_types(&h.body, &mut classes);
        }
    }
    classes.extend(referenced_classes(program).into_iter().map(str::to_string));
    // A fetched node whose type is one of *this* program's nodes is a local
    // struct, not a `godot::classes` type — don't import it.
    let own: HashSet<&str> = program.godot_nodes.iter().map(|n| n.name.as_str()).collect();
    classes.retain(|c| !own.contains(c.as_str()));
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

    // --- signals + handlers: a second, inherent `#[godot_api] impl` ------
    // gdext requires `#[signal]` and `#[func]` in the inherent impl, not the trait
    // impl. Signals back the typed `self.signals().<name>()` API (used by `Emit`);
    // handlers are the `#[func]`s a `Connect … To` wires a signal to.
    if !node.signals.is_empty() || !node.handlers.is_empty() {
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
        for h in &node.handlers {
            emit_handler(h, &fields, &field_ty, &t.enums, t, diags, out);
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
    // `seed_handles` seeds the body-lowering with object-handle params (the input
    // event), so `event.IsActionPressed(…)` routes to the Godot method name.
    let mut seed_handles: Vec<String> = Vec::new();
    let (sig, rebinds): (String, Vec<String>) = match name.as_str() {
        "ready" | "enter_tree" | "exit_tree" | "draw" => {
            (format!("fn {}(&mut self)", name), vec![])
        }
        // Godot hands `process`/`physics_process` an f64 delta; Bust sees it in
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
        // `On Input(event)` / `On UnhandledInput(event)` — Godot hands an
        // `InputEvent` object. The param is a handle (`event.IsActionPressed(…)`
        // routes to the Godot method), seeded so the rewrite treats it as one.
        "input" | "unhandled_input" => {
            let pname = ev
                .params
                .first()
                .map(|p| rust_name(&p.name))
                .unwrap_or_else(|| "event".to_string());
            seed_handles.push(pname.clone());
            (format!("fn {}(&mut self, {}: Gd<InputEvent>)", name, pname), vec![])
        }
        // An unknown event name: emit it snake-cased and let gdext validate that
        // it's a real virtual method (translated back to the .vbr line if not).
        _ => (format!("fn {}(&mut self)", name), vec![]),
    };

    out.push_str(&format!("    {} {{\n", sig));
    for line in rebinds {
        out.push_str(&format!("        {}\n", line));
    }
    lower_body(&ev.body, &ev.params, &seed_handles, fields, field_ty, enums, t, diags, out);
    out.push_str("    }\n");
}

/// Emit one signal handler as a `#[func]` method (Godot can call it back). The
/// body is lowered exactly like an event body; it goes in the inherent impl.
fn emit_handler(
    h: &GodotEvent,
    fields: &HashSet<String>,
    field_ty: &HashMap<String, DeclType>,
    enums: &HashSet<String>,
    t: &Tables,
    diags: &mut Diagnostics,
    out: &mut String,
) {
    let params: Vec<String> = h
        .params
        .iter()
        .map(|p| format!("{}: {}", rust_name(&p.name), decltype_rust(&p.ty)))
        .collect();
    let sep = if params.is_empty() { "" } else { ", " };
    out.push_str("    #[func]\n");
    out.push_str(&format!("    fn {}(&mut self{}{}) {{\n", to_snake(&h.name), sep, params.join(", ")));
    lower_body(&h.body, &h.params, &[], fields, field_ty, enums, t, diags, out);
    out.push_str("    }\n");
}

/// Lower an event/handler body and emit its statements at 2-indent. Resolve first
/// (bare names + `field_ty` in scope, like every other surface): types the
/// arithmetic and applies VB's numeric coercions — an integer literal in a float
/// slot gets its `.0`, so `elapsed = 0` compiles. The Godot-only forms
/// (`Me.Velocity`, `Input`, `Vector2`, `Emit`, `Connect`) resolve to `Unknown`
/// and pass through, ready for the member rewrite and Godot rewrite below.
fn lower_body(
    body: &[Stmt],
    params: &[Param],
    seed_handles: &[String],
    fields: &HashSet<String>,
    field_ty: &HashMap<String, DeclType>,
    enums: &HashSet<String>,
    t: &Tables,
    diags: &mut Diagnostics,
    out: &mut String,
) {
    let mut body: Vec<Stmt> = body.to_vec();
    resolver::resolve_event_body(
        &mut body, params, field_ty, &t.fns, &t.methods, &t.consts, &t.modules, &t.interfaces,
        enums, &t.structs, diags,
    );
    // Member rewrite (bare `speed` → `self.speed`), so handle detection and the
    // Godot rewrite see the settled form.
    let no_subs = HashSet::new();
    let body: Vec<Stmt> =
        body.into_iter().map(|s| rewrite_stmt(s, "self", fields, enums, &no_subs)).collect();
    // Object handles route their method calls to Godot names: the `On Input`
    // event's `event: Gd<InputEvent>` param (seeded), plus scene-tree handles
    // (`Dim h As T = Me.GetNode(...)` / `Spawn(...)`), which also take `let mut`.
    let mut handles: HashSet<String> = seed_handles.iter().cloned().collect();
    collect_handles(&body, &mut handles);
    let rw = Rw { handles: &handles };
    let body: Vec<Stmt> = body.into_iter().map(|s| rw.stmt(s)).collect();
    let mut mutated: HashSet<String> = HashSet::new();
    collect_mutated(&body, &mut mutated);
    mutated.extend(handles.iter().cloned());
    let byref: HashSet<String> = HashSet::new();
    // Events are a non-fatal sink: catch a propagated error, report it, keep running.
    out.push_str("        {\n");
    out.push_str("            let __vbr_event: Result<(), String> = (|| {\n");
    for stmt in &body {
        emit_stmt(stmt, &mutated, &byref, 3, diags, out);
    }
    out.push_str("                Ok(())\n");
    out.push_str("            })();\n");
    out.push_str("            if let Err(__e) = __vbr_event {\n");
    out.push_str("                eprintln!(\"Error: {}\", __e);\n");
    out.push_str("            }\n");
    out.push_str("        }\n");
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
        "Vector2" | "Vector3" => format!("{}::new({})", name, cast("f32")),
        "Vector2i" => format!("Vector2i::new({})", cast("i32")),
        // gdext `Rect2::new` takes two `Vector2`s; the x/y/w/h form is
        // `from_components`.
        "Rect2" => format!("Rect2::from_components({})", cast("f32")),
        "Rect2i" => format!("Rect2i::from_components({})", cast("i32")),
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

/// A body-lowering pass carrying the node handles in scope — locals bound by
/// `Dim h As T = Me.GetNode(...)`. A method call on a handle (`h.SetText(…)`)
/// routes to the Godot method name (`to_snake`) rather than a user method.
struct Rw<'a> {
    handles: &'a HashSet<String>,
}

impl Rw<'_> {
    /// Rewrite one statement. Structural cases: a base-class property assignment
    /// (hoisted `set_…`), an `Emit` (hoisted signal emit), and a `GetNode`
    /// binding (a typed scene-tree handle); everything else just has its
    /// expressions Godot-rewritten (recursing into nested bodies).
    fn stmt(&self, s: Stmt) -> Stmt {
        match s {
            // `Dim h As T = Me.GetNode("Path")` (a scene-tree handle) or
            // `Dim h As T = Spawn("res://…")` (load + instantiate) → a typed
            // `Gd<T>` handle. Kept a real `Dim` (not an inline-Rust block) so `h`
            // is visible afterwards; the type is rewritten to `Gd<T>`.
            Stmt::Dim { name, name_span, ty, init: Some(init), line, .. }
                if handle_init(&init).is_some() =>
            {
                let (kind, path_expr) = handle_init(&init).unwrap();
                let path = render_expr(&self.expr(path_expr.clone()), None);
                let t = decltype_rust(&ty);
                let rhs = match kind {
                    HandleKind::GetNode => format!("self.base().get_node_as::<{}>({})", t, path),
                    HandleKind::Spawn => {
                        format!("load::<PackedScene>({}).instantiate_as::<{}>()", path, t)
                    }
                };
                Stmt::Dim {
                    name,
                    name_span,
                    ty: DeclType::Named(format!("Gd<{}>", t)),
                    init: Some(inline(rhs)),
                    deferred: false,
                    line,
                }
            }
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
                let v = render_expr(&self.expr(value), None);
                Stmt::Expr(inline(format!(
                    "{{ let __vbr_v = {}; self.base_mut().set_{}(__vbr_v); }}",
                    v, prop
                )))
            }
            Stmt::Assign { target, value, op } => Stmt::Assign {
                target: self.expr(target),
                value: self.expr(value),
                op,
            },
            Stmt::Dim { name, name_span, ty, init, deferred, line } => Stmt::Dim {
                name,
                name_span,
                ty,
                init: init.map(|e| self.expr(e)),
                deferred,
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
                    .map(|(i, a)| format!("let __vbr_a{} = {};", i, render_expr(&self.expr(a), None)))
                    .collect();
                let names: Vec<String> = (0..lets.len()).map(|i| format!("__vbr_a{}", i)).collect();
                Stmt::Expr(inline(format!(
                    "{{ {} self.signals().{}().emit({}); }}",
                    lets.join(" "),
                    sig,
                    names.join(", ")
                )))
            }
            Stmt::Expr(e) => Stmt::Expr(self.expr(e)),
            Stmt::Print(e) => Stmt::Print(self.expr(e)),
            Stmt::Return(e) => Stmt::Return(e.map(|e| self.expr(e))),
            Stmt::RaiseError(e) => Stmt::RaiseError(self.expr(e)),
            Stmt::HandleErr { target, call, err_name, body, line } => Stmt::HandleErr {
                target: target.map(|e| self.expr(e)),
                call: self.expr(call),
                err_name,
                body: body.into_iter().map(|s| self.stmt(s)).collect(),
                line,
            },
            Stmt::If { branches, else_body } => Stmt::If {
                branches: branches
                    .into_iter()
                    .map(|(c, b)| (self.expr(c), b.into_iter().map(|s| self.stmt(s)).collect()))
                    .collect(),
                else_body: else_body.map(|b| b.into_iter().map(|s| self.stmt(s)).collect()),
            },
            Stmt::For { var, from, to, step, body } => Stmt::For {
                var,
                from: self.expr(from),
                to: self.expr(to),
                step: step.map(|e| self.expr(e)),
                body: body.into_iter().map(|s| self.stmt(s)).collect(),
            },
            other => other,
        }
    }

    /// Rewrite one expression, converting the Godot-specific forms to verbatim
    /// gdext and recursing through the ordinary containers.
    fn expr(&self, e: Expr) -> Expr {
        match e.kind {
            // `Connect src.Signal To Handler` (a call on `__vbr_connect`, args =
            // [signal, handler]) → `src.connect("signal", &Callable::from_object_
            // method(&self.to_gd(), "handler"))`. The callable is bound first so
            // `self.to_gd()` (shared borrow) is done before a `self.base_mut()`
            // source (a `Me` self-connect) borrows mutably.
            ExprKind::MethodCall { recv, method, args } if method == "__vbr_connect" => {
                let signal = to_snake(&str_of(args.first()));
                let handler = to_snake(&str_of(args.get(1)));
                let source = match &recv.kind {
                    ExprKind::Ident(n) if n == "Me" => "self.base_mut()".to_string(),
                    ExprKind::Ident(n) => rust_name(n),
                    _ => render_expr(&self.expr(*recv), None),
                };
                inline(format!(
                    "let __vbr_cb = Callable::from_object_method(&self.to_gd(), \"{}\"); \
                     {}.connect(\"{}\", &__vbr_cb)",
                    handler, source, signal
                ))
            }
            // `Me.<Prop>` (read) → `self.base().get_<prop>()` (any base-class property).
            ExprKind::Field(recv, prop) if matches!(&recv.kind, ExprKind::Ident(n) if n == "Me") => {
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
                let rendered = self.render_args(args);
                inline(format!("Input::singleton().{}({})", input_method(&method), rendered))
            }
            // `Emit ScoreChanged(10)` (a call on `__vbr_emit`) →
            // `self.signals().score_changed().emit(10)` — gdext's typed signal API.
            ExprKind::MethodCall { recv, method, args }
                if matches!(&recv.kind, ExprKind::Ident(n) if n == "__vbr_emit") =>
            {
                let rendered = self.render_args(args);
                inline(format!("self.signals().{}().emit({})", to_snake(&method), rendered))
            }
            // `Me.MoveAndSlide()` (a base-class *method*) → `self.base_mut().move_and_slide()`.
            // A mutable handle serves both `&self` and `&mut self` methods.
            ExprKind::MethodCall { recv, method, args }
                if matches!(&recv.kind, ExprKind::Ident(n) if n == "Me") =>
            {
                let rendered = self.render_godot_args(args);
                inline(format!("self.base_mut().{}({})", to_snake(&method), rendered))
            }
            // `handle.SetText("hi")` on a scene-tree handle → `handle.set_text("hi")`
            // (Godot method names are snake_case; a user method would keep its name).
            ExprKind::MethodCall { recv, method, args }
                if matches!(&recv.kind, ExprKind::Ident(n) if self.handles.contains(&rust_name(n))) =>
            {
                let h = match &recv.kind {
                    ExprKind::Ident(n) => rust_name(n),
                    _ => unreachable!(),
                };
                let rendered = self.render_godot_args(args);
                inline(format!("{}.{}({})", h, to_snake(&method), rendered))
            }
            // `Vector2(x, y)` / `Color(r, g, b)` — construct a Godot value type.
            ExprKind::Call { name, args } if value_type(&name) => {
                let rendered: Vec<String> = args.into_iter().map(|a| render_expr(&self.expr(a), None)).collect();
                inline(value_ctor(&name, &rendered))
            }
            // Unary minus is parsed as `0 - x`; if the resolver couldn't type it
            // (an opaque operand), the `0` stays integer (`0 - f32` won't compile),
            // so fold it into a real, type-agnostic negation.
            ExprKind::Binary { op: BinOp::Sub, lhs, rhs } if matches!(&lhs.kind, ExprKind::Int(0)) => {
                inline(format!("-({})", render_expr(&self.expr(*rhs), None)))
            }
            // Recurse through the ordinary containers so a Godot form nested inside
            // arithmetic (`velocity * Speed * delta`) is still reached.
            ExprKind::Binary { op, lhs, rhs } => Expr {
                kind: ExprKind::Binary {
                    op,
                    lhs: Box::new(self.expr(*lhs)),
                    rhs: Box::new(self.expr(*rhs)),
                },
                span: e.span,
            },
            ExprKind::Not(inner) => Expr { kind: ExprKind::Not(Box::new(self.expr(*inner))), span: e.span },
            ExprKind::Field(recv, name) => Expr {
                kind: ExprKind::Field(Box::new(self.expr(*recv)), name),
                span: e.span,
            },
            ExprKind::Index(recv, idx) => Expr {
                kind: ExprKind::Index(Box::new(self.expr(*recv)), Box::new(self.expr(*idx))),
                span: e.span,
            },
            ExprKind::MethodCall { recv, method, args } => Expr {
                kind: ExprKind::MethodCall {
                    recv: Box::new(self.expr(*recv)),
                    method,
                    args: args.into_iter().map(|a| self.expr(a)).collect(),
                },
                span: e.span,
            },
            ExprKind::Call { name, args } => Expr {
                kind: ExprKind::Call { name, args: args.into_iter().map(|a| self.expr(a)).collect() },
                span: e.span,
            },
            // Wrappers the resolver may insert (a numeric `as` cast, a `&`/`&mut`
            // borrow, a deref) — recurse so a Godot form tucked inside is reached.
            ExprKind::Cast(inner, t) => Expr {
                kind: ExprKind::Cast(Box::new(self.expr(*inner)), t),
                span: e.span,
            },
            ExprKind::Ref(inner) => Expr { kind: ExprKind::Ref(Box::new(self.expr(*inner))), span: e.span },
            ExprKind::MutRef(inner) => Expr { kind: ExprKind::MutRef(Box::new(self.expr(*inner))), span: e.span },
            ExprKind::Deref(inner) => Expr { kind: ExprKind::Deref(Box::new(self.expr(*inner))), span: e.span },
            _ => e,
        }
    }

    /// Godot-rewrite and render a call's arguments, comma-joined.
    fn render_args(&self, args: Vec<Expr>) -> String {
        args.into_iter().map(|a| render_expr(&self.expr(a), None)).collect::<Vec<_>>().join(", ")
    }

    /// Like `render_args`, but a concatenation (`"x: " & v` → a `String`) is
    /// **borrowed**: Godot's string-taking methods want `impl AsArg<GString>`,
    /// which a `String` doesn't satisfy but `&String` does. A string *literal*
    /// (`&str`) already satisfies it, so it's passed as-is. (A bare `String`
    /// variable arg still needs a manual `&` for now.)
    fn render_godot_args(&self, args: Vec<Expr>) -> String {
        args.into_iter()
            .map(|a| {
                // A concat (`String`) or a node handle (`Gd<T>`) is borrowed: Godot
                // string params want `impl AsArg<GString>` and node params want
                // `&Gd<Node>` (`AddChild(bullet)` → `add_child(&bullet)`).
                let borrow = matches!(&a.kind, ExprKind::Binary { op: BinOp::Concat, .. })
                    || matches!(&a.kind, ExprKind::Ident(n) if self.handles.contains(&rust_name(n)));
                let r = render_expr(&self.expr(a), None);
                if borrow { format!("&{}", r) } else { r }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The string a `Str` literal carries (empty for anything else / `None`) — used
/// to read a `Connect`'s encoded signal/handler names.
fn str_of(e: Option<&Expr>) -> String {
    match e.map(|e| &e.kind) {
        Some(ExprKind::Str(s)) => s.clone(),
        _ => String::new(),
    }
}

/// How a node handle is produced.
enum HandleKind {
    /// `Me.GetNode("Path")` — an existing node in the tree.
    GetNode,
    /// `Spawn("res://…")` — load a scene and instantiate a fresh node.
    Spawn,
}

/// If `e` fetches (`Me.GetNode("Path")`) or spawns (`Spawn("res://…")`) a node,
/// return how, plus the path/scene argument. Both bind a typed `Gd<T>` handle.
fn handle_init(e: &Expr) -> Option<(HandleKind, &Expr)> {
    match &e.kind {
        ExprKind::MethodCall { recv, method, args }
            if matches!(&recv.kind, ExprKind::Ident(n) if n == "Me")
                && method.eq_ignore_ascii_case("getnode")
                && args.len() == 1 =>
        {
            Some((HandleKind::GetNode, args.first()?))
        }
        ExprKind::Call { name, args } if name.eq_ignore_ascii_case("spawn") && args.len() == 1 => {
            Some((HandleKind::Spawn, args.first()?))
        }
        _ => None,
    }
}

/// Collect the node-handle locals (`Dim h As T = Me.GetNode(...)` / `= Spawn(...)`)
/// declared anywhere in a body, by their `rust_name`.
fn collect_handles(stmts: &[Stmt], out: &mut HashSet<String>) {
    for s in stmts {
        match s {
            Stmt::Dim { name, init: Some(init), .. } if handle_init(init).is_some() => {
                out.insert(rust_name(name));
            }
            Stmt::If { branches, else_body } => {
                for (_, b) in branches {
                    collect_handles(b, out);
                }
                if let Some(b) = else_body {
                    collect_handles(b, out);
                }
            }
            Stmt::For { body, .. } => collect_handles(body, out),
            _ => {}
        }
    }
}

/// The Godot classes a body's handles need imported: each handle's node type
/// (`Dim h As Label = …` → `Label`), plus `PackedScene` when it `Spawn`s.
fn collect_handle_types(stmts: &[Stmt], out: &mut Vec<String>) {
    for s in stmts {
        match s {
            Stmt::Dim { ty, init: Some(init), .. } if handle_init(init).is_some() => {
                out.push(decltype_rust(ty));
                if matches!(handle_init(init), Some((HandleKind::Spawn, _))) {
                    out.push("PackedScene".to_string());
                }
            }
            Stmt::If { branches, else_body } => {
                for (_, b) in branches {
                    collect_handle_types(b, out);
                }
                if let Some(b) = else_body {
                    collect_handle_types(b, out);
                }
            }
            Stmt::For { body, .. } => collect_handle_types(body, out),
            _ => {}
        }
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
