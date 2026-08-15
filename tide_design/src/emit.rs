//! Emit a runnable Bust `Screen` from the designer tree.
//!
//! Adapted from the Screen path in `vbr-ide-core::design` — Screen widgets only.

use crate::model::{Design, Kind, Node};

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn indent(depth: usize) -> String {
    "    ".repeat(depth)
}

pub fn design_to_vbr(design: &Design) -> String {
    let mut fields: Vec<(String, String, Option<String>)> = Vec::new();
    let mut events: Vec<(String, String, String)> = Vec::new();
    collect(&design.root, &mut fields, &mut events);
    collect_menu_events(&design.menu_root, &mut events);

    let need_point = uses_kind(&design.root, Kind::Chart);
    let need_bar = uses_kind(&design.root, Kind::BarChart);
    let need_row = uses_kind(&design.root, Kind::Table);
    let need_list = uses_kind(&design.root, Kind::List);
    let need_spark = uses_kind(&design.root, Kind::Sparkline);

    let mut out = String::new();
    if need_point {
        out.push_str("Type Point\n    Public x As Double\n    Public y As Double\nEnd Type\n\n");
    }
    if need_bar {
        out.push_str("Type Bar\n    Public label As String\n    Public value As Integer\nEnd Type\n\n");
    }
    if need_row {
        out.push_str("Type Row\n    Public col1 As String\n    Public col2 As String\nEnd Type\n\n");
    }
    out.push_str(&format!("Screen {}\n", design.screen_name));
    out.push_str(&format!("    Title {}\n\n", quote(&design.title)));

    emit_menu(design, &mut out);

    out.push_str("    State\n");
    if fields.is_empty() {
        out.push_str("        ' add fields your controls bind to\n");
    } else {
        for (f, ty, def) in &fields {
            match def {
                Some(d) => out.push_str(&format!("        Dim {f} As {ty} = {d}\n")),
                None => out.push_str(&format!("        Dim {f} As {ty}\n")),
            }
        }
    }
    out.push_str("    End State\n\n");

    out.push_str("    View\n");
    emit(&design.root, 2, &mut out);
    out.push_str("    End View\n");

    out.push_str("\n    On Key \"q\" Quit \"quit\"\n");

    for (ev, kind, field) in &events {
        out.push('\n');
        out.push_str(&event_stub(kind, ev, field));
    }
    out.push_str("\n    Event Quit\n    End Event\n");
    out.push_str("End Screen\n\n");

    if need_list {
        out.push_str(SAMPLE_LIST);
    }
    if need_row {
        out.push_str(SAMPLE_ROWS);
    }
    if need_spark {
        out.push_str(SAMPLE_SPARK);
    }
    if need_bar {
        out.push_str(SAMPLE_BARS);
    }
    if need_point {
        out.push_str(SAMPLE_CURVE);
    }

    out.push_str("' Open in TIDE, or Run → Test in tide_design (vbr runproject).\n");
    out.push_str("Function Main()\n");
    out.push_str(&format!("    {}.Run\n", design.screen_name));
    out.push_str("End Function\n");
    out
}

