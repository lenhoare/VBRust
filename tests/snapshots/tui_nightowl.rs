// Theme on a Screen — Night Owl (Sarah Drasner's palette) colours the terminal
// chrome, borders, and text. Same `Theme` line as a Window or Page.

use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::Frame;

struct Counter {
    count: i32,
}

impl Default for Counter {
    fn default() -> Self {
        let count = 0;
        Counter {
            count,
        }
    }
}

fn view(state: &Counter, frame: &mut Frame) {
    let theme_body = ratatui::style::Style::new().bg(ratatui::style::Color::Rgb(1, 22, 39)).fg(ratatui::style::Color::Rgb(214, 222, 235));
    let theme_chrome = ratatui::style::Style::new().bg(ratatui::style::Color::Rgb(130, 170, 255)).fg(ratatui::style::Color::Rgb(1, 22, 39));
    let theme_accent = ratatui::style::Color::Rgb(130, 170, 255);
    let area = frame.area();
    let chunks_status = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(area);
    let block = Block::bordered().title("Night Owl").style(theme_body).border_style(ratatui::style::Style::new().fg(theme_accent));
    let inner = block.inner(chunks_status[0]);
    frame.render_widget(block, chunks_status[0]);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(inner);
    frame.render_widget(Paragraph::new("A themed Bust terminal app").style(theme_body), chunks_0[0]);
    frame.render_widget(Paragraph::new(format!("Count: {}", state.count)).style(theme_body), chunks_0[1]);
    frame.render_widget(Paragraph::new(Line::from(vec![Span::raw(format!(" {}  ", format!("Count: {}", state.count))), Span::styled(" + ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" inc  "), Span::styled(" - ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" dec  "), Span::styled(" q ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" quit  ")])).style(theme_chrome), chunks_status[1]);
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let mut state = Counter::default();
    let mut terminal = ratatui::init();
    loop {
        terminal.draw(|frame| view(&state, frame))?;
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('+') => {
                        {
                            let __vbr_event: Result<(), String> = (|| {
                                state.count += 1;
                                Ok(())
                            })();
                            if let Err(__e) = __vbr_event {
                                eprintln!("Error: {}", __e);
                            }
                        }
                    }
                    KeyCode::Char('-') => {
                        {
                            let __vbr_event: Result<(), String> = (|| {
                                state.count -= 1;
                                Ok(())
                            })();
                            if let Err(__e) = __vbr_event {
                                eprintln!("Error: {}", __e);
                            }
                        }
                    }
                    KeyCode::Char('q') => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
    ratatui::restore();
    Ok(())
}
