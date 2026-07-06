use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

struct Fetcher {
    url: String,
    status: String,
}

impl Default for Fetcher {
    fn default() -> Self {
        Fetcher {
            url: "https://api.github.com/zen".to_string(),
            status: "press r to fetch, q to quit".to_string(),
        }
    }
}

fn view(state: &Fetcher, frame: &mut Frame) {
    let block = Block::bordered().title("HTTP Fetch");
    let area = frame.area();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1)]).split(inner);
    frame.render_widget(Paragraph::new(format!(" URL: {}", state.url)), chunks_0[0]);
    frame.render_widget(Paragraph::new(" Press r to fetch, q to quit"), chunks_0[1]);
    frame.render_widget(Paragraph::new(format!("{}", state.status)), chunks_0[2]);
}

use vbr_stdlib::{Http};

enum Message {
    FetchDone(Result<String, String>),
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let mut state = Fetcher::default();
    let mut terminal = ratatui::init();
    let (tx, rx) = std::sync::mpsc::channel::<Message>();
    loop {
        terminal.draw(|frame| view(&state, frame))?;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Message::FetchDone(result) => {
                    match result {
                        Ok ( _ ) => {
                            state.status = "ok — page fetched".to_string();
                        }
                        Err ( e ) => {
                            state.status = format!("error: {}", e);
                        }
                    }
                }
            }
        }
        if !event::poll(std::time::Duration::from_millis(50))? {
            continue;
        }
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('r') => {
                        state.status = "loading…".to_string();
                        let url = state.url.clone();
                        let tx = tx.clone();
                        std::thread::spawn(move || {
                            let _ = tx.send(Message::FetchDone(Http::get(&url)));
                        });
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
