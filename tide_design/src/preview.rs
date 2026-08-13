//! Static Ratatui preview of the design tree (not the live Screen runtime).

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Sparkline};
use ratatui::Frame;

use crate::model::{Design, Kind, MenuKind, Node, SizeHint};
use crate::theme::Theme;

/// Hit-test regions painted last frame (node id → screen rect).
pub type HitList = Vec<(usize, Rect)>;

pub fn draw_preview(f: &mut Frame, area: Rect, design: &Design, hits: &mut HitList) {
    hits.clear();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Theme::frame())
        .title(format!(" Preview — {} ", design.title))
        .style(Theme::preview());
    let inner = block.inner(area);
    f.render_widget(block, area);
    // Clear inner
    f.render_widget(Block::default().style(Theme::preview()), inner);
    let body = if design.has_menus() {
        let chunks = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).split(inner);
        paint_menu_bar(f, chunks[0], design);
        chunks[1]
    } else {
        inner
    };
    paint_node(f, body, &design.root, design.selected, hits);
}

fn paint_menu_bar(f: &mut Frame, area: Rect, design: &Design) {
    let mut labels = Vec::new();
    for menu in &design.menu_root.children {
        if menu.kind != MenuKind::Menu {
            continue;
        }
        let title = if menu.text.is_empty() {
            "File"
        } else {
            &menu.text
        };
        let selected = design.menu_selected == menu.id
            || menu.children.iter().any(|c| c.id == design.menu_selected);
        labels.push((format!(" {title} "), selected));
    }
    let spans: Vec<ratatui::text::Span> = labels
        .into_iter()
        .map(|(t, sel)| {
            let style = if sel {
                Theme::highlight()
            } else {
                Style::default()
                    .bg(ratatui::style::Color::Cyan)
                    .fg(ratatui::style::Color::Black)
            };
            ratatui::text::Span::styled(t, style)
        })
        .collect();
    f.render_widget(
        Paragraph::new(ratatui::text::Line::from(spans)).style(
            Style::default()
                .bg(ratatui::style::Color::Cyan)
                .fg(ratatui::style::Color::Black),
        ),
        area,
    );
}

