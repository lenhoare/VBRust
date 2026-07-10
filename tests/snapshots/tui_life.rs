// A miniature Game of Life screen. Two things live here:
// - loops inside an Event — the state rewrite recurses into For / For Each /
// Do bodies, so `grid[i]` becomes `state.grid[i]` at any depth;
// - a State field initialised by *calling* a helper — the initialiser runs
// the same resolver pass as a function body, so `CountLive(SeedGrid())`
// borrows its ByVal Vec argument exactly as it would anywhere else.

fn seedgrid() -> Vec<i64> {
    vec![0, 1, 0, 1, 0, 1, 0, 1, 0]
}

fn countlive(grid: &Vec<i64>) -> i64 {
    let mut total: i64 = 0;
    for cell in &*grid {
        total = total + *cell;
    }
    total
}

use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

struct Life {
    grid: Vec<i64>,
    living: i64,
    ticks: i64,
}

impl Default for Life {
    fn default() -> Self {
        Life {
            grid: seedgrid(),
            living: countlive(&seedgrid()),
            ticks: 0,
        }
    }
}

fn view(state: &Life, frame: &mut Frame) {
    let block = Block::bordered().title("VBR Life");
    let area = frame.area();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)]).split(inner);
    frame.render_widget(Paragraph::new(format!("Living: {}", state.living)), chunks_0[0]);
    frame.render_widget(Paragraph::new(format!("Ticks: {}", state.ticks)), chunks_0[1]);
    frame.render_widget(Paragraph::new(""), chunks_0[2]);
    frame.render_widget(Paragraph::new("Press s to step, q to quit"), chunks_0[3]);
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let mut state = Life::default();
    let mut terminal = ratatui::init();
    loop {
        terminal.draw(|frame| view(&state, frame))?;
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('s') => {
                        for i in 0..=8 {
                            state.grid[(i) as usize] = 1 - state.grid[(i) as usize];
                        }
                        state.living = 0;
                        for cell in &state.grid {
                            state.living = state.living + *cell;
                        }
                        state.ticks = state.ticks + 1;
                        while state.ticks > 99 {
                            state.ticks = state.ticks - 100;
                        }
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
