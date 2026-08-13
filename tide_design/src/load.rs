//! Load a `.vbt` Screen template into the designer tree.
//!
//! A template is VBR with no extra code: one `Screen`, a `Title`, optional
//! `Menu`, and a `View`. No `Function Main`, no `Event` bodies, no `On Key`.
//! Human-edited `.vbr` programs are refused — that's the can of worms.

use std::path::Path;

use vbr::ast::{
    Expr, ExprKind, MenuEntry, Program, Screen, ScreenMenu, SizeConstraint, TabPane, ViewNode,
};
use vbr::diagnostics::Diagnostics;
use vbr::lexer::lex;
use vbr::parser;

use crate::files::{is_vbr, is_vbt};
use crate::model::{Design, Kind, MenuKind, MenuNode, Node, SizeHint};

pub fn load_template(path: &Path) -> Result<Design, String> {
    if is_vbr(path) {
        return Err(
            "That's a VBR program, not a template. The designer only opens .vbt \
             (File → Save as template) — it won't round-trip event code a human may have edited."
                .into(),
        );
    }
    if !is_vbt(path) {
        return Err("Open a .vbt Screen template (File → Save as template).".into());
    }
    let src = std::fs::read_to_string(path).map_err(|e| format!("Cannot open: {e}"))?;
    let mut design = design_from_source(&src)?;
    design.path = Some(path.to_path_buf());
    design.dirty = false;
    Ok(design)
}

pub fn design_from_source(src: &str) -> Result<Design, String> {
    let mut diags = Diagnostics::new();
    let program = parser::parse(lex(src), &mut diags);
    if diags.has_errors() {
        let msg = diags
            .items()
            .iter()
            .find(|d| matches!(d.level, vbr::diagnostics::Level::Error))
            .map(|d| d.render())
            .unwrap_or_else(|| "Could not parse template.".into());
        return Err(msg);
    }
    template_program(&program)
}

fn template_program(program: &Program) -> Result<Design, String> {
    let extra = extra_code(program);
    if !extra.is_empty() {
        return Err(format!(
            "This isn't a Screen template — it has {extra}. Save a .vbt from the designer \
             (structure only). A .vbr program isn't opened, so we don't have to guess which \
             bits are the View."
        ));
    }
    if program.screens.len() != 1 {
        return Err("A template has exactly one Screen.".into());
    }
    screen_to_design(&program.screens[0])
}

fn extra_code(p: &Program) -> String {
    let mut bits = Vec::new();
    if !p.functions.is_empty() {
        bits.push("Function / Main");
    }
    if !p.windows.is_empty() {
        bits.push("Window");
    }
    if !p.pages.is_empty() {
        bits.push("Page");
    }
    if !p.structs.is_empty() {
        bits.push("Type");
    }
    if !p.enums.is_empty() {
        bits.push("Enum");
    }
    if !p.constants.is_empty() {
        bits.push("Const");
    }
    if !p.uses.is_empty() {
        bits.push("Use");
    }
    if !p.tests.is_empty() {
        bits.push("Test");
    }
    if !p.canvases.is_empty() {
        bits.push("Canvas");
    }
    if !p.css.is_empty() {
        bits.push("Css");
    }
    if !p.godot_nodes.is_empty() {
        bits.push("Node2D");
    }
    if p.screens.len() == 1 {
        let sc = &p.screens[0];
        if sc.status.is_some() {
            bits.push("Status");
        }
        if !sc.keys.is_empty() {
            bits.push("On Key");
        }
        if !sc.timers.is_empty() {
            bits.push("Every / timer");
        }
        if !sc.events.is_empty() {
            bits.push("Event");
        }
        if !sc.subs.is_empty() {
            bits.push("Sub");
        }
        if !sc.state.is_empty() {
            bits.push("State");
        }
    }
    bits.join(", ")
}

