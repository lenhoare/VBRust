// tui_post.vbr — an awaited Http.Post in a terminal app.
// 
// Press Enter to POST a JSON body (with a Content-Type and a Bearer token) and
// show the reply, without freezing the screen — `Await` runs the request off the
// UI thread, exactly like `Await Http.Get`. This is the shape of an LLM call.

use vbr_stdlib::{Http};

use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
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
        let status = "Press Enter to send".to_string();
        let reply = "".to_string();
        let key = "sk-demo-key".to_string();
        let endpoint = "https://api.example.com/v1/complete".to_string();
        Poster {
            status,
            reply,
            key,
            endpoint,
        }
    }
}

fn view(state: &Poster, frame: &mut Frame) {
    let area = frame.area();
    let chunks_status = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(area);
    let block = Block::bordered().title("POST from a TUI");
    let inner = block.inner(chunks_status[0]);
    frame.render_widget(block, chunks_status[0]);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1)]).split(inner);
    frame.render_widget(Paragraph::new(format!("{}", state.status)), chunks_0[0]);
    frame.render_widget(Paragraph::new(format!("{}", state.reply)), chunks_0[1]);
    frame.render_widget(Paragraph::new("Enter to POST • q to quit"), chunks_0[2]);
    frame.render_widget(Paragraph::new(Line::from(vec![Span::raw(" "), Span::styled(" Enter ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" Send  "), Span::styled(" q ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" Quit  ")])).style(ratatui::style::Style::new().bg(ratatui::style::Color::Cyan).fg(ratatui::style::Color::Black)), chunks_status[1]);
}

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
                    {
                        let __vbr_event: Result<(), String> = (|| {
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
                            Ok(())
                        })();
                        if let Err(__e) = __vbr_event {
                            eprintln!("Error: {}", __e);
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
                        {
                            let __vbr_event: Result<(), String> = (|| {
                                state.status = "sending…".to_string();
                                let mut headers: HashMap<String, String> = HashMap::new();
                                headers.insert("Authorization".to_string(), format!("Bearer {}", state.key));
                                headers.insert("Content-Type".to_string(), "application/json".to_string());
                                let body: String = "{\"prompt\": \"hello\"}".to_string();
                                Ok(())
                            })();
                            if let Err(__e) = __vbr_event {
                                eprintln!("Error: {}", __e);
                            }
                        }
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
