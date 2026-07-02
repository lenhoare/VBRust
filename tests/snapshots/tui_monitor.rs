use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

struct Monitor {
    seconds: i32,
    status: String,
    url: String,
}

impl Default for Monitor {
    fn default() -> Self {
        Monitor {
            seconds: 0,
            status: "starting…".to_string(),
            url: "https://example.com".to_string(),
        }
    }
}

fn view(state: &Monitor, frame: &mut Frame) {
    let block = Block::bordered().title("Auto-refresh Monitor");
    let area = frame.area();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1)]).split(inner);
    frame.render_widget(Paragraph::new(format!("{}{}", format!("{}{}", "Uptime: ", state.seconds), "s")), chunks_0[0]);
    frame.render_widget(Paragraph::new(format!("{}", state.status)), chunks_0[1]);
    frame.render_widget(Paragraph::new("ticks every 1s, refreshes every 5s • q to quit"), chunks_0[2]);
}

use vbr_stdlib::{Http};

enum Message {
    PollDone(Result<String, String>),
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let mut state = Monitor::default();
    let mut terminal = ratatui::init();
    let (tx, rx) = std::sync::mpsc::channel::<Message>();
    let mut last_tick_0 = std::time::Instant::now();
    let mut last_tick_1 = std::time::Instant::now();
    loop {
        terminal.draw(|frame| view(&state, frame))?;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Message::PollDone(result) => {
                    match result {
                        Ok ( _ ) => {
                            state.status = format!("{}{}", format!("{}{}", "ok at ", state.seconds), "s");
                        }
                        Err ( e ) => {
                            state.status = format!("{}{}", "error: ", e);
                        }
                    }
                }
            }
        }
        if last_tick_0.elapsed().as_millis() >= 1000 {
            state.seconds += 1;
            last_tick_0 = std::time::Instant::now();
        }
        if last_tick_1.elapsed().as_millis() >= 5000 {
            state.status = "refreshing…".to_string();
            let url = state.url.clone();
            let tx = tx.clone();
            std::thread::spawn(move || {
                let _ = tx.send(Message::PollDone(Http::get(&url)));
            });
            last_tick_1 = std::time::Instant::now();
        }
        if !event::poll(std::time::Duration::from_millis(50))? {
            continue;
        }
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
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
