use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
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
    let block = Block::bordered().title("VBR Terminal Counter");
    let area = frame.area();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)]).split(inner);
    frame.render_widget(Paragraph::new("A VBR terminal app"), chunks_0[0]);
    frame.render_widget(Paragraph::new(format!("Count: {}", state.count)), chunks_0[1]);
    frame.render_widget(Paragraph::new(""), chunks_0[2]);
    frame.render_widget(Paragraph::new("Press + / - to change, q to quit"), chunks_0[3]);
}

fn main() -> std::io::Result<()> {
    use ratzilla::{DomBackend, WebRenderer};
    use ratzilla::event::KeyCode;
    let state = std::rc::Rc::new(std::cell::RefCell::new(Counter::default()));
    let backend = DomBackend::new()?;
    let mut terminal = ratzilla::ratatui::Terminal::new(backend)?;
    terminal.on_key_event({
        let state = state.clone();
        move |key| {
            let mut guard = state.borrow_mut();
            let state = &mut *guard;
            match key.code {
                KeyCode::Char('+') => {
                    state.count += 1;
                }
                KeyCode::Char('-') => {
                    state.count -= 1;
                }
                _ => {}
            }
        }
    })?;
    terminal.draw_web(move |frame| view(&state.borrow(), frame));
    Ok(())
}