fn screen_to_design(sc: &Screen) -> Result<Design, String> {
    let root = view_to_node(&sc.view, SizeHint::Default)?;
    if !matches!(root.kind, Kind::Column | Kind::Row | Kind::Frame | Kind::Tabs) {
        return Err(
            "The View root must be a Column, Row, Frame, or Tabs — that's the structure the \
             designer edits."
                .into(),
        );
    }
    let selected = root.id;
    let menu_root = menu_to_nodes(sc.menu.as_ref())?;
    let menu_selected = menu_root.id;
    Ok(Design {
        screen_name: sc.name.clone(),
        title: sc.title.clone().unwrap_or_else(|| sc.name.clone()),
        root,
        selected,
        menu_root,
        menu_selected,
        dirty: false,
        path: None,
    })
}

fn menu_to_nodes(menu: Option<&ScreenMenu>) -> Result<MenuNode, String> {
    let mut bar = MenuNode::bar();
    let Some(menu) = menu else {
        return Ok(bar);
    };
    for g in &menu.menus {
        let mut m = MenuNode::new(MenuKind::Menu);
        m.text = g.title.clone();
        m.event.clear();
        for it in &g.items {
            match it {
                MenuEntry::Separator => {
                    m.children.push(MenuNode::new(MenuKind::Separator));
                }
                MenuEntry::Item { label, handler } => {
                    let mut item = MenuNode::new(MenuKind::Item);
                    item.text = label.clone();
                    item.event = handler.clone();
                    m.children.push(item);
                }
            }
        }
        bar.children.push(m);
    }
    Ok(bar)
}

fn view_to_node(node: &ViewNode, size: SizeHint) -> Result<Node, String> {
    match node {
        ViewNode::Constrained { size: c, child } => view_to_node(child, size_from(*c)),
        ViewNode::Column {
            children,
            spacing,
            padding,
        } => container(Kind::Column, None, children, spacing, padding, size),
        ViewNode::Row {
            children,
            spacing,
            padding,
        } => container(Kind::Row, None, children, spacing, padding, size),
        ViewNode::Frame {
            title,
            children,
            spacing,
            padding,
        } => {
            let text = match title {
                Some(e) => str_lit(e)?,
                None => String::new(),
            };
            container(Kind::Frame, Some(text), children, spacing, padding, size)
        }
        ViewNode::Tabs {
            field,
            tabs,
            on_change,
        } => {
            let mut n = blank(Kind::Tabs);
            n.field = field.clone();
            n.event = on_change.clone().unwrap_or_default();
            n.size = normalize_size(size, Kind::Tabs);
            for pane in tabs {
                n.children.push(tab_pane(pane)?);
            }
            Ok(n)
        }
        ViewNode::Space {
            horizontal,
            amount,
        } => {
            if *horizontal {
                return Err("A template Space is Height (a Column gap), not Width.".into());
            }
            let mut n = blank(Kind::Space);
            n.size = SizeHint::Length(*amount as u32);
            Ok(n)
        }
        ViewNode::Text(e) => {
            let mut n = blank(Kind::Text);
            n.size = normalize_size(size, Kind::Text);
            match &e.kind {
                ExprKind::Str(s) => n.text = s.clone(),
                ExprKind::Ident(name) => n.field = name.clone(),
                _ => {
                    return Err(
                        "Text in a template must be a string literal or a field name.".into(),
                    )
                }
            }
            Ok(n)
        }
        ViewNode::Button { label, on_click } => {
            let mut n = blank(Kind::Button);
            n.text = str_lit(label)?;
            n.event = on_click.clone().unwrap_or_default();
            n.size = normalize_size(size, Kind::Button);
            Ok(n)
        }
        ViewNode::Checkbox {
            label,
            value,
            on_toggle,
        } => {
            let mut n = blank(Kind::Checkbox);
            n.text = str_lit(label)?;
            n.field = value.clone();
            n.event = on_toggle.clone().unwrap_or_default();
            n.size = normalize_size(size, Kind::Checkbox);
            Ok(n)
        }
        ViewNode::Radio {
            label,
            value,
            option,
            on_select,
        } => {
            let mut n = blank(Kind::Radio);
            n.text = str_lit(label)?;
            n.field = value.clone();
            n.option = option_text(option)?;
            n.event = on_select.clone();
            n.size = normalize_size(size, Kind::Radio);
            Ok(n)
        }
        ViewNode::Input { field, on_submit } => {
            let mut n = blank(Kind::Input);
            n.field = field.clone();
            n.event = on_submit.clone().unwrap_or_default();
            n.size = normalize_size(size, Kind::Input);
            Ok(n)
        }
        ViewNode::Memo { field } => {
            let mut n = blank(Kind::Memo);
            n.field = field.clone();
            n.size = normalize_size(size, Kind::Memo);
            Ok(n)
        }
        ViewNode::List { field, on_select } => {
            let mut n = blank(Kind::List);
            n.field = field.clone();
            n.event = on_select.clone().unwrap_or_default();
            n.size = normalize_size(size, Kind::List);
            Ok(n)
        }
        ViewNode::Table { field, on_select } => {
            let mut n = blank(Kind::Table);
            n.field = field.clone();
            n.event = on_select.clone().unwrap_or_default();
            n.size = normalize_size(size, Kind::Table);
            Ok(n)
        }
        ViewNode::Gauge { value, .. } => {
            let mut n = blank(Kind::Gauge);
            n.field = value.clone();
            n.size = normalize_size(size, Kind::Gauge);
            Ok(n)
        }
        ViewNode::Sparkline { field } => {
            let mut n = blank(Kind::Sparkline);
            n.field = field.clone();
            n.size = normalize_size(size, Kind::Sparkline);
            Ok(n)
        }
        ViewNode::BarChart { field } => {
            let mut n = blank(Kind::BarChart);
            n.field = field.clone();
            n.size = normalize_size(size, Kind::BarChart);
            Ok(n)
        }
        ViewNode::Chart { fields, scatter, x_bounds, y_bounds } => {
            if *scatter || x_bounds.is_some() || y_bounds.is_some() {
                return Err(
                    "A template Chart is the simple `Chart field` form (no Scatter / axes)."
                        .into(),
                );
            }
            if fields.len() != 1 {
                return Err("A template Chart has one series field.".into());
            }
            let mut n = blank(Kind::Chart);
            n.field = fields[0].clone();
            n.size = normalize_size(size, Kind::Chart);
            Ok(n)
        }
        ViewNode::If { .. } | ViewNode::Match { .. } => Err(
            "Templates can't include If / Match in the View — that's logic, not structure."
                .into(),
        ),
        ViewNode::Image { .. }
        | ViewNode::Canvas { .. }
        | ViewNode::TextInput { .. }
        | ViewNode::TextArea { .. }
        | ViewNode::Slider { .. }
        | ViewNode::Toggler { .. }
        | ViewNode::ProgressBar { .. } => Err(
            "That's a Window widget — the designer only edits Screen structure.".into(),
        ),
    }
}

