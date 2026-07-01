#[derive(Debug, Clone)]
struct Person {
    pub name: String,
    pub age: i32,
    pub city: String,
}

fn roster() -> Vec<Person> {
    let mut v: Vec<Person> = Vec::new();
    v.push(Person { name: "Ada".to_string(), age: 36, city: "London".to_string() });
    v.push(Person { name: "Bjarne".to_string(), age: 60, city: "Aarhus".to_string() });
    v.push(Person { name: "Grace".to_string(), age: 79, city: "New York".to_string() });
    v
}

use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

struct People {
    people: Vec<Person>,
    status: String,
    people_state: ratatui::widgets::TableState,
}

impl Default for People {
    fn default() -> Self {
        People {
            people: roster(),
            status: "(select a row)".to_string(),
            people_state: ratatui::widgets::TableState::default().with_selected(Some(0)),
        }
    }
}

fn view(state: &mut People, frame: &mut Frame) {
    let block = Block::bordered().title("People — Up/Down, Enter, q to quit");
    let area = frame.area();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Fill(1), Constraint::Length(1)]).split(inner);
    frame.render_widget(Paragraph::new(" Up/Down to move, Enter to select, q to quit"), chunks_0[0]);
    let rows_1: Vec<ratatui::widgets::Row> = state.people.iter().map(|row| ratatui::widgets::Row::new(vec![row.name.clone(), row.age.to_string(), row.city.clone()])).collect();
    let table_1 = ratatui::widgets::Table::new(rows_1, [Constraint::Fill(1), Constraint::Fill(1), Constraint::Fill(1)])
        .header(ratatui::widgets::Row::new(vec!["name", "age", "city"]).style(ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::BOLD)))
        .row_highlight_style(ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)).highlight_symbol("» ");
    frame.render_stateful_widget(table_1, chunks_0[1], &mut state.people_state);
    frame.render_widget(Paragraph::new(format!("{}{}", " ", state.status)), chunks_0[2]);
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let mut state = People::default();
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
                        state.people_state.select_next();
                    }
                    KeyCode::Up => {
                        state.people_state.select_previous();
                    }
                    KeyCode::Enter => {
                        if let Some(i) = state.people_state.selected() {
                            let who = state.people[i].clone();
                            state.status = format!("{}{}", format!("{}{}", format!("{}{}", format!("{}{}", who.name, " is "), who.age), ", from "), who.city);
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
