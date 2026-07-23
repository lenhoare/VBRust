//! The form designer's code generator: a widget tree (built visually in the
//! frontend, sent over as JSON) → clean VBR `View` code. One direction only —
//! the design is the source, the VBR is a read-only artifact you paste into a
//! `Window` file, exactly as the IDE's Rust pane is a read-only artifact of your
//! VBR. Kept here (not the frontend) so the emitted syntax is unit-tested.

use serde::Deserialize;

/// One node in the form tree: a widget kind, its properties, and (for
/// containers) its children.
#[derive(Debug, Clone, Deserialize)]
pub struct Node {
    pub kind: String,
    #[serde(default)]
    pub props: NodeProps,
    #[serde(default)]
    pub children: Vec<Node>,
}

/// Everything a widget might carry. All optional — each `kind` reads the few it
/// cares about.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct NodeProps {
    /// Label / placeholder / button caption / image path / rule orientation.
    #[serde(default)]
    pub text: Option<String>,
    /// Bound state field (inputs, checkbox, slider…).
    #[serde(default)]
    pub field: Option<String>,
    /// Event handler name (On Click / On Input / …).
    #[serde(default)]
    pub event: Option<String>,
    /// Child sizing line emitted *before* this node: "Fill", "Fill 2", "Length 40".
    #[serde(default)]
    pub width: Option<String>,
    #[serde(default)]
    pub spacing: Option<u32>,
    #[serde(default)]
    pub padding: Option<u32>,
    /// Canvas reference name.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub w: Option<u32>,
    #[serde(default)]
    pub h: Option<u32>,
    #[serde(default)]
    pub min: Option<i64>,
    #[serde(default)]
    pub max: Option<i64>,
}

/// A VBR string literal: wrap in quotes, doubling any embedded quote (VB's
/// escaping — never a backslash).
fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn indent(depth: usize) -> String {
    "    ".repeat(depth)
}

/// Generate a complete, runnable `Window` for a form tree: an inferred `State`
/// (a typed field per bound control), the `View`, `Event` stubs for each
/// interactive control, and a `Function Main` that runs it. A bare `View`
/// can't stand alone — it must live in a `Window` — so this is the whole file.
pub fn design_to_vbr(root: &Node, name: &str) -> String {
    let mut fields: Vec<(String, String, String)> = Vec::new(); // (field, type, default)
    let mut events: Vec<(String, String, String)> = Vec::new(); // (event, kind, field)
    collect(root, &mut fields, &mut events);

    let mut out = String::new();
    out.push_str(&format!("Window {name}\n"));
    out.push_str(&format!("    Title {}\n\n", quote(name)));

    out.push_str("    State\n");
    if fields.is_empty() {
        out.push_str("        ' add fields your controls bind to\n");
    } else {
        for (f, ty, def) in &fields {
            out.push_str(&format!("        Dim {f} As {ty} = {def}\n"));
        }
    }
    out.push_str("    End State\n\n");

    out.push_str("    View\n");
    emit(root, 2, &mut out);
    out.push_str("    End View\n");

    for (ev, kind, field) in &events {
        out.push('\n');
        out.push_str(&event_stub(kind, ev, field));
    }
    out.push_str("End Window\n\n");

    out.push_str(&format!(
        "' In a multi-file project, move Main to your entry file and call `{name}.Run` there.\n"
    ));
    out.push_str("Function Main()\n");
    out.push_str(&format!("    {name}.Run\n"));
    out.push_str("End Function\n");
    out
}

/// The VB type + default value a control's bound field needs in `State`.
fn field_type(kind: &str) -> Option<(&'static str, &'static str)> {
    match kind {
        "TextInput" | "Text" => Some(("String", "\"\"")),
        "TextArea" => Some(("TextArea", "\"\"")),
        "Checkbox" | "Toggler" => Some(("Boolean", "False")),
        "Slider" | "ProgressBar" => Some(("Integer", "0")),
        _ => None,
    }
}

/// An `Event` handler stub for an interactive control — the payload events write
/// their new value straight back to the bound field.
fn event_stub(kind: &str, name: &str, field: &str) -> String {
    match kind {
        "Button" => format!("    Event {name}()\n        ' TODO: handle the click\n    End Event\n"),
        "TextInput" => {
            format!("    Event {name}(value As String)\n        {field} = value\n    End Event\n")
        }
        "Checkbox" | "Toggler" => {
            format!("    Event {name}(value As Boolean)\n        {field} = value\n    End Event\n")
        }
        "Slider" => {
            format!("    Event {name}(value As Integer)\n        {field} = value\n    End Event\n")
        }
        _ => String::new(),
    }
}