fn tab_pane(pane: &TabPane) -> Result<Node, String> {
    let mut n = blank(Kind::Tab);
    n.text = str_lit(&pane.title)?;
    for child in &pane.children {
        let (inner, size) = unwrap_size(child);
        n.children.push(view_to_node(inner, size)?);
    }
    Ok(n)
}

fn container(
    kind: Kind,
    text: Option<String>,
    children: &[ViewNode],
    spacing: &Option<u16>,
    padding: &Option<u16>,
    size: SizeHint,
) -> Result<Node, String> {
    if spacing.is_some() || padding.is_some() {
        return Err(
            "Templates can't include Spacing / Padding — the designer doesn't store those."
                .into(),
        );
    }
    let mut n = blank(kind);
    if let Some(t) = text {
        n.text = t;
    }
    n.size = normalize_size(size, kind);
    for child in children {
        let (inner, sz) = unwrap_size(child);
        n.children.push(view_to_node(inner, sz)?);
    }
    Ok(n)
}

fn unwrap_size(node: &ViewNode) -> (&ViewNode, SizeHint) {
    match node {
        ViewNode::Constrained { size, child } => (child, size_from(*size)),
        other => (other, SizeHint::Default),
    }
}

fn blank(kind: Kind) -> Node {
    let mut n = Node::new(kind);
    n.event.clear();
    if kind != Kind::Radio {
        n.option.clear();
    }
    n
}