fn paint_node(f: &mut Frame, area: Rect, node: &Node, selected: usize, hits: &mut HitList) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    hits.push((node.id, area));
    let hl = node.id == selected;

    match node.kind {
        Kind::Column => {
            let constraints = child_constraints(&node.children, Direction::Vertical, area.height);
            if constraints.is_empty() {
                paint_placeholder(f, area, "Column", hl);
                return;
            }
            let chunks = Layout::vertical(constraints).split(area);
            for (i, child) in node.children.iter().enumerate() {
                if let Some(c) = chunks.get(i) {
                    paint_node(f, *c, child, selected, hits);
                }
            }
        }
        Kind::Row => {
            let constraints = child_constraints(&node.children, Direction::Horizontal, area.width);
            if constraints.is_empty() {
                paint_placeholder(f, area, "Row", hl);
                return;
            }
            let chunks = Layout::horizontal(constraints).split(area);
            for (i, child) in node.children.iter().enumerate() {
                if let Some(c) = chunks.get(i) {
                    paint_node(f, *c, child, selected, hits);
                }
            }
        }
        Kind::Frame => {
            let style = if hl {
                Theme::highlight()
            } else {
                Theme::preview()
            };
            let title = if node.text.is_empty() {
                " Frame ".into()
            } else {
                format!(" {} ", node.text)
            };
            let block = Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(style)
                .style(Theme::preview());
            let inner = block.inner(area);
            f.render_widget(block, area);
            if node.children.is_empty() {
                paint_placeholder(f, inner, "Frame", hl);
                return;
            }
            let constraints = child_constraints(&node.children, Direction::Vertical, inner.height);
            let chunks = Layout::vertical(constraints).split(inner);
            for (i, child) in node.children.iter().enumerate() {
                if let Some(c) = chunks.get(i) {
                    paint_node(f, *c, child, selected, hits);
                }
            }
        }
        Kind::Tabs => {
            let style = if hl {
                Theme::highlight()
            } else {
                Theme::preview()
            };
            let panes: Vec<&Node> = node
                .children
                .iter()
                .filter(|c| c.kind == Kind::Tab)
                .collect();
            let titles: Vec<String> = panes
                .iter()
                .map(|p| {
                    if p.text.is_empty() {
                        "Page".into()
                    } else {
                        p.text.clone()
                    }
                })
                .collect();
            let bar = if titles.is_empty() {
                "(tabs)".into()
            } else {
                titles.join(" │ ")
            };
            let chunks = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).split(area);
            f.render_widget(Paragraph::new(bar).style(style), chunks[0]);
            let pane = panes
                .iter()
                .copied()
                .find(|p| contains_id(p, selected))
                .or_else(|| panes.first().copied());
            if let Some(tab) = pane {
                if tab.children.is_empty() {
                    paint_placeholder(f, chunks[1], "Tab", hl || tab.id == selected);
                } else {
                    let constraints =
                        child_constraints(&tab.children, Direction::Vertical, chunks[1].height);
                    let inner = Layout::vertical(constraints).split(chunks[1]);
                    for (i, child) in tab.children.iter().enumerate() {
                        if let Some(c) = inner.get(i) {
                            paint_node(f, *c, child, selected, hits);
                        }
                    }
                }
            } else {
                paint_placeholder(f, chunks[1], "Tabs", hl);
            }
        }
        Kind::Tab => {
            if node.children.is_empty() {
                paint_placeholder(f, area, "Tab", hl);
                return;
            }
            let constraints = child_constraints(&node.children, Direction::Vertical, area.height);
            let chunks = Layout::vertical(constraints).split(area);
            for (i, child) in node.children.iter().enumerate() {
                if let Some(c) = chunks.get(i) {
                    paint_node(f, *c, child, selected, hits);
                }
            }
        }
        Kind::Space => {
            if hl {
                f.render_widget(
                    Paragraph::new(" ").style(Theme::highlight()),
                    area,
                );
            }
        }
        Kind::Button => {
            let style = if hl {
                Theme::highlight()
            } else {
                Theme::preview()
            };
            let label = if node.text.is_empty() { "OK" } else { &node.text };
            f.render_widget(Paragraph::new(format!("[ {label} ]")).style(style), area);
        }
        Kind::Checkbox => {
            let style = if hl {
                Theme::highlight()
            } else {
                Theme::preview()
            };
            let label = if node.text.is_empty() {
                "checkbox"
            } else {
                &node.text
            };
            f.render_widget(Paragraph::new(format!("[ ] {label}")).style(style), area);
        }
        Kind::Radio => {
            let style = if hl {
                Theme::highlight()
            } else {
                Theme::preview()
            };
            let label = if node.text.is_empty() {
                "option"
            } else {
                &node.text
            };
            f.render_widget(Paragraph::new(format!("( ) {label}")).style(style), area);
        }
        Kind::Text => {
            let label = if !node.text.is_empty() {
                node.text.clone()
            } else if !node.field.is_empty() {
                format!("{{{}}}", node.field)
            } else {
                "(text)".into()
            };
            let style = if hl {
                Theme::highlight()
            } else {
                Theme::preview()
            };
            f.render_widget(Paragraph::new(label).style(style), area);
        }
        Kind::Input => {
            let style = if hl {
                Theme::highlight()
            } else {
                Theme::preview()
            };
            let field = if node.field.is_empty() {
                "input"
            } else {
                &node.field
            };
            let block = Block::default()
                .borders(Borders::ALL)
                .title(format!(" {field} "))
                .border_style(style)
                .style(style);
            let inner = block.inner(area);
            f.render_widget(block, area);
            f.render_widget(Paragraph::new("________").style(style), inner);
        }
        Kind::Memo => {
            let style = if hl {
                Theme::highlight()
            } else {
                Theme::preview()
            };
            let field = if node.field.is_empty() {
                "notes"
            } else {
                &node.field
            };
            f.render_widget(
                Paragraph::new("line one\nline two\n…")
                    .style(style)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(format!(" {field} "))
                            .border_style(style),
                    ),
                area,
            );
        }
        Kind::List | Kind::Table => {
            let style = if hl {
                Theme::highlight()
            } else {
                Theme::preview()
            };
            let field = if node.field.is_empty() {
                node.kind.label()
            } else {
                &node.field
            };
            let items = [
                ListItem::new(Line::from("Alice")),
                ListItem::new(Line::from("Bob")),
                ListItem::new(Line::from("Charlie")),
            ];
            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!(" {field} "))
                        .border_style(style),
                )
                .style(style);
            f.render_widget(list, area);
        }
        Kind::Gauge => {
            let style = if hl {
                Theme::highlight()
            } else {
                Theme::preview()
            };
            let g = Gauge::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" gauge ")
                        .border_style(style),
                )
                .gauge_style(Style::default().fg(ratatui::style::Color::Cyan))
                .percent(42);
            f.render_widget(g, area);
        }
        Kind::Sparkline => {
            let style = if hl {
                Theme::highlight()
            } else {
                Theme::preview()
            };
            let data = [1u64, 2, 3, 2, 5, 4, 7, 3];
            let sp = Sparkline::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" sparkline ")
                        .border_style(style),
                )
                .data(&data)
                .style(style);
            f.render_widget(sp, area);
        }
        Kind::BarChart => {
            let style = if hl {
                Theme::highlight()
            } else {
                Theme::preview()
            };
            // Lightweight stand-in — full BarChart needs more setup.
            f.render_widget(
                Paragraph::new(" ▂▄▆█ ▄█ ▆  (BarChart)")
                    .style(style)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(" bars ")
                            .border_style(style),
                    ),
                area,
            );
        }
        Kind::Chart => {
            let style = if hl {
                Theme::highlight()
            } else {
                Theme::preview()
            };
            let field = if node.field.is_empty() {
                "chart"
            } else {
                &node.field
            };
            f.render_widget(
                Paragraph::new("  ·  ··  ···  ··  (Chart)")
                    .style(style)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(format!(" {field} "))
                            .border_style(style),
                    ),
                area,
            );
        }
    }
}

