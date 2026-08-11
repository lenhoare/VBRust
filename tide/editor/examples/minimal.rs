//! Minimal host: edit a file in the terminal with tide-editor.
//!
//! ```bash
//! cargo run -p tide-editor --example minimal -- README.md
//! ```

use std::env;
use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{DefaultTerminal, Frame};
use tide_editor::{
    Document, EditorView, EditorWidget, Key, KeyEvent, KeyMods, PlainHighlighter,
};

fn main() -> io::Result<()> {
    let path = env::args().nth(1);
    let mut doc = match &path {
        Some(p) => Document::open(p).unwrap_or_else(|e| {
            eprintln!("open {p}: {e}");
            Document::new()
        }),
        None => Document::from_str("Hello from tide-editor.\nEdit me. Ctrl+Q to quit.\n"),
    };
    let mut view = EditorView::new();

    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut doc, &mut view);
    ratatui::restore();
    result
}

fn run(
    terminal: &mut DefaultTerminal,
    doc: &mut Document,
    view: &mut EditorView,
) -> io::Result<()> {
    let highlighter = PlainHighlighter;
    loop {
        terminal.draw(|f| draw(f, doc, view, &highlighter))?;

        if !event::poll(Duration::from_millis(50))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
        {
            break;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('s') | KeyCode::Char('S'))
        {
            if doc.path().is_some() {
                let _ = doc.save();
            }
            continue;
        }

        if let Some(ev) = map_key(key) {
            view.handle_key(doc, &ev);
            let area_h = terminal.size()?.height.saturating_sub(2) as usize;
            let area_w = terminal
                .size()?
                .width
                .saturating_sub(view.gutter_width(doc)) as usize;
            view.ensure_visible(doc, area_h.max(1), area_w.max(1));
        }
    }
    Ok(())
}

fn draw(f: &mut Frame, doc: &Document, view: &EditorView, highlighter: &PlainHighlighter) {
    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(f.area());

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" tide-editor minimal — Ctrl+S save, Ctrl+Q quit ")
        .style(Style::default().fg(Color::White).bg(Color::Black));
    let inner = block.inner(chunks[0]);
    f.render_widget(block, chunks[0]);

    let widget = EditorWidget::new(doc, view)
        .highlighter(highlighter)
        .style(Style::default().fg(Color::White).bg(Color::Black));
    f.render_widget(widget, inner);

    let (line, col) = view.cursor_position(doc);
    let dirty = if doc.is_dirty() { "*" } else { " " };
    let name = doc
        .path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "[untitled]".into());
    let status = format!(" {name}{dirty}  Ln {}, Col {} ", line + 1, col + 1);
    f.render_widget(
        Paragraph::new(status).style(Style::default().bg(Color::DarkGray).fg(Color::White)),
        chunks[1],
    );
}

fn map_key(key: crossterm::event::KeyEvent) -> Option<KeyEvent> {
    let mods = KeyMods {
        ctrl: key.modifiers.contains(KeyModifiers::CONTROL),
        alt: key.modifiers.contains(KeyModifiers::ALT),
        shift: key.modifiers.contains(KeyModifiers::SHIFT),
    };
    let k = match key.code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Enter => Key::Enter,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Delete => Key::Delete,
        KeyCode::Tab => Key::Tab,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Esc => Key::Esc,
        _ => return None,
    };
    Some(KeyEvent::new(k, mods))
}
