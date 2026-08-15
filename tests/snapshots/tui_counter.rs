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
    let area = frame.area();
    let chunks_status = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(area);
    let block = Block::bordered().title("Bust Terminal Counter");
    let inner = block.inner(chunks_status[0]);
    frame.render_widget(block, chunks_status[0]);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(inner);
    frame.render_widget(Paragraph::new("A Bust terminal app"), chunks_0[0]);
    frame.render_widget(Paragraph::new(format!("Count: {}", state.count)), chunks_0[1]);
    frame.render_widget(Paragraph::new(Line::from(vec![Span::raw(format!(" {}  ", format!("Count: {}", state.count))), Span::styled(" + ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" inc  "), Span::styled(" - ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" dec  "), Span::styled(" q ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" quit  ")])).style(ratatui::style::Style::new().bg(ratatui::style::Color::Cyan).fg(ratatui::style::Color::Black)), chunks_status[1]);
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
