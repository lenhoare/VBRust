use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

struct Tabs {
    tab: i32,
    busy: bool,
}

impl Default for Tabs {
    fn default() -> Self {
        Tabs {
            tab: 1,
            busy: false,
        }
    }
}

fn view(state: &Tabs, frame: &mut Frame) {
    let block = Block::bordered().title("Tabs");
    let area = frame.area();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1)]).split(inner);
    frame.render_widget(Paragraph::new(" 1/2/3 switch tab • b toggles busy • q quits"), chunks_0[0]);
    if state.busy {
        frame.render_widget(Paragraph::new(" ● working…"), chunks_0[1]);
    } else {
        frame.render_widget(Paragraph::new(" ○ idle"), chunks_0[1]);
    }
    match state.tab {
        1 => {
            let chunks_1 = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(chunks_0[2]);
            frame.render_widget(Paragraph::new("── Overview ──"), chunks_1[0]);
            frame.render_widget(Paragraph::new("Welcome to tab one."), chunks_1[1]);
        }
        2 => {
            let chunks_2 = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(chunks_0[2]);
            frame.render_widget(Paragraph::new("── Details ──"), chunks_2[0]);
            frame.render_widget(Paragraph::new("Tab two has the details."), chunks_2[1]);
        }
        _ => {
            frame.render_widget(Paragraph::new("── Settings (tab 3) ──"), chunks_0[2]);
        }
    }
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let mut state = Tabs::default();
    let mut terminal = ratatui::init();
    loop {
        terminal.draw(|frame| view(&state, frame))?;
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('1') => {
                        state.tab = 1;
                    }
                    KeyCode::Char('2') => {
                        state.tab = 2;
                    }
                    KeyCode::Char('3') => {
                        state.tab = 3;
                    }
                    KeyCode::Char('b') => {
                        state.busy = !state.busy;
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
