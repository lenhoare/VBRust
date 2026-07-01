use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::Frame;

struct Dashboard {
    tab: i32,
}

impl Default for Dashboard {
    fn default() -> Self {
        Dashboard {
            tab: 1,
        }
    }
}

fn view(state: &Dashboard, frame: &mut Frame) {
    let block = Block::bordered().title("VBR TUI Layout");
    let area = frame.area();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Fill(1), Constraint::Length(1)]).split(inner);
    frame.render_widget(Paragraph::new("  Dashboard — press 1/2/3 to switch tab, q to quit"), chunks_0[0]);
    let chunks_1 = Layout::horizontal([Constraint::Percentage(30), Constraint::Fill(1)]).spacing(1).split(chunks_0[1]);
    let chunks_2 = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)]).split(chunks_1[0]);
    frame.render_widget(Paragraph::new(" Sidebar"), chunks_2[0]);
    frame.render_widget(Paragraph::new(" - Overview"), chunks_2[1]);
    frame.render_widget(Paragraph::new(" - Details"), chunks_2[2]);
    frame.render_widget(Paragraph::new(" - Settings"), chunks_2[3]);
    let chunks_3 = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)]).split(chunks_1[1]);
    frame.render_widget(Paragraph::new(" Main panel"), chunks_3[0]);
    frame.render_widget(Paragraph::new(""), chunks_3[1]);
    frame.render_widget(Paragraph::new(format!("{}{}", "Active tab: ", state.tab)), chunks_3[2]);
    frame.render_widget(Paragraph::new("  status: ok"), chunks_0[2]);
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let mut state = Dashboard::default();
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
