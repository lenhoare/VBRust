fn seed() -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    v.push("buy milk".to_string());
    v.push("call Ada".to_string());
    v
}

use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

struct Notes {
    entry: String,
    notes: Vec<String>,
    status: String,
    notes_state: ratatui::widgets::ListState,
    focus_index: usize,
}

impl Default for Notes {
    fn default() -> Self {
        Notes {
            entry: "".to_string(),
            notes: seed(),
            status: "type a note, Enter to add".to_string(),
            notes_state: ratatui::widgets::ListState::default().with_selected(Some(0)),
            focus_index: 0,
        }
    }
}

fn view(state: &mut Notes, frame: &mut Frame) {
    let block = Block::bordered().title("Notes");
    let area = frame.area();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Length(3), Constraint::Fill(1), Constraint::Length(1)]).split(inner);
    frame.render_widget(Paragraph::new(" Tab: switch • type in the box • Enter: add/select • Esc: quit"), chunks_0[0]);
    frame.render_widget(Paragraph::new(state.entry.as_str()).block(Block::bordered().title("entry")), chunks_0[1]);
    if state.focus_index == 0 { frame.set_cursor_position((chunks_0[1].x + 1 + state.entry.chars().count() as u16, chunks_0[1].y + 1)); }
    let items_1: Vec<ratatui::widgets::ListItem> = state.notes.iter().map(|s| ratatui::widgets::ListItem::new(s.clone())).collect();
    let list_1 = ratatui::widgets::List::new(items_1).highlight_symbol("» ").highlight_style(ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED));
    frame.render_stateful_widget(list_1, chunks_0[2], &mut state.notes_state);
    frame.render_widget(Paragraph::new(format!(" {}", state.status)), chunks_0[3]);
}

fn main() -> std::io::Result<()> {
    use ratzilla::{DomBackend, WebRenderer};
    use ratzilla::event::KeyCode;
    let state = std::rc::Rc::new(std::cell::RefCell::new(Notes::default()));
    let backend = DomBackend::new()?;
    let mut terminal = ratzilla::ratatui::Terminal::new(backend)?;
    terminal.on_key_event({
        let state = state.clone();
        move |key| {
            let mut guard = state.borrow_mut();
            let state = &mut *guard;
            match key.code {
                KeyCode::Down => {
                    match state.focus_index {
                        1 => state.notes_state.select_next(),
                        _ => {}
                    }
                }
                KeyCode::Up => {
                    match state.focus_index {
                        1 => state.notes_state.select_previous(),
                        _ => {}
                    }
                }
                KeyCode::Tab => {
                    state.focus_index = (state.focus_index + 1) % 2;
                }
                KeyCode::Enter => {
                    match state.focus_index {
                        0 => {
                            let text = state.entry.clone();
                            state.status = format!("added: {}", text);
                            state.notes.push(text);
                            state.entry = "".to_string();
                        }
                        1 => {
                            if let Some(i) = state.notes_state.selected() {
                                let item = state.notes[i].clone();
                                state.status = format!("selected: {}", item);
                            }
                        }
                        _ => {}
                    }
                }
                KeyCode::Backspace => {
                    match state.focus_index {
                        0 => { state.entry.pop(); }
                        _ => {}
                    }
                }
                KeyCode::Char(c) => {
                    match state.focus_index {
                        0 => { state.entry.push(c); }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    })?;
    terminal.draw_web(move |frame| view(&mut state.borrow_mut(), frame));
    Ok(())
}
