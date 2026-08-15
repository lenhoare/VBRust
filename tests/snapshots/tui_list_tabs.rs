// tui_list_tabs.vbr — a selectable List nested inside a Tabs pane. The List
// is a focusable widget wherever it sits: Bust declares its `<field>_state`
// (the ratatui ListState) and wires Up/Down/Enter even when the widget only
// appears in one pane. Switch tabs with Left/Right (or 1/2) when the tab bar
// is focused; Tab moves focus to the list. Each pane's list keeps its own
// selection.

use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
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
        let tab = 0;
        let fruit = vec!["apple".to_string(), "pear".to_string(), "plum".to_string()];
        let veg = vec!["kale".to_string(), "leek".to_string(), "bean".to_string()];
        let picked = "nothing yet".to_string();
        ListTabs {
            tab,
            fruit,
            veg,
            picked,
            fruit_state: ratatui::widgets::ListState::default().with_selected(Some(0)),
            veg_state: ratatui::widgets::ListState::default().with_selected(Some(0)),
            focus_index: 0,
        }
    }
}

fn view(state: &mut ListTabs, frame: &mut Frame) {
    let area = frame.area();
    let chunks_status = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(area);
    let block = Block::bordered().title("Tabbed lists");
    let inner = block.inner(chunks_status[0]);
    frame.render_widget(block, chunks_status[0]);
    let titles_0 = vec!["Fruit".to_string(), "Veg".to_string()];
    let mut style_0 = ratatui::style::Style::new();
    if state.focus_index == 0 {
        style_0 = style_0.add_modifier(ratatui::style::Modifier::UNDERLINED);
    }
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).split(inner);
    frame.render_widget(ratatui::widgets::Tabs::new(titles_0).select((state.tab.max(0) as usize).min(1)).highlight_style(ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)).style(style_0), chunks_0[0]);
    match state.tab {
        0 => {
            let items_1: Vec<ratatui::widgets::ListItem> = state.fruit.iter().map(|s| ratatui::widgets::ListItem::new(s.clone())).collect();
            let list_1 = ratatui::widgets::List::new(items_1).highlight_symbol("» ").highlight_style(ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED));
            frame.render_stateful_widget(list_1, chunks_0[1], &mut state.fruit_state);
        }
        _ => {
            let items_2: Vec<ratatui::widgets::ListItem> = state.veg.iter().map(|s| ratatui::widgets::ListItem::new(s.clone())).collect();
            let list_2 = ratatui::widgets::List::new(items_2).highlight_symbol("» ").highlight_style(ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED));
            frame.render_stateful_widget(list_2, chunks_0[1], &mut state.veg_state);
        }
    }
    frame.render_widget(Paragraph::new(Line::from(vec![Span::raw(format!(" {}  ", format!("Picked: {}", state.picked))), Span::styled(" q ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" quit  "), Span::styled(" Tab ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" focus  "), Span::styled(" Up/Down ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" move  "), Span::styled(" Left/Right ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" switch  "), Span::styled(" Enter ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" ok  ")])).style(ratatui::style::Style::new().bg(ratatui::style::Color::Cyan).fg(ratatui::style::Color::Black)), chunks_status[1]);
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
                    KeyCode::Char('q') => {
                        break;
                    }
                    KeyCode::Tab => {
                        state.focus_index = (state.focus_index + 1) % 3;
                    }
                    KeyCode::Down => {
                        match state.focus_index {
                            1 => state.fruit_state.select_next(),
                            2 => state.veg_state.select_next(),
                            _ => {}
                        }
                    }
                    KeyCode::Up => {
                        match state.focus_index {
                            1 => state.fruit_state.select_previous(),
                            2 => state.veg_state.select_previous(),
                            _ => {}
                        }
                    }
                    KeyCode::Left => {
                        match state.focus_index {
                            0 => {
                                let next_tab = (state.tab - 1).rem_euclid(2);
                                if next_tab != state.tab {
                                    state.tab = next_tab;
                                }
                            }
                            _ => {}
                        }
                    }
                    KeyCode::Right => {
                        match state.focus_index {
                            0 => {
                                let next_tab = (state.tab + 1).rem_euclid(2);
                                if next_tab != state.tab {
                                    state.tab = next_tab;
                                }
                            }
                            _ => {}
                        }
                    }
                    KeyCode::Enter => {
                        match state.focus_index {
                            0 => {
                                let next_tab = (state.tab + 1).rem_euclid(2);
                                if next_tab != state.tab {
                                    state.tab = next_tab;
                                }
                            }
                            1 => {
                                if let Some(i) = state.fruit_state.selected() {
                                    let choice = state.fruit[i].clone();
                                    {
                                        let __vbr_event: Result<(), String> = (|| {
                                            state.picked = format!("fruit: {}", choice);
                                            Ok(())
                                        })();
                                        if let Err(__e) = __vbr_event {
                                            eprintln!("Error: {}", __e);
                                        }
                                    }
                                }
                            }
                            2 => {
                                if let Some(i) = state.veg_state.selected() {
                                    let choice = state.veg[i].clone();
                                    {
                                        let __vbr_event: Result<(), String> = (|| {
                                            state.picked = format!("veg: {}", choice);
                                            Ok(())
                                        })();
                                        if let Err(__e) = __vbr_event {
                                            eprintln!("Error: {}", __e);
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    KeyCode::Char(c) => {
                        match state.focus_index {
                            0 => {
                                if let Some(d) = c.to_digit(10) {
                                    let i = d as i32 - 1;
                                    if i >= 0 && i < 2 && i != state.tab {
                                        state.tab = i;
                                    }
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
