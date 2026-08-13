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
use ratatui::text::{Line, Span};
use ratatui::Frame;

struct Menu {
    fruits: Vec<String>,
    choice: String,
    fruits_state: ratatui::widgets::ListState,
}

impl Default for Menu {
    fn default() -> Self {
        let fruits = fruits();
        let choice = "(none yet)".to_string();
        Menu {
            fruits,
            choice,
            fruits_state: ratatui::widgets::ListState::default().with_selected(Some(0)),
        }
    }
}

fn view(state: &mut Menu, frame: &mut Frame) {
    let area = frame.area();
    let chunks_status = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(area);
    let block = Block::bordered().title("Fruit Picker");
    let inner = block.inner(chunks_status[0]);
    frame.render_widget(block, chunks_status[0]);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Fill(1), Constraint::Length(1)]).split(inner);
    frame.render_widget(Paragraph::new(" Up/Down to move, Enter to pick, q to quit"), chunks_0[0]);
    let items_1: Vec<ratatui::widgets::ListItem> = state.fruits.iter().map(|s| ratatui::widgets::ListItem::new(s.clone())).collect();
    let list_1 = ratatui::widgets::List::new(items_1).highlight_symbol("» ").highlight_style(ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED));
    frame.render_stateful_widget(list_1, chunks_0[1], &mut state.fruits_state);
    frame.render_widget(Paragraph::new(format!(" You picked: {}", state.choice)), chunks_0[2]);
    frame.render_widget(Paragraph::new(Line::from(vec![Span::raw(" "), Span::styled(" q ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" Quit  "), Span::styled(" Up/Down ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" move  "), Span::styled(" Enter ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" ok  ")])).style(ratatui::style::Style::new().bg(ratatui::style::Color::Cyan).fg(ratatui::style::Color::Black)), chunks_status[1]);
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
