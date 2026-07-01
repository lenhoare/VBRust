use ratatui::widgets::{Block, Paragraph};
use ratatui::text::Line;
use ratatui::Frame;

struct Counter {
    count: i32,
}

impl Default for Counter {
    fn default() -> Self {
        Counter {
            count: 0,
        }
    }
}

fn view(state: &Counter, frame: &mut Frame) {
    let lines: Vec<Line> = vec![
        Line::from("A VBR terminal app"),
        Line::from(format!("{}{}", "Count: ", state.count)),
        Line::from(""),
        Line::from("Press + / - to change, q to quit"),
    ];
    let block = Block::bordered().title("VBR Terminal Counter");
    frame.render_widget(Paragraph::new(lines).block(block), frame.area());
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let mut state = Counter::default();
    let mut terminal = ratatui::init();
    loop {
        terminal.draw(|frame| view(&state, frame))?;
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('+') => {
                        state.count += 1;
                    }
                    KeyCode::Char('-') => {
                        state.count -= 1;
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
