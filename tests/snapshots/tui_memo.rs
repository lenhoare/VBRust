use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::Frame;

struct Scratch {
    notes: String,
    notes_state: tui_textarea::TextArea<'static>,
}

impl Default for Scratch {
    fn default() -> Self {
        let notes = "Type here. Enter makes a new line.".to_string();
        let notes_state = if notes.is_empty() { tui_textarea::TextArea::default() } else { tui_textarea::TextArea::from(notes.lines()) };
        Scratch {
            notes,
            notes_state,
        }
    }
}

fn view(state: &mut Scratch, frame: &mut Frame) {
    let area = frame.area();
    let chunks_status = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(area);
    let block = Block::bordered().title("Memo");
    let inner = block.inner(chunks_status[0]);
    frame.render_widget(block, chunks_status[0]);
    state.notes_state.set_block(Block::bordered().title("notes"));
    state.notes_state.set_cursor_style(ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED));
    state.notes_state.set_cursor_line_style(ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::UNDERLINED));
    frame.render_widget(&state.notes_state, inner);
    frame.render_widget(Paragraph::new(Line::from(vec![Span::raw(format!(" {}  ", format!("chars {}", state.notes.len()))), Span::styled(" Esc ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" quit  ")])).style(ratatui::style::Style::new().bg(ratatui::style::Color::Cyan).fg(ratatui::style::Color::Black)), chunks_status[1]);
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let mut state = Scratch::default();
    let mut terminal = ratatui::init();
    loop {
        terminal.draw(|frame| view(&mut state, frame))?;
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Esc => {
                        break;
                    }
                    _ => {
                        let _ = state.notes_state.input(key);
                        state.notes = state.notes_state.lines().join("\n");
                    }
                }
            }
        }
    }
    ratatui::restore();
    Ok(())
}
