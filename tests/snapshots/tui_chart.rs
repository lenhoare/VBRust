#[derive(Debug, Clone)]
struct Point {
    pub x: f64,
    pub y: f64,
}

fn curve() -> Vec<Point> {
    let mut v: Vec<Point> = Vec::new();
    for x in 0..=20 {
        let xd: f64 = x as f64;
        v.push(Point { x: xd, y: xd * xd / 10.0 });
    }
    v
}

use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

struct Plot {
    curve: Vec<Point>,
}

impl Default for Plot {
    fn default() -> Self {
        Plot {
            curve: curve(),
        }
    }
}

fn view(state: &Plot, frame: &mut Frame) {
    let block = Block::bordered().title("y = x² / 10");
    let area = frame.area();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).split(inner);
    frame.render_widget(Paragraph::new(" An X/Y line chart of computed points — q to quit"), chunks_0[0]);
    let pts_1: Vec<(f64, f64)> = state.curve.iter().map(|p| (p.x as f64, p.y as f64)).collect();
    let xlo_1 = pts_1.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let xhi_1 = pts_1.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    let (xlo_1, xhi_1) = if xlo_1 <= xhi_1 { (xlo_1, xhi_1) } else { (0.0, 1.0) };
    let ylo_1 = pts_1.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let yhi_1 = pts_1.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    let (ylo_1, yhi_1) = if ylo_1 <= yhi_1 { (ylo_1, yhi_1) } else { (0.0, 1.0) };
    let dataset_1 = ratatui::widgets::Dataset::default().marker(ratatui::symbols::Marker::Braille).graph_type(ratatui::widgets::GraphType::Line).style(ratatui::style::Style::new().fg(ratatui::style::Color::Cyan)).data(&pts_1);
    let chart_1 = ratatui::widgets::Chart::new(vec![dataset_1]).block(Block::bordered().title("curve"))
        .x_axis(ratatui::widgets::Axis::default().bounds([xlo_1, xhi_1]).labels(vec![format!("{:.1}", xlo_1), format!("{:.1}", xhi_1)]))
        .y_axis(ratatui::widgets::Axis::default().bounds([ylo_1, yhi_1]).labels(vec![format!("{:.1}", ylo_1), format!("{:.1}", yhi_1)]));
    frame.render_widget(chart_1, chunks_0[1]);
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let state = Plot::default();
    let mut terminal = ratatui::init();
    loop {
        terminal.draw(|frame| view(&state, frame))?;
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
