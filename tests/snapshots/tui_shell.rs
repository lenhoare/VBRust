// A background process behind a Screen — the local-server shape: start a
// long-running command when the app boots (a fallible State initialiser, so a
// failed launch stops the program cleanly before the terminal opens), check on
// it from events, and stop it from a key. The child is detached from the
// terminal, so it can't scribble over the UI.

use vbr_stdlib::{Shell, Process};

use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

struct ProcessPanel {
    worker: Process,
    status: String,
}

impl ProcessPanel {
    fn init() -> Result<ProcessPanel, String> {
        Ok(ProcessPanel {
            worker: Shell::start("sleep 300")?,
            status: "worker started".to_string(),
        })
    }
}

fn view(state: &ProcessPanel, frame: &mut Frame) {
    let block = Block::bordered().title("Process Panel");
    let area = frame.area();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)]).split(inner);
    frame.render_widget(Paragraph::new(format!("{}", state.status)), chunks_0[0]);
    frame.render_widget(Paragraph::new(""), chunks_0[1]);
    frame.render_widget(Paragraph::new("c = check on it, k = kill it, q = quit"), chunks_0[2]);
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
                        if state.worker.is_running() {
                            state.status = "worker is running".to_string();
                        } else {
                            state.status = "worker has stopped".to_string();
                        }
                    }
                    KeyCode::Char('k') => {
                        state.worker.kill();
                        state.status = "worker killed".to_string();
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