/// Walk the tree collecting the state fields and events the Window needs.
/// Duplicates (by name) are kept once — the frontend assigns unique names.
fn collect(
    node: &Node,
    fields: &mut Vec<(String, String, String)>,
    events: &mut Vec<(String, String, String)>,
) {
    if let Some(f) = &node.props.field {
        if let Some((ty, def)) = field_type(&node.kind) {
            if !fields.iter().any(|(n, _, _)| n == f) {
                fields.push((f.clone(), ty.to_string(), def.to_string()));
            }
        }
    }
    if matches!(
        node.kind.as_str(),
        "Button" | "TextInput" | "Checkbox" | "Toggler" | "Slider"
    ) {
        let name = node.props.event.clone().unwrap_or_else(|| "Handler".to_string());
        if !events.iter().any(|(n, _, _)| n == &name) {
            let field = node.props.field.clone().unwrap_or_else(|| "field".to_string());
            events.push((name, node.kind.clone(), field));
        }
    }
    for c in &node.children {
        collect(c, fields, events);
    }
}

fn emit(node: &Node, depth: usize, out: &mut String) {
    let i = indent(depth);
    let p = &node.props;
    let label = p.text.clone().unwrap_or_default();
    let field = p.field.clone().unwrap_or_else(|| "field".to_string());
    let event = |fallback: &str| p.event.clone().unwrap_or_else(|| fallback.to_string());

    match node.kind.as_str() {
        "Column" | "Row" => {
            out.push_str(&format!("{i}{}\n", node.kind));
            if let Some(s) = p.spacing {
                out.push_str(&format!("{i}    Spacing {s}\n"));
            }
            if let Some(pad) = p.padding {
                out.push_str(&format!("{i}    Padding {pad}\n"));
            }
            for child in &node.children {
                // A child's `width` becomes a main-axis sizing line before it.
                if let Some(w) = &child.props.width {
                    out.push_str(&format!("{i}    {w}\n"));
                }
                emit(child, depth + 1, out);
            }
            out.push_str(&format!("{i}End {}\n", node.kind));
        }
        "Text" => {
            // A bound field shows a live value; otherwise a literal.
            match &p.field {
                Some(f) => out.push_str(&format!("{i}Text {f}\n")),
                None => out.push_str(&format!("{i}Text {}\n", quote(&label))),
            }
        }
        "Button" => {
            out.push_str(&format!("{i}Button {}\n", quote(&label)));
            out.push_str(&format!("{i}    On Click {}\n", event("Clicked")));
            out.push_str(&format!("{i}End Button\n"));
        }
        "TextInput" => {
            out.push_str(&format!("{i}TextInput {}, {field}\n", quote(&label)));
            out.push_str(&format!("{i}    On Input {}\n", event("Typed")));
            out.push_str(&format!("{i}End TextInput\n"));
        }
        "TextArea" => {
            out.push_str(&format!("{i}TextArea {field}\n"));
        }
        "Checkbox" => {
            out.push_str(&format!("{i}Checkbox {}, {field}\n", quote(&label)));
            out.push_str(&format!("{i}    On Toggle {}\n", event("Toggled")));
            out.push_str(&format!("{i}End Checkbox\n"));
        }
        "Toggler" => {
            out.push_str(&format!("{i}Toggler {}, {field}\n", quote(&label)));
            out.push_str(&format!("{i}    On Toggle {}\n", event("Toggled")));
            out.push_str(&format!("{i}End Toggler\n"));
        }
        "Slider" => {
            let min = p.min.unwrap_or(0);
            let max = p.max.unwrap_or(100);
            out.push_str(&format!("{i}Slider {min}..={max}, {field}\n"));
            out.push_str(&format!("{i}    On Change {}\n", event("Changed")));
            out.push_str(&format!("{i}End Slider\n"));
        }
        "Image" => {
            out.push_str(&format!("{i}Image {}\n", quote(&label)));
        }
        "Space" => {
            out.push_str(&format!("{i}Space Height {}\n", p.h.unwrap_or(20)));
        }
        // The GUI's progress/level widget: a range and a bound numeric field.
        // (List/Chart/Sparkline/Gauge etc. are Screen-only — a Window draws
        // charts on a Canvas.)
        "ProgressBar" => {
            let min = p.min.unwrap_or(0);
            let max = p.max.unwrap_or(100);
            out.push_str(&format!("{i}ProgressBar {min}..={max}, {field}\n"));
        }
        // Canvas (and a "Chart", which in a GUI is drawn on one) references a
        // drawing block you define separately.
        "Canvas" | "Chart" => {
            let name = p.name.clone().unwrap_or_else(|| "myCanvas".to_string());
            let w = p.w.unwrap_or(300);
            let h = p.h.unwrap_or(200);
            out.push_str(&format!(
                "{i}' TODO: define a `Canvas {name} … End Canvas` drawing block\n"
            ));
            out.push_str(&format!("{i}Canvas {name} Width {w} Height {h}\n"));
        }
        other => {
            out.push_str(&format!("{i}' unsupported widget: {other}\n"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(kind: &str, children: Vec<Node>) -> Node {
        Node {
            kind: kind.to_string(),
            props: NodeProps::default(),
            children,
        }
    }

    #[test]
    fn emits_a_full_window_with_view_and_main() {
        let out = design_to_vbr(&node("Column", vec![]), "Form1");
        assert!(out.contains("Window Form1\n"), "got:\n{out}");
        assert!(out.contains("Title \"Form1\""), "got:\n{out}");
        assert!(out.contains("    View\n"), "got:\n{out}");
        assert!(out.contains("    End View\n"), "got:\n{out}");
        assert!(out.contains("End Window\n"), "got:\n{out}");
        assert!(out.contains("Function Main()"), "got:\n{out}");
        assert!(out.contains("Form1.Run"), "got:\n{out}");
    }

    #[test]
    fn button_emits_view_widget_and_event_stub() {
        let mut btn = node("Button", vec![]);
        btn.props.text = Some("Save".to_string());
        btn.props.event = Some("SaveClicked".to_string());
        let out = design_to_vbr(&node("Column", vec![btn]), "Form1");
        assert!(out.contains("Button \"Save\""), "got:\n{out}");
        assert!(out.contains("On Click SaveClicked"), "got:\n{out}");
        assert!(out.contains("Event SaveClicked()"), "the handler stub:\n{out}");
    }

    #[test]
    fn state_is_inferred_from_bindings() {
        let mut input = node("TextInput", vec![]);
        input.props.text = Some("Your name".to_string());
        input.props.field = Some("username".to_string());
        input.props.event = Some("Typed".to_string());
        let out = design_to_vbr(&node("Column", vec![input]), "Form1");
        assert!(out.contains("Dim username As String = \"\""), "state field:\n{out}");
        assert!(out.contains("Event Typed(value As String)"), "typed event:\n{out}");
        assert!(out.contains("username = value"), "event writes the field:\n{out}");
    }

    #[test]
    fn child_width_becomes_a_sizing_line() {
        let mut text = node("Text", vec![]);
        text.props.text = Some("Body".to_string());
        text.props.width = Some("Fill 2".to_string());
        let out = design_to_vbr(&node("Column", vec![text]), "Form1");
        let fill_at = out.find("Fill 2").unwrap();
        let text_at = out.find("Text \"Body\"").unwrap();
        assert!(fill_at < text_at, "sizing line must come before the child:\n{out}");
    }

    #[test]
    fn quotes_are_doubled_not_backslashed() {
        let mut btn = node("Button", vec![]);
        btn.props.text = Some("Say \"hi\"".to_string());
        let out = design_to_vbr(&node("Column", vec![btn]), "Form1");
        assert!(out.contains("\"Say \"\"hi\"\"\""), "VB doubles quotes:\n{out}");
        assert!(!out.contains("\\\""), "no backslash escapes:\n{out}");
    }

    #[test]
    fn canvas_drops_in_as_a_reference_with_a_todo() {
        let mut c = node("Canvas", vec![]);
        c.props.name = Some("Face".to_string());
        c.props.w = Some(300);
        c.props.h = Some(220);
        let out = design_to_vbr(&node("Column", vec![c]), "Form1");
        assert!(out.contains("Canvas Face Width 300 Height 220"), "got:\n{out}");
        assert!(out.contains("' TODO"), "should leave a reminder:\n{out}");
    }
}
