fn fruits() -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    v.push("Apple".to_string());
    v.push("Banana".to_string());
    v.push("Cherry".to_string());
    v.push("Date".to_string());
    v
}

use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

struct Menu {
    fruits: Vec<String>,
    choice: String,
    fruits_state: ratatui::widgets::ListState,
}

impl Default for Menu {
    fn default() -> Self {
        Menu {
            fruits: fruits(),
            choice: "(none yet)".to_string(),
            fruits_state: ratatui::widgets::ListState::default().with_selected(Some(0)),
        }
    }
}

fn view(state: &mut Menu, frame: &mut Frame) {
    let block = Block::bordered().title("Fruit Picker");
    let area = frame.area();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Fill(1), Constraint::Length(1)]).split(inner);
    frame.render_widget(Paragraph::new(" Up/Down to move, Enter to pick, q to quit"), chunks_0[0]);
    let items_1: Vec<ratatui::widgets::ListItem> = state.fruits.iter().map(|s| ratatui::widgets::ListItem::new(s.clone())).collect();
    let list_1 = ratatui::widgets::List::new(items_1).highlight_symbol("» ").highlight_style(ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED));
    frame.render_stateful_widget(list_1, chunks_0[1], &mut state.fruits_state);
    frame.render_widget(Paragraph::new(format!("{}{}", " You picked: ", state.choice)), chunks_0[2]);
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let mut state = Menu::default();
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
                        state.fruits_state.select_next();
                    }
                    KeyCode::Up => {
                        state.fruits_state.select_previous();
                    }
                    KeyCode::Enter => {
                        if let Some(i) = state.fruits_state.selected() {
                            let item = state.fruits[i].clone();
                            state.choice = item;
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
