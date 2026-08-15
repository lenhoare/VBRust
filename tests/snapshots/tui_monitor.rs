use vbr_stdlib::{Http};

use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::Frame;

struct Monitor {
    seconds: i32,
    status: String,
    url: String,
}

impl Default for Monitor {
    fn default() -> Self {
        let seconds = 0;
        let status = "starting…".to_string();
        let url = "https://api.github.com/zen".to_string();
        Monitor {
            seconds,
            status,
            url,
        }
    }
}

fn view(state: &Monitor, frame: &mut Frame) {
    let area = frame.area();
    let chunks_status = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(area);
    let block = Block::bordered().title("Auto-refresh Monitor");
    let inner = block.inner(chunks_status[0]);
    frame.render_widget(block, chunks_status[0]);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1)]).split(inner);
    frame.render_widget(Paragraph::new(format!("Uptime: {}s", state.seconds)), chunks_0[0]);
    frame.render_widget(Paragraph::new(format!("{}", state.status)), chunks_0[1]);
    frame.render_widget(Paragraph::new("ticks every 1s, refreshes every 5s • q to quit"), chunks_0[2]);
    frame.render_widget(Paragraph::new(Line::from(vec![Span::raw(" "), Span::styled(" q ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" Quit  ")])).style(ratatui::style::Style::new().bg(ratatui::style::Color::Cyan).fg(ratatui::style::Color::Black)), chunks_status[1]);
}

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
                    {
                        let __vbr_event: Result<(), String> = (|| {
                            match result {
                                Ok ( _ ) => {
                                    state.status = format!("ok at {}s", state.seconds);
                                }
                                Err ( e ) => {
                                    state.status = format!("error: {}", e);
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
        if last_tick_0.elapsed().as_millis() >= 1000 {
            {
                let __vbr_event: Result<(), String> = (|| {
                    state.seconds += 1;
                    Ok(())
                })();
                if let Err(__e) = __vbr_event {
                    eprintln!("Error: {}", __e);
                }
            }
            last_tick_0 = std::time::Instant::now();
        }
        if last_tick_1.elapsed().as_millis() >= 5000 {
            {
                let __vbr_event: Result<(), String> = (|| {
                    state.status = "refreshing…".to_string();
                    Ok(())
                })();
                if let Err(__e) = __vbr_event {
                    eprintln!("Error: {}", __e);
                }
            }
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
