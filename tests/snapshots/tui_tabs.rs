use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::Frame;

struct Tabs {
    tab: i32,
    busy: bool,
    focus_index: usize,
}

impl Default for Tabs {
    fn default() -> Self {
        let tab = 0;
        let busy = false;
        Tabs {
            tab,
            busy,
            focus_index: 0,
        }
    }
}

fn view(state: &Tabs, frame: &mut Frame) {
    let area = frame.area();
    let chunks_status = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(area);
    let block = Block::bordered().title("Tabs");
    let inner = block.inner(chunks_status[0]);
    frame.render_widget(block, chunks_status[0]);
    let titles_0 = vec!["Overview".to_string(), "Details".to_string(), "Settings".to_string()];
    let mut style_0 = ratatui::style::Style::new();
    if state.focus_index == 0 {
        style_0 = style_0.add_modifier(ratatui::style::Modifier::UNDERLINED);
    }
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).split(inner);
    frame.render_widget(ratatui::widgets::Tabs::new(titles_0).select((state.tab.max(0) as usize).min(2)).highlight_style(ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)).style(style_0), chunks_0[0]);
    match state.tab {
        0 => {
            let chunks_1 = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).split(chunks_0[1]);
            frame.render_widget(Paragraph::new("Welcome to tab one."), chunks_1[0]);
            if state.busy {
                frame.render_widget(Paragraph::new(" ● working…"), chunks_1[1]);
            } else {
                frame.render_widget(Paragraph::new(" ○ idle"), chunks_1[1]);
            }
        }
        1 => {
            frame.render_widget(Paragraph::new("Tab two has the details."), chunks_0[1]);
        }
        _ => {
            let mut style_2 = ratatui::style::Style::new();
            if state.focus_index == 1 {
                style_2 = style_2.add_modifier(ratatui::style::Modifier::REVERSED);
            }
            let mark_2 = if state.busy { "x" } else { " " };
            frame.render_widget(Paragraph::new(format!("[{}] {}", mark_2, "Busy")).style(style_2), chunks_0[1]);
        }
    }
    frame.render_widget(Paragraph::new(Line::from(vec![Span::raw(format!(" {}  ", format!("tab {}", state.tab))), Span::styled(" q ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" quit  "), Span::styled(" b ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" busy  "), Span::styled(" Tab ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" focus  "), Span::styled(" Left/Right ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" switch  "), Span::styled(" Enter ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" ok  ")])).style(ratatui::style::Style::new().bg(ratatui::style::Color::Cyan).fg(ratatui::style::Color::Black)), chunks_status[1]);
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let mut state = Tabs::default();
    let mut terminal = ratatui::init();
    loop {
        terminal.draw(|frame| view(&state, frame))?;
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => {
                        break;
                    }
                    KeyCode::Char('b') => {
                        {
                            let __vbr_event: Result<(), String> = (|| {
                                state.busy = !state.busy;
                                Ok(())
                            })();
                            if let Err(__e) = __vbr_event {
                                eprintln!("Error: {}", __e);
                            }
                        }
                    }
                    KeyCode::Tab => {
                        state.focus_index = (state.focus_index + 1) % 2;
                    }
                    KeyCode::Left => {
                        match state.focus_index {
                            0 => {
                                let next_tab = (state.tab - 1).rem_euclid(3);
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
                                let next_tab = (state.tab + 1).rem_euclid(3);
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
                                let next_tab = (state.tab + 1).rem_euclid(3);
                                if next_tab != state.tab {
                                    state.tab = next_tab;
                                }
                            }
                            1 => {
                                state.busy = !state.busy;
                                let value = state.busy;
                                {
                                    let __vbr_event: Result<(), String> = (|| {
                                        state.busy = value;
                                        Ok(())
                                    })();
                                    if let Err(__e) = __vbr_event {
                                        eprintln!("Error: {}", __e);
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
                                    if i >= 0 && i < 3 && i != state.tab {
                                        state.tab = i;
                                    }
                                }
                            }
                            1 => {
                                if c == ' ' {
                                    state.busy = !state.busy;
                                    let value = state.busy;
                                    {
                                        let __vbr_event: Result<(), String> = (|| {
                                            state.busy = value;
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
                    _ => {}
                }
            }
        }
    }
    ratatui::restore();
    Ok(())
}