fn contains_id(node: &Node, id: usize) -> bool {
    node.id == id || node.children.iter().any(|c| contains_id(c, id))
}

fn paint_placeholder(f: &mut Frame, area: Rect, label: &str, hl: bool) {
    let style = if hl {
        Theme::highlight()
    } else {
        Theme::preview()
    };
    f.render_widget(
        Paragraph::new(format!("({label})")).style(style).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(style),
        ),
        area,
    );
}

fn child_constraints(children: &[Node], dir: Direction, total: u16) -> Vec<Constraint> {
    if children.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(children.len());
    for c in children {
        let size = c.effective_size();
        out.push(constraint_for(&size, total));
    }
    let _ = dir;
    out
}

fn constraint_for(size: &SizeHint, total: u16) -> Constraint {
    match size {
        SizeHint::Length(n) => Constraint::Length((*n as u16).min(total).max(1)),
        SizeHint::Percent(n) => Constraint::Percentage((*n as u16).min(100)),
        SizeHint::Min(n) => Constraint::Min((*n as u16).max(1)),
        SizeHint::FillN(n) => Constraint::Fill((*n as u16).max(1)),
        SizeHint::Fill | SizeHint::Default => Constraint::Fill(1),
    }
}

pub fn hit_test(hits: &HitList, col: u16, row: u16) -> Option<usize> {
    // Prefer the deepest (last painted / most specific) hit.
    hits.iter()
        .rev()
        .find(|(_, r)| col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height)
        .map(|(id, _)| *id)
}
