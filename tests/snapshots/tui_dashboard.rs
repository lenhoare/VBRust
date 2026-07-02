#[derive(Debug, Clone)]
struct Bar {
    pub label: String,
    pub value: i32,
}

fn history() -> Vec<i32> {
    let mut v: Vec<i32> = Vec::new();
    v.push(3);
    v.push(7);
    v.push(4);
    v.push(9);
    v.push(6);
    v.push(8);
    v.push(5);
    v
}

fn sales() -> Vec<Bar> {
    let mut v: Vec<Bar> = Vec::new();
    v.push(Bar { label: "Jan".to_string(), value: 12 });
    v.push(Bar { label: "Feb".to_string(), value: 19 });
    v.push(Bar { label: "Mar".to_string(), value: 8 });
    v.push(Bar { label: "Apr".to_string(), value: 15 });
    v
}

use ratatui::widgets::{Block};
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

struct Dash {
    cpu: i32,
    history: Vec<i32>,
    sales: Vec<Bar>,
}

impl Default for Dash {
    fn default() -> Self {
        Dash {
            cpu: 62,
            history: history(),
            sales: sales(),
        }
    }
}

fn view(state: &Dash, frame: &mut Frame) {
    let block = Block::bordered().title("Dashboard");
    let area = frame.area();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let chunks_0 = Layout::vertical([Constraint::Length(3), Constraint::Length(8), Constraint::Fill(1)]).spacing(1).split(inner);
    let ratio_1 = ((state.cpu as f64 - 0 as f64) / (100 as f64 - 0 as f64)).clamp(0.0, 1.0);
    frame.render_widget(ratatui::widgets::Gauge::default().block(Block::bordered().title("cpu")).ratio(ratio_1), chunks_0[0]);
    let spark_2: Vec<u64> = state.history.iter().map(|&v| v as u64).collect();
    frame.render_widget(ratatui::widgets::Sparkline::default().block(Block::bordered().title("history")).data(&spark_2), chunks_0[1]);
    let bars_3: Vec<(&str, u64)> = state.sales.iter().map(|it| (it.label.as_str(), it.value as u64)).collect();
    frame.render_widget(ratatui::widgets::BarChart::default().block(Block::bordered().title("sales")).data(&bars_3).bar_width(7), chunks_0[2]);
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let state = Dash::default();
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
