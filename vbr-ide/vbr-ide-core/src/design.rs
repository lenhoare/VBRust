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

/// Generate the full `View … End View` block for a form tree.
pub fn design_to_vbr(root: &Node) -> String {
    let mut out = String::from("View\n");
    emit(root, 1, &mut out);
    out.push_str("End View\n");
    out
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
    fn wraps_in_a_view_block() {
        let out = design_to_vbr(&node("Column", vec![]));
        assert!(out.starts_with("View\n"));
        assert!(out.trim_end().ends_with("End View"));
        assert!(out.contains("    Column\n"));
        assert!(out.contains("    End Column\n"));
    }

    #[test]
    fn button_emits_on_click_stub() {
        let mut btn = node("Button", vec![]);
        btn.props.text = Some("Save".to_string());
        btn.props.event = Some("SaveClicked".to_string());
        let out = design_to_vbr(&node("Column", vec![btn]));
        assert!(out.contains("Button \"Save\""), "got:\n{out}");
        assert!(out.contains("On Click SaveClicked"), "got:\n{out}");
        assert!(out.contains("End Button"), "got:\n{out}");
    }

    #[test]
    fn child_width_becomes_a_sizing_line() {
        let mut text = node("Text", vec![]);
        text.props.text = Some("Body".to_string());
        text.props.width = Some("Fill 2".to_string());
        let out = design_to_vbr(&node("Column", vec![text]));
        // The sizing line precedes the child.
        let fill_at = out.find("Fill 2").unwrap();
        let text_at = out.find("Text \"Body\"").unwrap();
        assert!(fill_at < text_at, "sizing line must come before the child:\n{out}");
    }

    #[test]
    fn quotes_are_doubled_not_backslashed() {
        let mut btn = node("Button", vec![]);
        btn.props.text = Some("Say \"hi\"".to_string());
        let out = design_to_vbr(&btn);
        assert!(out.contains("\"Say \"\"hi\"\"\""), "VB doubles quotes:\n{out}");
        assert!(!out.contains("\\\""), "no backslash escapes:\n{out}");
    }

    #[test]
    fn canvas_drops_in_as_a_reference_with_a_todo() {
        let mut c = node("Canvas", vec![]);
        c.props.name = Some("Face".to_string());
        c.props.w = Some(300);
        c.props.h = Some(220);
        let out = design_to_vbr(&node("Column", vec![c]));
        assert!(out.contains("Canvas Face Width 300 Height 220"), "got:\n{out}");
        assert!(out.contains("' TODO"), "should leave a reminder:\n{out}");
    }
}
