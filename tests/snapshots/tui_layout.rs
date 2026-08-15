use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::Frame;

struct Dashboard {
    tab: i32,
}

impl Default for Dashboard {
    fn default() -> Self {
        let tab = 1;
        Dashboard {
            tab,
        }
    }
}

fn view(state: &Dashboard, frame: &mut Frame) {
    let area = frame.area();
    let chunks_status = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(area);
    let block = Block::bordered().title("Bust TUI Layout");
    let inner = block.inner(chunks_status[0]);
    frame.render_widget(block, chunks_status[0]);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Fill(1), Constraint::Length(1)]).split(inner);
    frame.render_widget(Paragraph::new("  Dashboard — press 1/2/3 to switch tab, q to quit"), chunks_0[0]);
    let chunks_1 = Layout::horizontal([Constraint::Percentage(30), Constraint::Fill(1)]).spacing(1).split(chunks_0[1]);
    let chunks_2 = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)]).split(chunks_1[0]);
    frame.render_widget(Paragraph::new(" Sidebar"), chunks_2[0]);
    frame.render_widget(Paragraph::new(" - Overview"), chunks_2[1]);
    frame.render_widget(Paragraph::new(" - Details"), chunks_2[2]);
    frame.render_widget(Paragraph::new(" - Settings"), chunks_2[3]);
    let chunks_3 = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)]).split(chunks_1[1]);
    frame.render_widget(Paragraph::new(" Main panel"), chunks_3[0]);
    frame.render_widget(Paragraph::new(""), chunks_3[1]);
    frame.render_widget(Paragraph::new(format!("Active tab: {}", state.tab)), chunks_3[2]);
    frame.render_widget(Paragraph::new("  status: ok"), chunks_0[2]);
    frame.render_widget(Paragraph::new(Line::from(vec![Span::raw(" "), Span::styled(" 1 ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" ShowOne  "), Span::styled(" 2 ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" ShowTwo  "), Span::styled(" 3 ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" ShowThree  "), Span::styled(" q ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" Quit  ")])).style(ratatui::style::Style::new().bg(ratatui::style::Color::Cyan).fg(ratatui::style::Color::Black)), chunks_status[1]);
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let mut state = Dashboard::default();
    let mut terminal = ratatui::init();
    loop {
        terminal.draw(|frame| view(&state, frame))?;
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('1') => {
                        {
                            let __vbr_event: Result<(), String> = (|| {
                                state.tab = 1;
                                Ok(())
                            })();
                            if let Err(__e) = __vbr_event {
                                eprintln!("Error: {}", __e);
                            }
                        }
                    }
                    KeyCode::Char('2') => {
                        {
                            let __vbr_event: Result<(), String> = (|| {
                                state.tab = 2;
                                Ok(())
                            })();
                            if let Err(__e) = __vbr_event {
                                eprintln!("Error: {}", __e);
                            }
                        }
                    }
                    KeyCode::Char('3') => {
                        {
                            let __vbr_event: Result<(), String> = (|| {
                                state.tab = 3;
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
