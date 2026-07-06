// Timers drive the state: Every 100ms the gauge pulses up or down, and each
// full beat lands in the sparkline's history. No keys needed — the app animates
// itself. The same file runs in the terminal (vbr runproject) and in the
// browser (vbr runweb). (tui web slice 3)

use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

struct Pulse {
    level: i32,
    rising: bool,
    beats: i32,
    history: Vec<i32>,
}

impl Default for Pulse {
    fn default() -> Self {
        Pulse {
            level: 0,
            rising: true,
            beats: 0,
            history: Vec::new(),
        }
    }
}

fn view(state: &Pulse, frame: &mut Frame) {
    let block = Block::bordered().title("Pulse");
    let area = frame.area();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Length(3), Constraint::Fill(1)]).split(inner);
    frame.render_widget(Paragraph::new(format!("Beats: {} • q to quit", state.beats)), chunks_0[0]);
    let ratio_1 = ((state.level as f64 - 0 as f64) / (100 as f64 - 0 as f64)).clamp(0.0, 1.0);
    frame.render_widget(ratatui::widgets::Gauge::default().block(Block::bordered().title("level")).ratio(ratio_1), chunks_0[1]);
    let spark_2: Vec<u64> = state.history.iter().map(|&v| v as u64).collect();
    frame.render_widget(ratatui::widgets::Sparkline::default().block(Block::bordered().title("history")).data(&spark_2), chunks_0[2]);
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let mut state = Pulse::default();
    let mut terminal = ratatui::init();
    let mut last_tick_0 = std::time::Instant::now();
    loop {
        terminal.draw(|frame| view(&state, frame))?;
        if last_tick_0.elapsed().as_millis() >= 100 {
            if state.rising {
                state.level += 5;
                if state.level >= 100 {
                    state.rising = false;
                    state.beats += 1;
                    state.history.push(state.level);
                }
            } else {
                state.level -= 5;
                if state.level <= 0 {
                    state.rising = true;
                }
            }
            last_tick_0 = std::time::Instant::now();
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
