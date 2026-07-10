mod life;

// A Screen driving logic that lives in another file — the shape a real project
// wants: main.vbr owns the terminal, life.vbr owns the rules. Cross-module
// calls work from State initialisers and events alike, with the same argument
// treatment as local calls (`Life.SetCell(grid, …)` borrows `&mut state.grid`).
// One current limit: a *view* expression can't read `Life.WIDTH` directly —
// mirror it into state or read it through an event (see projects/vbr_gaps.md).

use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

struct LifeLab {
    grid: Vec<i64>,
    living: i64,
    status: String,
}

impl Default for LifeLab {
    fn default() -> Self {
        LifeLab {
            grid: crate::life::newgrid(),
            living: 0,
            status: "ready".to_string(),
        }
    }
}

fn view(state: &LifeLab, frame: &mut Frame) {
    let block = Block::bordered().title("Life Lab");
    let area = frame.area();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)]).split(inner);
    frame.render_widget(Paragraph::new(format!("Living: {}", state.living)), chunks_0[0]);
    frame.render_widget(Paragraph::new(format!("{}", state.status)), chunks_0[1]);
    frame.render_widget(Paragraph::new(""), chunks_0[2]);
    frame.render_widget(Paragraph::new("s = seed a blinker, c = count, q = quit"), chunks_0[3]);
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let mut state = LifeLab::default();
    let mut terminal = ratatui::init();
    loop {
        terminal.draw(|frame| view(&state, frame))?;
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('s') => {
                        crate::life::setcell(&mut state.grid, 1, 1, 1);
                        crate::life::setcell(&mut state.grid, 2, 1, 1);
                        crate::life::setcell(&mut state.grid, 3, 1, 1);
                        state.status = format!("seeded {}", crate::life::formatrule("3", "23"));
                    }
                    KeyCode::Char('c') => {
                        state.living = crate::life::countlive(&state.grid);
                        state.status = "counted".to_string();
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
