fn left_items() -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    v.push("Alpha".to_string());
    v.push("Beta".to_string());
    v.push("Gamma".to_string());
    v
}

fn right_items() -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    v.push("One".to_string());
    v.push("Two".to_string());
    v.push("Three".to_string());
    v
}

use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

struct Panels {
    left: Vec<String>,
    right: Vec<String>,
    log: String,
    left_state: ratatui::widgets::ListState,
    right_state: ratatui::widgets::ListState,
    focus_index: usize,
}

impl Default for Panels {
    fn default() -> Self {
        Panels {
            left: left_items(),
            right: right_items(),
            log: "(nothing picked yet)".to_string(),
            left_state: ratatui::widgets::ListState::default().with_selected(Some(0)),
            right_state: ratatui::widgets::ListState::default().with_selected(Some(0)),
            focus_index: 0,
        }
    }
}

fn view(state: &mut Panels, frame: &mut Frame) {
    let block = Block::bordered().title("Two Lists — Tab to switch");
    let area = frame.area();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Fill(1), Constraint::Length(1)]).split(inner);
    frame.render_widget(Paragraph::new(" Tab switches list, Up/Down move, Enter picks, q quits"), chunks_0[0]);
    let chunks_1 = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).spacing(2).split(chunks_0[1]);
    let items_2: Vec<ratatui::widgets::ListItem> = state.left.iter().map(|s| ratatui::widgets::ListItem::new(s.clone())).collect();
    let list_2 = ratatui::widgets::List::new(items_2).highlight_symbol("» ").highlight_style(ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED));
    frame.render_stateful_widget(list_2, chunks_1[0], &mut state.left_state);
    let items_3: Vec<ratatui::widgets::ListItem> = state.right.iter().map(|s| ratatui::widgets::ListItem::new(s.clone())).collect();
    let list_3 = ratatui::widgets::List::new(items_3).highlight_symbol("» ").highlight_style(ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED));
    frame.render_stateful_widget(list_3, chunks_1[1], &mut state.right_state);
    frame.render_widget(Paragraph::new(format!("{}{}", " Last pick: ", state.log)), chunks_0[2]);
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let mut state = Panels::default();
    let mut terminal = ratatui::init();
    loop {
        terminal.draw(|frame| view(&mut state, frame))?;
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => {
                        break;
                    }
                    KeyCode::Down => {
                        match state.focus_index {
                            0 => state.left_state.select_next(),
                            1 => state.right_state.select_next(),
                            _ => {}
                        }
                    }
                    KeyCode::Up => {
                        match state.focus_index {
                            0 => state.left_state.select_previous(),
                            1 => state.right_state.select_previous(),
                            _ => {}
                        }
                    }
                    KeyCode::Tab => {
                        state.focus_index = (state.focus_index + 1) % 2;
                    }
                    KeyCode::Enter => {
                        match state.focus_index {
                            0 => {
                                if let Some(i) = state.left_state.selected() {
                                    let item = state.left[i].clone();
                                    state.log = format!("{}{}", "left / ", item);
                                }
                            }
                            1 => {
                                if let Some(i) = state.right_state.selected() {
                                    let item = state.right[i].clone();
                                    state.log = format!("{}{}", "right / ", item);
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    ratatui::restore();
    Ok(())
}
