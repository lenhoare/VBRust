// Button, Checkbox, and Radio on a Screen — Tab to move, Enter or Space to fire.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum Size {
    Small,
    Medium,
    Large,
}
impl std::fmt::Display for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::Frame;

struct Controls {
    count: i32,
    remember: bool,
    choice: Size,
    log: String,
    focus_index: usize,
}

impl Default for Controls {
    fn default() -> Self {
        let count = 0;
        let remember = false;
        let choice = Size::Small;
        let log = "Tab around, Enter or Space to fire".to_string();
        Controls {
            count,
            remember,
            choice,
            log,
            focus_index: 0,
        }
    }
}

fn view(state: &Controls, frame: &mut Frame) {
    let area = frame.area();
    let chunks_status = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(area);
    let block = Block::bordered().title("Controls");
    let inner = block.inner(chunks_status[0]);
    frame.render_widget(block, chunks_status[0]);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1), Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)]).split(inner);
    let mut style_1 = ratatui::style::Style::new();
    if state.focus_index == 0 {
        style_1 = style_1.add_modifier(ratatui::style::Modifier::REVERSED);
    }
    frame.render_widget(Paragraph::new(format!("[ {} ]", "Click me")).style(style_1), chunks_0[0]);
    let mut style_2 = ratatui::style::Style::new();
    if state.focus_index == 1 {
        style_2 = style_2.add_modifier(ratatui::style::Modifier::REVERSED);
    }
    let mark_2 = if state.remember { "x" } else { " " };
    frame.render_widget(Paragraph::new(format!("[{}] {}", mark_2, "Remember me")).style(style_2), chunks_0[1]);
    let mut style_3 = ratatui::style::Style::new();
    if state.focus_index == 2 {
        style_3 = style_3.add_modifier(ratatui::style::Modifier::REVERSED);
    }
    let mark_3 = if state.choice == Size::Small { "*" } else { " " };
    frame.render_widget(Paragraph::new(format!("({}) {}", mark_3, "Small")).style(style_3), chunks_0[2]);
    let mut style_4 = ratatui::style::Style::new();
    if state.focus_index == 3 {
        style_4 = style_4.add_modifier(ratatui::style::Modifier::REVERSED);
    }
    let mark_4 = if state.choice == Size::Medium { "*" } else { " " };
    frame.render_widget(Paragraph::new(format!("({}) {}", mark_4, "Medium")).style(style_4), chunks_0[3]);
    let mut style_5 = ratatui::style::Style::new();
    if state.focus_index == 4 {
        style_5 = style_5.add_modifier(ratatui::style::Modifier::REVERSED);
    }
    let mark_5 = if state.choice == Size::Large { "*" } else { " " };
    frame.render_widget(Paragraph::new(format!("({}) {}", mark_5, "Large")).style(style_5), chunks_0[4]);
    frame.render_widget(Paragraph::new(format!(" clicks: {}", state.count)), chunks_0[5]);
    frame.render_widget(Paragraph::new(Line::from(vec![Span::raw(format!(" {}  ", format!("{}", state.log))), Span::styled(" q ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" quit  "), Span::styled(" Tab ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" focus  "), Span::styled(" Enter ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" ok  ")])).style(ratatui::style::Style::new().bg(ratatui::style::Color::Cyan).fg(ratatui::style::Color::Black)), chunks_status[1]);
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let mut state = Controls::default();
    let mut terminal = ratatui::init();
    loop {
        terminal.draw(|frame| view(&state, frame))?;
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => {
                        break;
                    }
                    KeyCode::Tab => {
                        state.focus_index = (state.focus_index + 1) % 5;
                    }
                    KeyCode::Enter => {
                        match state.focus_index {
                            0 => {
                                {
                                    let __vbr_event: Result<(), String> = (|| {
                                        state.count += 1;
                                        state.log = format!("button → {}", state.count);
                                        Ok(())
                                    })();
                                    if let Err(__e) = __vbr_event {
                                        eprintln!("Error: {}", __e);
                                    }
                                }
                            }
                            1 => {
                                state.remember = !state.remember;
                                let value = state.remember;
                                {
                                    let __vbr_event: Result<(), String> = (|| {
                                        state.remember = value;
                                        if state.remember {
                                            state.log = "checkbox on".to_string();
                                        } else {
                                            state.log = "checkbox off".to_string();
                                        }
                                        Ok(())
                                    })();
                                    if let Err(__e) = __vbr_event {
                                        eprintln!("Error: {}", __e);
                                    }
                                }
                            }
                            2 => {
                                state.choice = Size::Small;
                                let value = state.choice;
                                {
                                    let __vbr_event: Result<(), String> = (|| {
                                        state.choice = value;
                                        state.log = "radio changed".to_string();
                                        Ok(())
                                    })();
                                    if let Err(__e) = __vbr_event {
                                        eprintln!("Error: {}", __e);
                                    }
                                }
                            }
                            3 => {
                                state.choice = Size::Medium;
                                let value = state.choice;
                                {
                                    let __vbr_event: Result<(), String> = (|| {
                                        state.choice = value;
                                        state.log = "radio changed".to_string();
                                        Ok(())
                                    })();
                                    if let Err(__e) = __vbr_event {
                                        eprintln!("Error: {}", __e);
                                    }
                                }
                            }
                            4 => {
                                state.choice = Size::Large;
                                let value = state.choice;
                                {
                                    let __vbr_event: Result<(), String> = (|| {
                                        state.choice = value;
                                        state.log = "radio changed".to_string();
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
                                if c == ' ' {
                                    {
                                        let __vbr_event: Result<(), String> = (|| {
                                            state.count += 1;
                                            state.log = format!("button → {}", state.count);
                                            Ok(())
                                        })();
                                        if let Err(__e) = __vbr_event {
                                            eprintln!("Error: {}", __e);
                                        }
                                    }
                                }
                            }
                            1 => {
                                if c == ' ' {
                                    state.remember = !state.remember;
                                    let value = state.remember;
                                    {
                                        let __vbr_event: Result<(), String> = (|| {
                                            state.remember = value;
                                            if state.remember {
                                                state.log = "checkbox on".to_string();
                                            } else {
                                                state.log = "checkbox off".to_string();
                                            }
                                            Ok(())
                                        })();
                                        if let Err(__e) = __vbr_event {
                                            eprintln!("Error: {}", __e);
                                        }
                                    }
                                }
                            }
                            2 => {
                                if c == ' ' {
                                    state.choice = Size::Small;
                                    let value = state.choice;
                                    {
                                        let __vbr_event: Result<(), String> = (|| {
                                            state.choice = value;
                                            state.log = "radio changed".to_string();
                                            Ok(())
                                        })();
                                        if let Err(__e) = __vbr_event {
                                            eprintln!("Error: {}", __e);
                                        }
                                    }
                                }
                            }
                            3 => {
                                if c == ' ' {
                                    state.choice = Size::Medium;
                                    let value = state.choice;
                                    {
                                        let __vbr_event: Result<(), String> = (|| {
                                            state.choice = value;
                                            state.log = "radio changed".to_string();
                                            Ok(())
                                        })();
                                        if let Err(__e) = __vbr_event {
                                            eprintln!("Error: {}", __e);
                                        }
                                    }
                                }
                            }
                            4 => {
                                if c == ' ' {
                                    state.choice = Size::Large;
                                    let value = state.choice;
                                    {
                                        let __vbr_event: Result<(), String> = (|| {
                                            state.choice = value;
                                            state.log = "radio changed".to_string();
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
