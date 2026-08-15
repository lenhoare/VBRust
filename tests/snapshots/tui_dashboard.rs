#[derive(Debug, Clone)]
struct Bar {
    pub label: String,
    pub value: i32,
}

fn history() -> Result<Vec<i32>, String> {
    let mut v: Vec<i32> = Vec::new();
    v.push(3);
    v.push(7);
    v.push(4);
    v.push(9);
    v.push(6);
    v.push(8);
    v.push(5);
    Ok(v)
}

fn sales() -> Result<Vec<Bar>, String> {
    let mut v: Vec<Bar> = Vec::new();
    v.push(Bar { label: "Jan".to_string(), value: 12 });
    v.push(Bar { label: "Feb".to_string(), value: 19 });
    v.push(Bar { label: "Mar".to_string(), value: 8 });
    v.push(Bar { label: "Apr".to_string(), value: 15 });
    Ok(v)
}

use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::Frame;

struct Dash {
    cpu: i32,
    history: Vec<i32>,
    sales: Vec<Bar>,
}

impl Dash {
    fn init() -> Result<Dash, String> {
        let cpu = 62;
        let history = history()?;
        let sales = sales()?;
        Ok(Dash {
            cpu,
            history,
            sales,
        })
    }
}

fn view(state: &Dash, frame: &mut Frame) {
    let area = frame.area();
    let chunks_status = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(area);
    let block = Block::bordered().title("Dashboard");
    let inner = block.inner(chunks_status[0]);
    frame.render_widget(block, chunks_status[0]);
    let chunks_0 = Layout::vertical([Constraint::Length(3), Constraint::Length(8), Constraint::Fill(1)]).spacing(1).split(inner);
    let ratio_1 = ((state.cpu as f64 - 0 as f64) / (100 as f64 - 0 as f64)).clamp(0.0, 1.0);
    frame.render_widget(ratatui::widgets::Gauge::default().block(Block::bordered().title("cpu")).ratio(ratio_1), chunks_0[0]);
    let spark_2: Vec<u64> = state.history.iter().map(|&v| v as u64).collect();
    frame.render_widget(ratatui::widgets::Sparkline::default().block(Block::bordered().title("history")).data(&spark_2), chunks_0[1]);
    let bars_3: Vec<(&str, u64)> = state.sales.iter().map(|it| (it.label.as_str(), it.value as u64)).collect();
    frame.render_widget(ratatui::widgets::BarChart::default().block(Block::bordered().title("sales")).data(&bars_3).bar_width(7), chunks_0[2]);
    frame.render_widget(Paragraph::new(Line::from(vec![Span::raw(" "), Span::styled(" Esc ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" Quit  ")])).style(ratatui::style::Style::new().bg(ratatui::style::Color::Cyan).fg(ratatui::style::Color::Black)), chunks_status[1]);
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let state = match Dash::init() {
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
                    KeyCode::Esc => {
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
