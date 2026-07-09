// tui_post.vbr — an awaited Http.Post in a terminal app.
// 
// Press Enter to POST a JSON body (with a Content-Type and a Bearer token) and
// show the reply, without freezing the screen — `Await` runs the request off the
// UI thread, exactly like `Await Http.Get`. This is the shape of an LLM call.

use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;
use std::collections::HashMap;

struct Poster {
    status: String,
    reply: String,
    key: String,
    endpoint: String,
}

impl Default for Poster {
    fn default() -> Self {
        Poster {
            status: "Press Enter to send".to_string(),
            reply: "".to_string(),
            key: "sk-demo-key".to_string(),
            endpoint: "https://api.example.com/v1/complete".to_string(),
        }
    }
}

fn view(state: &Poster, frame: &mut Frame) {
    let block = Block::bordered().title("POST from a TUI");
    let area = frame.area();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1)]).split(inner);
    frame.render_widget(Paragraph::new(format!("{}", state.status)), chunks_0[0]);
    frame.render_widget(Paragraph::new(format!("{}", state.reply)), chunks_0[1]);
    frame.render_widget(Paragraph::new("Enter to POST • q to quit"), chunks_0[2]);
}

use vbr_stdlib::{Http};

enum Message {
    SendDone(Result<String, String>),
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let mut state = Poster::default();
    let mut terminal = ratatui::init();
    let (tx, rx) = std::sync::mpsc::channel::<Message>();
    loop {
        terminal.draw(|frame| view(&state, frame))?;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Message::SendDone(result) => {
                    match result {
                        Ok ( text ) => {
                            state.status = "ok".to_string();
                            state.reply = text;
                        }
                        Err ( message ) => {
                            state.status = "failed".to_string();
                            state.reply = message;
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
                    KeyCode::Enter => {
                        state.status = "sending…".to_string();
                        let mut headers: HashMap<String, String> = HashMap::new();
                        headers.insert("Authorization".to_string(), format!("Bearer {}", state.key));
                        headers.insert("Content-Type".to_string(), "application/json".to_string());
                        let body: String = "{\"prompt\": \"hello\"}".to_string();
                        let endpoint = state.endpoint.clone();
                        let tx = tx.clone();
                        std::thread::spawn(move || {
                            let _ = tx.send(Message::SendDone(Http::post(&endpoint, &body, headers)));
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
