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

fn main() -> std::io::Result<()> {
    use ratzilla::{DomBackend, WebRenderer};
    let state = std::rc::Rc::new(std::cell::RefCell::new(Monitor::default()));
    let backend = DomBackend::new()?;
    let terminal = ratzilla::ratatui::Terminal::new(backend)?;
    gloo_timers::callback::Interval::new(1000, {
        let state = state.clone();
        move || {
            let mut guard = state.borrow_mut();
            let state = &mut *guard;
            {
                let __vbr_event: Result<(), String> = (|| {
                    state.seconds += 1;
                    Ok(())
                })();
                if let Err(__e) = __vbr_event {
                    eprintln!("Error: {}", __e);
                }
            }
        }
    })
    .forget();
    gloo_timers::callback::Interval::new(5000, {
        let state = state.clone();
        move || {
            let rc = state.clone();
            let mut guard = state.borrow_mut();
            let state = &mut *guard;
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
            wasm_bindgen_futures::spawn_local({
                let state = rc.clone();
                async move {
                    let result = http_get(&url).await;
                    let mut guard = state.borrow_mut();
                    let state = &mut *guard;
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
            });
        }
    })
    .forget();
    terminal.draw_web(move |frame| view(&state.borrow(), frame));
    Ok(())
}

/// The browser's `fetch`, shaped like the stdlib's `Http.Get`: the response
/// body on success; any failure (network, CORS, an HTTP error status) as a
/// `String` error.
async fn http_get(url: &str) -> Result<String, String> {
    let response = gloo_net::http::Request::get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }
    response.text().await.map_err(|e| e.to_string())
}