fn size_from(c: SizeConstraint) -> SizeHint {
    match c {
        SizeConstraint::Length(n) => SizeHint::Length(n as u32),
        SizeConstraint::Percent(n) => SizeHint::Percent(n as u32),
        SizeConstraint::Fill(1) => SizeHint::Fill,
        SizeConstraint::Fill(n) => SizeHint::FillN(n as u32),
        SizeConstraint::Min(n) => SizeHint::Min(n as u32),
    }
}

fn normalize_size(size: SizeHint, kind: Kind) -> SizeHint {
    if size == kind.auto_size() {
        SizeHint::Default
    } else {
        size
    }
}

fn str_lit(e: &Expr) -> Result<String, String> {
    match &e.kind {
        ExprKind::Str(s) => Ok(s.clone()),
        _ => Err("Template labels must be string literals.".into()),
    }
}

fn option_text(e: &Expr) -> Result<String, String> {
    match &e.kind {
        ExprKind::Int(i) => Ok(i.to_string()),
        ExprKind::Ident(s) => Ok(s.clone()),
        ExprKind::Field(inner, name) => {
            if let ExprKind::Ident(recv) = &inner.kind {
                Ok(format!("{recv}.{name}"))
            } else {
                Err("Radio option must be a number or a name.".into())
            }
        }
        _ => Err("Radio option must be a number or a name.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emit::{design_to_vbt, design_to_vbr};
    use crate::model::Kind;

    #[test]
    fn roundtrip_memo_and_menu() {
        let mut d = Design::default();
        d.screen_name = "Notes".into();
        d.title = "Notes".into();
        d.selected = d.root.id;
        assert!(d.add_child(Kind::Memo));
        d.menu_selected = d.menu_root.id;
        assert!(d.menu_add(MenuKind::Item));
        if let Some(item) = d.menu_selected_node_mut() {
            item.text = "Quit".into();
            item.event = "Quit".into();
        }
        let src = design_to_vbt(&d);
        assert!(src.contains("Screen Notes\n"), "{src}");
        assert!(src.contains("Memo notes\n"), "{src}");
        assert!(src.contains("Item \"Quit\" Quit\n"), "{src}");
        assert!(!src.contains("Function Main"), "{src}");
        assert!(!src.contains("On Key"), "{src}");
        assert!(!src.contains("End State"), "{src}");

        let loaded = design_from_source(&src).expect(&src);
        assert_eq!(loaded.screen_name, "Notes");
        assert_eq!(loaded.root.children.len(), 2); // default Text + Memo
        assert!(loaded.has_menus());
        let again = design_to_vbt(&loaded);
        assert_eq!(src, again);
    }

    #[test]
    fn refuses_a_program() {
        let mut d = Design::default();
        d.selected = d.root.id;
        d.add_child(Kind::Button);
        let vbr = design_to_vbr(&d);
        let err = design_from_source(&vbr).unwrap_err();
        assert!(err.contains("isn't a Screen template"), "{err}");
        assert!(err.contains("Function") || err.contains("Event") || err.contains("On Key"), "{err}");
    }

    #[test]
    fn refuses_view_if() {
        let src = r#"
Screen App
    Title "App"
    View
        Column
            If True Then
                Text "hi"
            End If
        End Column
    End View
End Screen
"#;
        let err = design_from_source(src).unwrap_err();
        assert!(err.contains("If") || err.contains("logic"), "{err}");
    }

    #[test]
    fn sample_notes_template_opens() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/notes.vbt");
        let d = load_template(&path).expect("notes.vbt");
        assert_eq!(d.screen_name, "Notes");
        assert!(d.has_menus());
        assert!(d.root.children.iter().any(|c| c.kind == Kind::Memo));
    }

    #[test]
    fn all_bundled_templates_open() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates");
        let mut n = 0;
        let mut names = Vec::new();
        for ent in std::fs::read_dir(&dir).expect("templates/") {
            let path = ent.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("vbt") {
                continue;
            }
            n += 1;
            names.push(path.file_name().unwrap().to_string_lossy().into_owned());
            load_template(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        }
        names.sort();
        assert!(
            n >= 20,
            "expected at least 20 templates, found {n}: {names:?}"
        );
    }
}