const SAMPLE_LIST: &str = "\
Function SampleList() As Vec<String>
    Dim v As Vec<String>
    v.Push(\"One\")
    v.Push(\"Two\")
    v.Push(\"Three\")
    Return v
End Function\n\n";

const SAMPLE_ROWS: &str = "\
Function SampleRows() As Vec<Row>
    Dim v As Vec<Row>
    v.Push(Row { col1: \"Ada\", col2: \"36\" })
    v.Push(Row { col1: \"Grace\", col2: \"79\" })
    v.Push(Row { col1: \"Bjarne\", col2: \"60\" })
    Return v
End Function\n\n";

const SAMPLE_SPARK: &str = "\
Function SampleSpark() As Vec<Integer>
    Dim v As Vec<Integer>
    v.Push(3)
    v.Push(7)
    v.Push(4)
    v.Push(9)
    v.Push(6)
    v.Push(8)
    v.Push(5)
    Return v
End Function\n\n";

const SAMPLE_BARS: &str = "\
Function SampleBars() As Vec<Bar>
    Dim v As Vec<Bar>
    v.Push(Bar { label: \"A\", value: 12 })
    v.Push(Bar { label: \"B\", value: 19 })
    v.Push(Bar { label: \"C\", value: 8 })
    Return v
End Function\n\n";

const SAMPLE_CURVE: &str = "\
Function SampleCurve() As Vec<Point>
    Dim v As Vec<Point>
    v.Push(Point { x: 0.0, y: 0.0 })
    v.Push(Point { x: 1.0, y: 0.5 })
    v.Push(Point { x: 2.0, y: 2.0 })
    v.Push(Point { x: 3.0, y: 4.5 })
    v.Push(Point { x: 4.0, y: 8.0 })
    Return v
End Function\n\n";

/// Screen structure only — Title, Menu, View. No State, events, keys, or Main.
/// The designer will round-trip this; a human-edited `.vbr` will not.
pub fn design_to_vbt(design: &Design) -> String {
    let mut out = String::new();
    out.push_str("' Bust Screen template — structure only. Open in tide_design.\n");
    out.push_str(&format!("Screen {}\n", design.screen_name));
    out.push_str(&format!("    Title {}\n\n", quote(&design.title)));
    emit_menu(design, &mut out);
    out.push_str("    View\n");
    emit(&design.root, 2, &mut out);
    out.push_str("    End View\n");
    out.push_str("End Screen\n");
    out
}

fn emit_menu(design: &Design, out: &mut String) {
    if !design.has_menus() {
        return;
    }
    out.push_str("    Menu\n");
    for menu in &design.menu_root.children {
        if menu.kind != crate::model::MenuKind::Menu {
            continue;
        }
        let title = if menu.text.is_empty() {
            "File"
        } else {
            &menu.text
        };
        out.push_str(&format!("        Menu {}\n", quote(title)));
        for item in &menu.children {
            match item.kind {
                crate::model::MenuKind::Separator => {
                    out.push_str("            Separator\n");
                }
                crate::model::MenuKind::Item => {
                    let label = if item.text.is_empty() {
                        "Item"
                    } else {
                        &item.text
                    };
                    let ev = if item.event.is_empty() {
                        "DoItem"
                    } else {
                        &item.event
                    };
                    out.push_str(&format!("            Item {} {ev}\n", quote(label)));
                }
                _ => {}
            }
        }
        out.push_str("        End Menu\n");
    }
    out.push_str("    End Menu\n\n");
}

fn field_type(kind: Kind) -> Option<(&'static str, Option<&'static str>)> {
    match kind {
        Kind::Text => Some(("String", Some("\"\""))),
        Kind::Input => Some(("String", Some("\"\""))),
        Kind::Memo => Some(("String", Some("\"\""))),
        Kind::List => Some(("Vec<String>", Some("SampleList()"))),
        Kind::Table => Some(("Vec<Row>", Some("SampleRows()"))),
        Kind::BarChart => Some(("Vec<Bar>", Some("SampleBars()"))),
        Kind::Sparkline => Some(("Vec<Integer>", Some("SampleSpark()"))),
        Kind::Gauge => Some(("Integer", Some("50"))),
        Kind::Chart => Some(("Vec<Point>", Some("SampleCurve()"))),
        Kind::Checkbox => Some(("Boolean", Some("False"))),
        Kind::Radio => Some(("Integer", Some("0"))),
        Kind::Tabs => Some(("Integer", Some("0"))),
        Kind::Column | Kind::Row | Kind::Frame | Kind::Tab | Kind::Space | Kind::Button => None,
    }
}

fn uses_kind(node: &Node, kind: Kind) -> bool {
    node.kind == kind || node.children.iter().any(|c| uses_kind(c, kind))
}

fn event_stub(kind: &str, name: &str, field: &str) -> String {
    match kind {
        "Input" => {
            format!("    Event {name}()\n        ' submitted text is in `{field}`\n    End Event\n")
        }
        "List" | "Table" => {
            format!("    Event {name}()\n        ' TODO: handle selection in `{field}`\n    End Event\n")
        }
        "Button" => {
            format!("    Event {name}()\n    End Event\n")
        }
        "Checkbox" => {
            format!("    Event {name}(value As Boolean)\n        ' bound field `{field}` is already toggled\n    End Event\n")
        }
        "Radio" => {
            format!("    Event {name}(value As Integer)\n        ' bound field `{field}` holds the choice\n    End Event\n")
        }
        "Tabs" => {
            format!("    Event {name}(index As Integer)\n        ' selected tab (0 = first)\n    End Event\n")
        }
        "Menu" => {
            format!("    Event {name}()\n    End Event\n")
        }
        _ => String::new(),
    }
}

fn collect(
    node: &Node,
    fields: &mut Vec<(String, String, Option<String>)>,
    events: &mut Vec<(String, String, String)>,
) {
    if !node.field.is_empty() {
        if let Some((ty, def)) = field_type(node.kind) {
            if !fields.iter().any(|(n, _, _)| n == &node.field) {
                fields.push((node.field.clone(), ty.to_string(), def.map(|d| d.to_string())));
            }
        }
    }
    // Bound Text uses field; literal Text does not need State.
    if node.kind == Kind::Text && node.field.is_empty() {
        // skip
    }

    if matches!(
        node.kind,
        Kind::Input | Kind::List | Kind::Table | Kind::Button | Kind::Checkbox | Kind::Radio | Kind::Tabs
    ) && !node.event.is_empty() {
        if !events.iter().any(|(n, _, _)| n == &node.event) {
            let field = if node.field.is_empty() {
                "field".into()
            } else {
                node.field.clone()
            };
            events.push((node.event.clone(), node.kind.label().to_string(), field));
        }
    }
    for c in &node.children {
        collect(c, fields, events);
    }
}

fn collect_menu_events(node: &crate::model::MenuNode, events: &mut Vec<(String, String, String)>) {
    if node.kind == crate::model::MenuKind::Item
        && !node.event.is_empty()
        && !node.event.eq_ignore_ascii_case("Quit")
        && !events.iter().any(|(n, _, _)| n == &node.event)
    {
        events.push((node.event.clone(), "Menu".into(), String::new()));
    }
    for c in &node.children {
        collect_menu_events(c, events);
    }
}

fn emit(node: &Node, depth: usize, out: &mut String) {
    let i = indent(depth);
    match node.kind {
        Kind::Column | Kind::Row | Kind::Frame => {
            if node.kind == Kind::Frame && !node.text.is_empty() {
                out.push_str(&format!("{i}Frame {}\n", quote(&node.text)));
            } else {
                out.push_str(&format!("{i}{}\n", node.kind.label()));
            }
            for child in &node.children {
                let skip_size = child.kind == Kind::Space
                    && matches!(
                        child.effective_size(),
                        crate::model::SizeHint::Length(_) | crate::model::SizeHint::Default
                    );
                if !skip_size {
                    if let Some(w) = child.effective_size().as_vbr_line() {
                        out.push_str(&format!("{i}    {w}\n"));
                    }
                }
                emit(child, depth + 1, out);
            }
            out.push_str(&format!("{i}End {}\n", node.kind.label()));
        }
        Kind::Tabs => {
            let field = if node.field.is_empty() {
                "tab"
            } else {
                &node.field
            };
            out.push_str(&format!("{i}Tabs {field}\n"));
            if !node.event.is_empty() {
                out.push_str(&format!("{i}    On Change {}\n", node.event));
            }
            for child in &node.children {
                if child.kind == Kind::Tab {
                    emit(child, depth + 1, out);
                } else {
                    out.push_str(&format!("{i}    Tab \"Page\"\n"));
                    emit(child, depth + 2, out);
                    out.push_str(&format!("{i}    End Tab\n"));
                }
            }
            out.push_str(&format!("{i}End Tabs\n"));
        }
        Kind::Tab => {
            let title = if node.text.is_empty() {
                "Page"
            } else {
                &node.text
            };
            out.push_str(&format!("{i}Tab {}\n", quote(title)));
            for child in &node.children {
                let skip_size = child.kind == Kind::Space
                    && matches!(
                        child.effective_size(),
                        crate::model::SizeHint::Length(_) | crate::model::SizeHint::Default
                    );
                if !skip_size {
                    if let Some(w) = child.effective_size().as_vbr_line() {
                        out.push_str(&format!("{i}    {w}\n"));
                    }
                }
                emit(child, depth + 1, out);
            }
            out.push_str(&format!("{i}End Tab\n"));
        }
        Kind::Space => {
            let n = match node.effective_size() {
                crate::model::SizeHint::Length(n) => n,
                _ => 1,
            };
            out.push_str(&format!("{i}Space Height {n}\n"));
        }
        Kind::Button => {
            out.push_str(&format!("{i}Button {}\n", quote(&node.text)));
            if !node.event.is_empty() {
                out.push_str(&format!("{i}    On Click {}\n", node.event));
            }
            out.push_str(&format!("{i}End Button\n"));
        }
        Kind::Checkbox => {
            let field = if node.field.is_empty() {
                "checked"
            } else {
                &node.field
            };
            out.push_str(&format!("{i}Checkbox {}, {field}\n", quote(&node.text)));
            if !node.event.is_empty() {
                out.push_str(&format!("{i}    On Toggle {}\n", node.event));
            }
            out.push_str(&format!("{i}End Checkbox\n"));
        }
        Kind::Radio => {
            let field = if node.field.is_empty() {
                "choice"
            } else {
                &node.field
            };
            let opt = if node.option.is_empty() {
                "0"
            } else {
                &node.option
            };
            out.push_str(&format!("{i}Radio {}, {field}, {opt}\n", quote(&node.text)));
            if !node.event.is_empty() {
                out.push_str(&format!("{i}    On Select {}\n", node.event));
            }
            out.push_str(&format!("{i}End Radio\n"));
        }
        Kind::Text => {
            if !node.field.is_empty() {
                out.push_str(&format!("{i}Text {}\n", node.field));
            } else {
                out.push_str(&format!("{i}Text {}\n", quote(&node.text)));
            }
        }
        Kind::Input => {
            let field = if node.field.is_empty() {
                "input"
            } else {
                &node.field
            };
            out.push_str(&format!("{i}Input {field}\n"));
            if !node.event.is_empty() {
                out.push_str(&format!("{i}    On Submit {}\n", node.event));
            }
            out.push_str(&format!("{i}End Input\n"));
        }
        Kind::Memo => {
            let field = if node.field.is_empty() {
                "notes"
            } else {
                &node.field
            };
            out.push_str(&format!("{i}Memo {field}\n"));
            out.push_str(&format!("{i}End Memo\n"));
        }
        Kind::List => {
            let field = if node.field.is_empty() {
                "items"
            } else {
                &node.field
            };
            out.push_str(&format!("{i}List {field}\n"));
            if !node.event.is_empty() {
                out.push_str(&format!("{i}    On Select {}\n", node.event));
            }
            out.push_str(&format!("{i}End List\n"));
        }
        Kind::Table => {
            let field = if node.field.is_empty() {
                "rows"
            } else {
                &node.field
            };
            out.push_str(&format!("{i}Table {field}\n"));
            if !node.event.is_empty() {
                out.push_str(&format!("{i}    On Select {}\n", node.event));
            }
            out.push_str(&format!("{i}End Table\n"));
        }
        Kind::Gauge => {
            let field = if node.field.is_empty() {
                "level"
            } else {
                &node.field
            };
            out.push_str(&format!("{i}Gauge 0..=100, {field}\n"));
        }
        Kind::Sparkline => {
            let field = if node.field.is_empty() {
                "series"
            } else {
                &node.field
            };
            out.push_str(&format!("{i}Sparkline {field}\n"));
        }
        Kind::BarChart => {
            let field = if node.field.is_empty() {
                "bars"
            } else {
                &node.field
            };
            out.push_str(&format!("{i}BarChart {field}\n"));
        }
        Kind::Chart => {
            let field = if node.field.is_empty() {
                "curve"
            } else {
                &node.field
            };
            out.push_str(&format!("{i}Chart {field}\n"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Design, Kind};

    #[test]
    fn emits_screen_and_main() {
        let mut d = Design::default();
        d.selected = d.root.id;
        d.add_child(Kind::Input);
        let out = design_to_vbr(&d);
        assert!(out.contains("Screen Screen1\n"), "{out}");
        assert!(out.contains("Input input\n"), "{out}");
        assert!(out.contains("On Key \"q\" Quit \"quit\"\n"), "{out}");
        assert!(out.contains("Function Main()"), "{out}");
        assert!(out.contains("Screen1.Run"), "{out}");
        assert!(!out.contains("Window"), "{out}");
    }

    #[test]
    fn emits_frame_space_chart() {
        let mut d = Design::default();
        d.selected = d.root.id;
        assert!(d.add_child(Kind::Frame));
        d.selected = d.root.children.last().unwrap().id;
        assert!(d.add_child(Kind::Space));
        d.selected = d.root.id;
        assert!(d.add_child(Kind::Chart));
        let out = design_to_vbr(&d);
        assert!(out.contains("Type Point\n"), "{out}");
        assert!(out.contains("Frame \"Panel\"\n"), "{out}");
        assert!(out.contains("Space Height 1\n"), "{out}");
        assert!(out.contains("End Frame\n"), "{out}");
        assert!(out.contains("Chart curve\n"), "{out}");
        assert!(out.contains("Dim curve As Vec<Point> = SampleCurve()\n"), "{out}");
        assert!(out.contains("Function SampleCurve()"), "{out}");
    }

    #[test]
    fn emits_button_checkbox_radio() {
        let mut d = Design::default();
        d.selected = d.root.id;
        assert!(d.add_child(Kind::Button));
        d.selected = d.root.id;
        assert!(d.add_child(Kind::Checkbox));
        d.selected = d.root.id;
        assert!(d.add_child(Kind::Radio));
        let out = design_to_vbr(&d);
        assert!(out.contains("Button \"OK\"\n"), "{out}");
        assert!(out.contains("On Click Clicked\n"), "{out}");
        assert!(out.contains("End Button\n"), "{out}");
        assert!(out.contains("Checkbox \"Remember me\", checked\n"), "{out}");
        assert!(out.contains("On Toggle Toggled\n"), "{out}");
        assert!(out.contains("Radio \"Option\", choice, 0\n"), "{out}");
        assert!(out.contains("On Select Picked\n"), "{out}");
        assert!(out.contains("Dim checked As Boolean = False\n"), "{out}");
        assert!(out.contains("Dim choice As Integer = 0\n"), "{out}");
    }

    #[test]
    fn emits_tabs_and_tab() {
        let mut d = Design::default();
        d.selected = d.root.id;
        assert!(d.add_child(Kind::Tabs));
        let out = design_to_vbr(&d);
        assert!(out.contains("Tabs tab\n"), "{out}");
        assert!(out.contains("Tab \"Page\"\n"), "{out}");
        assert!(out.contains("End Tab\n"), "{out}");
        assert!(out.contains("End Tabs\n"), "{out}");
        assert!(out.contains("Dim tab As Integer = 0\n"), "{out}");
        assert!(!out.contains("On Change"), "{out}");
    }

    #[test]
    fn emits_memo() {
        let mut d = Design::default();
        d.selected = d.root.id;
        assert!(d.add_child(Kind::Memo));
        let out = design_to_vbr(&d);
        assert!(out.contains("Memo notes\n"), "{out}");
        assert!(out.contains("End Memo\n"), "{out}");
        assert!(out.contains("Dim notes As String = \"\"\n"), "{out}");
    }

    #[test]
    fn emits_menu_bar() {
        let mut d = Design::default();
        d.menu_selected = d.menu_root.id;
        assert!(d.menu_add(crate::model::MenuKind::Item));
        if let Some(item) = d.menu_selected_node_mut() {
            item.text = "Quit".into();
            item.event = "Quit".into();
        }
        let out = design_to_vbr(&d);
        assert!(out.contains("    Menu\n"), "{out}");
        assert!(out.contains("        Menu \"File\"\n"), "{out}");
        assert!(out.contains("            Item \"Quit\" Quit\n"), "{out}");
        assert!(out.contains("        End Menu\n"), "{out}");
        assert!(out.contains("    End Menu\n"), "{out}");
        assert!(!out.contains("Event Quit()\n"), "{out}");
    }

    #[test]
    fn emits_table_and_barchart_types() {
        let mut d = Design::default();
        d.selected = d.root.id;
        assert!(d.add_child(Kind::Table));
        d.selected = d.root.id;
        assert!(d.add_child(Kind::BarChart));
        d.selected = d.root.id;
        assert!(d.add_child(Kind::List));
        let out = design_to_vbr(&d);
        assert!(out.contains("Type Row\n"), "{out}");
        assert!(out.contains("Type Bar\n"), "{out}");
        assert!(out.contains("Dim rows As Vec<Row> = SampleRows()\n"), "{out}");
        assert!(out.contains("Dim bars As Vec<Bar> = SampleBars()\n"), "{out}");
        assert!(out.contains("Dim items As Vec<String> = SampleList()\n"), "{out}");
    }

    #[test]
    fn emitted_vbr_compiles() {
        let mut d = Design::default();
        d.selected = d.root.id;
        for kind in [
            Kind::Memo,
            Kind::Input,
            Kind::Button,
            Kind::Checkbox,
            Kind::Radio,
            Kind::List,
            Kind::Table,
            Kind::Gauge,
            Kind::Sparkline,
            Kind::BarChart,
            Kind::Chart,
            Kind::Tabs,
        ] {
            d.selected = d.root.id;
            assert!(d.add_child(kind), "{kind:?}");
        }
        let src = design_to_vbr(&d);
        let compiled = vbr::compile(&src);
        assert!(
            !compiled.has_errors,
            "emitted Bust failed to compile:\n{}\n--- source ---\n{src}",
            compiled.diagnostics.join("\n")
        );
    }

    #[test]
    fn emits_template_without_logic() {
        let mut d = Design::default();
        d.selected = d.root.id;
        d.add_child(Kind::Memo);
        let out = design_to_vbt(&d);
        assert!(out.contains("Screen Screen1\n"), "{out}");
        assert!(out.contains("Memo notes\n"), "{out}");
        assert!(!out.contains("Function Main"), "{out}");
        assert!(!out.contains("On Key"), "{out}");
        assert!(!out.contains("End State"), "{out}");
        assert!(!out.contains("Event "), "{out}");
    }
}
