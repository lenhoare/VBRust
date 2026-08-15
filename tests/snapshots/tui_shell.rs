// A background process behind a Screen — the local-server shape: start a
// long-running command when the app boots (a fallible State initialiser, so a
// failed launch stops the program cleanly before the terminal opens), check on
// it from events, and stop it from a key. The child is detached from the
// terminal, so it can't scribble over the UI.

use vbr_stdlib::{Shell, Process};

use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::Frame;

struct ProcessPanel {
    worker: Process,
    status: String,
}

impl ProcessPanel {
    fn init() -> Result<ProcessPanel, String> {
        let worker = Shell::start("sleep 300")?;
        let status = "worker started".to_string();
        Ok(ProcessPanel {
            worker,
            status,
        })
    }
}

fn view(state: &ProcessPanel, frame: &mut Frame) {
    let area = frame.area();
    let chunks_status = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(area);
    let block = Block::bordered().title("Process Panel");
    let inner = block.inner(chunks_status[0]);
    frame.render_widget(block, chunks_status[0]);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)]).split(inner);
    frame.render_widget(Paragraph::new(format!("{}", state.status)), chunks_0[0]);
    frame.render_widget(Paragraph::new(""), chunks_0[1]);
    frame.render_widget(Paragraph::new("c = check on it, k = kill it, q = quit"), chunks_0[2]);
    frame.render_widget(Paragraph::new(Line::from(vec![Span::raw(" "), Span::styled(" c ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" Check  "), Span::styled(" k ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" Halt  "), Span::styled(" q ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" Quit  ")])).style(ratatui::style::Style::new().bg(ratatui::style::Color::Cyan).fg(ratatui::style::Color::Black)), chunks_status[1]);
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let mut state = match ProcessPanel::init() {
        Ok(state) => state,
        Err(message) => {
            eprintln!("could not start: {}", message);
            std::process::exit(1);
        }
    };
    let mut terminal = ratatui::init();
    loop {
        terminal.draw(|frame| view(&state, frame))?;
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('c') => {
                        {
                            let __vbr_event: Result<(), String> = (|| {
                                if state.worker.is_running() {
                                    state.status = "worker is running".to_string();
                                } else {
                                    state.status = "worker has stopped".to_string();
                                }
                                Ok(())
                            })();
                            if let Err(__e) = __vbr_event {
                                eprintln!("Error: {}", __e);
                            }
                        }
                    }
                    KeyCode::Char('k') => {
                        {
                            let __vbr_event: Result<(), String> = (|| {
                                state.worker.kill();
                                state.status = "worker killed".to_string();
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
