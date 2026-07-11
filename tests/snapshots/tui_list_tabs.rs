// tui_list_tabs.vbr — a selectable List nested inside a view `Match`. The List
// is a focusable widget wherever it sits: VBR declares its `<field>_state`
// (the ratatui ListState) and wires Up/Down/Enter even when the widget only
// appears in one arm of a `Match` (or a branch of a view `If`). Switch tabs to
// show a different list; the selection state rides along.

use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

struct ListTabs {
    tab: i32,
    fruit: Vec<String>,
    veg: Vec<String>,
    picked: String,
    fruit_state: ratatui::widgets::ListState,
    veg_state: ratatui::widgets::ListState,
    focus_index: usize,
}

impl Default for ListTabs {
    fn default() -> Self {
        ListTabs {
            tab: 1,
            fruit: vec!["apple".to_string(), "pear".to_string(), "plum".to_string()],
            veg: vec!["kale".to_string(), "leek".to_string(), "bean".to_string()],
            picked: "nothing yet".to_string(),
            fruit_state: ratatui::widgets::ListState::default().with_selected(Some(0)),
            veg_state: ratatui::widgets::ListState::default().with_selected(Some(0)),
            focus_index: 0,
        }
    }
}

fn view(state: &mut ListTabs, frame: &mut Frame) {
    let block = Block::bordered().title("Tabbed lists");
    let area = frame.area();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1)]).split(inner);
    frame.render_widget(Paragraph::new(" 1/2 switch tab • Up/Down move • Enter pick • q quits"), chunks_0[0]);
    frame.render_widget(Paragraph::new(format!(" Picked: {}", state.picked)), chunks_0[1]);
    match state.tab {
        1 => {
            let chunks_1 = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).split(chunks_0[2]);
            frame.render_widget(Paragraph::new("── Fruit ──"), chunks_1[0]);
            let items_2: Vec<ratatui::widgets::ListItem> = state.fruit.iter().map(|s| ratatui::widgets::ListItem::new(s.clone())).collect();
            let list_2 = ratatui::widgets::List::new(items_2).highlight_symbol("» ").highlight_style(ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED));
            frame.render_stateful_widget(list_2, chunks_1[1], &mut state.fruit_state);
        }
        _ => {
            let chunks_3 = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).split(chunks_0[2]);
            frame.render_widget(Paragraph::new("── Veg ──"), chunks_3[0]);
            let items_4: Vec<ratatui::widgets::ListItem> = state.veg.iter().map(|s| ratatui::widgets::ListItem::new(s.clone())).collect();
            let list_4 = ratatui::widgets::List::new(items_4).highlight_symbol("» ").highlight_style(ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED));
            frame.render_stateful_widget(list_4, chunks_3[1], &mut state.veg_state);
        }
    }
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let mut state = ListTabs::default();
    let mut terminal = ratatui::init();
    loop {
        terminal.draw(|frame| view(&mut state, frame))?;
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('1') => {
                        state.tab = 1;
                    }
                    KeyCode::Char('2') => {
                        state.tab = 2;
                    }
                    KeyCode::Char('q') => {
                        break;
                    }
                    KeyCode::Down => {
                        match state.focus_index {
                            0 => state.fruit_state.select_next(),
                            1 => state.veg_state.select_next(),
                            _ => {}
                        }
                    }
                    KeyCode::Up => {
                        match state.focus_index {
                            0 => state.fruit_state.select_previous(),
                            1 => state.veg_state.select_previous(),
                            _ => {}
                        }
                    }
                    KeyCode::Tab => {
                        state.focus_index = (state.focus_index + 1) % 2;
                    }
                    KeyCode::Enter => {
                        match state.focus_index {
                            0 => {
                                if let Some(i) = state.fruit_state.selected() {
                                    let choice = state.fruit[i].clone();
                                    state.picked = format!("fruit: {}", choice);
                                }
                            }
                            1 => {
                                if let Some(i) = state.veg_state.selected() {
                                    let choice = state.veg[i].clone();
                                    state.picked = format!("veg: {}", choice);
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
