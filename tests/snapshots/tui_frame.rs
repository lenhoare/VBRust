// A Screen using Frame panels (titled borders) and Space gaps.

fn leftitems() -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    v.push("Alpha".to_string());
    v.push("Beta".to_string());
    v
}

fn rightitems() -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    v.push("One".to_string());
    v.push("Two".to_string());
    v
}

use ratatui::widgets::{Block, Paragraph};
use ratatui::layout::{Constraint, Layout};
use ratatui::text::{Line, Span};
use ratatui::Frame;

struct Framed {
    left: Vec<String>,
    right: Vec<String>,
    log: String,
    left_state: ratatui::widgets::ListState,
    right_state: ratatui::widgets::ListState,
    focus_index: usize,
}

impl Default for Framed {
    fn default() -> Self {
        let left = leftitems();
        let right = rightitems();
        let log = "(nothing picked yet)".to_string();
        Framed {
            left,
            right,
            log,
            left_state: ratatui::widgets::ListState::default().with_selected(Some(0)),
            right_state: ratatui::widgets::ListState::default().with_selected(Some(0)),
            focus_index: 0,
        }
    }
}

fn view(state: &mut Framed, frame: &mut Frame) {
    let area = frame.area();
    let chunks_status = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(area);
    let block = Block::bordered().title("Frames");
    let inner = block.inner(chunks_status[0]);
    frame.render_widget(block, chunks_status[0]);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Fill(1), Constraint::Length(1)]).split(inner);
    frame.render_widget(Paragraph::new(" Tab switches list, Enter picks, q quits"), chunks_0[0]);
    let chunks_1 = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).spacing(1).split(chunks_0[2]);
    let block_2 = Block::bordered().title("Left");
    let inner_2 = block_2.inner(chunks_1[0]);
    frame.render_widget(block_2, chunks_1[0]);
    let items_3: Vec<ratatui::widgets::ListItem> = state.left.iter().map(|s| ratatui::widgets::ListItem::new(s.clone())).collect();
    let list_3 = ratatui::widgets::List::new(items_3).highlight_symbol("» ").highlight_style(ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED));
    frame.render_stateful_widget(list_3, inner_2, &mut state.left_state);
    let block_4 = Block::bordered().title("Right");
    let inner_4 = block_4.inner(chunks_1[1]);
    frame.render_widget(block_4, chunks_1[1]);
    let items_5: Vec<ratatui::widgets::ListItem> = state.right.iter().map(|s| ratatui::widgets::ListItem::new(s.clone())).collect();
    let list_5 = ratatui::widgets::List::new(items_5).highlight_symbol("» ").highlight_style(ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED));
    frame.render_stateful_widget(list_5, inner_4, &mut state.right_state);
    frame.render_widget(Paragraph::new(format!(" Last pick: {}", state.log)), chunks_0[3]);
    frame.render_widget(Paragraph::new(Line::from(vec![Span::raw(" "), Span::styled(" q ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" Quit  "), Span::styled(" Tab ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" focus  "), Span::styled(" Up/Down ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" move  "), Span::styled(" Enter ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" ok  ")])).style(ratatui::style::Style::new().bg(ratatui::style::Color::Cyan).fg(ratatui::style::Color::Black)), chunks_status[1]);
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    let mut state = Framed::default();
    let mut terminal = ratatui::init();
    loop {
        terminal.draw(|frame| view(&mut state, frame))?;
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => {
                        break;
                    }
                    KeyCode::Tab => {
                        state.focus_index = (state.focus_index + 1) % 2;
                    }
                    KeyCode::Down => {
                        match state.focus_index {
                            0 => state.left_state.select_next(),
                            1 => state.right_state.select_next(),
                            _ => {}
                        }
                    }
                    KeyCode::Up => {
                        match state.focus_index {
                            0 => state.left_state.select_previous(),
                            1 => state.right_state.select_previous(),
                            _ => {}
                        }
                    }
                    KeyCode::Enter => {
                        match state.focus_index {
                            0 => {
                                if let Some(i) = state.left_state.selected() {
                                    let item = state.left[i].clone();
                                    state.log = format!("left / {}", item);
                                }
                            }
                            1 => {
                                if let Some(i) = state.right_state.selected() {
                                    let item = state.right[i].clone();
                                    state.log = format!("right / {}", item);
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    ratatui::restore();
    Ok(())
}
