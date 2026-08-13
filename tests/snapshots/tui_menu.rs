use ratatui::widgets::{Block, Paragraph, Clear};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::Frame;

struct App {
    log: String,
    menu_open: Option<usize>,
    menu_sel: usize,
}

impl Default for App {
    fn default() -> Self {
        let log = "F10, then Enter on Quit — or Alt+F / Alt+H.".to_string();
        App {
            log,
            menu_open: None,
            menu_sel: 0,
        }
    }
}

impl App {
    fn menu_activate(&mut self, i: usize) {
        self.menu_open = Some(i);
        self.menu_sel = Self::menu_first_item(i);
    }
    fn menu_close(&mut self) {
        self.menu_open = None;
        self.menu_sel = 0;
    }
    fn menu_len(menu: usize) -> usize {
        match menu {
            0 => 3,
            1 => 1,
            _ => 0,
        }
    }
    fn menu_is_sep(menu: usize, item: usize) -> bool {
        match (menu, item) {
            (0, 1) => true,
            _ => false,
        }
    }
    fn menu_first_item(menu: usize) -> usize {
        let n = Self::menu_len(menu);
        (0..n).find(|&i| !Self::menu_is_sep(menu, i)).unwrap_or(0)
    }
    fn menu_next(&mut self, delta: isize) {
        let n = 2isize;
        let cur = self.menu_open.unwrap_or(0) as isize;
        let mut i = cur + delta;
        while i < 0 {
            i += n;
        }
        self.menu_activate((i % n) as usize);
    }
    fn menu_move_sel(&mut self, delta: isize) {
        let Some(m) = self.menu_open else { return };
        let n = Self::menu_len(m) as isize;
        if n == 0 {
            return;
        }
        let mut s = self.menu_sel as isize;
        for _ in 0..n {
            s += delta;
            while s < 0 {
                s += n;
            }
            s %= n;
            if !Self::menu_is_sep(m, s as usize) {
                self.menu_sel = s as usize;
                return;
            }
        }
    }
}

fn view(state: &App, frame: &mut Frame) {
    let area = frame.area();
    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Fill(1), Constraint::Length(1)]).split(area);
    let menu_area = chunks[0];
    let menu_style = ratatui::style::Style::new().bg(ratatui::style::Color::Cyan).fg(ratatui::style::Color::Black);
    let mut menu_spans = Vec::new();
    menu_spans.push(Span::styled(" File ", if state.menu_open == Some(0) { menu_style.add_modifier(ratatui::style::Modifier::REVERSED) } else { menu_style }));
    menu_spans.push(Span::styled(" Help ", if state.menu_open == Some(1) { menu_style.add_modifier(ratatui::style::Modifier::REVERSED) } else { menu_style }));
    frame.render_widget(Paragraph::new(Line::from(menu_spans)).style(menu_style), menu_area);
    let view_area = chunks[1];
    let block = Block::bordered().title("Menu");
    let inner = block.inner(view_area);
    frame.render_widget(block, view_area);
    let chunks_0 = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).split(inner);
    frame.render_widget(Paragraph::new(format!("{}", state.log)), chunks_0[0]);
    frame.render_widget(Paragraph::new("Arrows move. Esc closes the menu."), chunks_0[1]);
    let status_area = chunks[2];
    frame.render_widget(Paragraph::new(Line::from(vec![Span::raw(format!(" {}  ", "F10 opens the bar")), Span::styled(" Esc ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" quit  "), Span::styled(" F10 ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" menu  ")])).style(ratatui::style::Style::new().bg(ratatui::style::Color::Cyan).fg(ratatui::style::Color::Black)), status_area);
    if let Some(open) = state.menu_open {
        let labels: [&str; 2] = [" File ", " Help "];
        let mut x = menu_area.x;
        for (i, lab) in labels.iter().enumerate() {
            if i == open {
                break;
            }
            x = x.saturating_add(lab.len() as u16);
        }
        match open {
            0 => {
                let items: [&str; 3] = [" Beep", "────────", " Quit"];
                let width = items.iter().map(|s| s.len()).max().unwrap_or(10).max(14) as u16 + 4;
                let height = items.len() as u16 + 2;
                let rect = Rect {
                    x,
                    y: menu_area.y.saturating_add(1),
                    width: width.min(menu_area.width.saturating_sub(x.saturating_sub(menu_area.x))),
                    height,
                };
                frame.render_widget(Clear, rect);
                let block = Block::bordered().style(menu_style);
                let inner = block.inner(rect);
                frame.render_widget(block, rect);
                for (i, label) in items.iter().enumerate() {
                    let style = if i == state.menu_sel { menu_style.add_modifier(ratatui::style::Modifier::REVERSED) } else { menu_style };
                    let row = Rect {
                        x: inner.x,
                        y: inner.y.saturating_add(i as u16),
                        width: inner.width,
                        height: 1,
                    };
                    frame.render_widget(Paragraph::new(*label).style(style), row);
                }
            }
            1 => {
                let items: [&str; 1] = [" About"];
                let width = items.iter().map(|s| s.len()).max().unwrap_or(10).max(14) as u16 + 4;
                let height = items.len() as u16 + 2;
                let rect = Rect {
                    x,
                    y: menu_area.y.saturating_add(1),
                    width: width.min(menu_area.width.saturating_sub(x.saturating_sub(menu_area.x))),
                    height,
                };
                frame.render_widget(Clear, rect);
                let block = Block::bordered().style(menu_style);
                let inner = block.inner(rect);
                frame.render_widget(block, rect);
                for (i, label) in items.iter().enumerate() {
                    let style = if i == state.menu_sel { menu_style.add_modifier(ratatui::style::Modifier::REVERSED) } else { menu_style };
                    let row = Rect {
                        x: inner.x,
                        y: inner.y.saturating_add(i as u16),
                        width: inner.width,
                        height: 1,
                    };
                    frame.render_widget(Paragraph::new(*label).style(style), row);
                }
            }
            _ => {}
        }
    }
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
    let mut state = App::default();
    let mut terminal = ratatui::init();
    loop {
        terminal.draw(|frame| view(&state, frame))?;
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                if state.menu_open.is_some() {
                    match key.code {
                        KeyCode::Esc | KeyCode::F(10) => state.menu_close(),
                        KeyCode::Left => state.menu_next(-1),
                        KeyCode::Right => state.menu_next(1),
                        KeyCode::Up => state.menu_move_sel(-1),
                        KeyCode::Down => state.menu_move_sel(1),
                        KeyCode::Enter => {
                            if let Some(m) = state.menu_open {
                                let i = state.menu_sel;
                                state.menu_close();
                                match (m, i) {
                                    (0, 0) => {
                                        state.log = "File → Beep".to_string();
                                    }
                                    (0, 2) => {
                                        break;
                                    }
                                    (1, 0) => {
                                        state.log = "Help → About".to_string();
                                    }
                                    _ => {}
                                }
                            }
                        }
                        KeyCode::Char(c) => match (state.menu_open, c.to_ascii_lowercase()) {
                            (Some(0), 'b') => {
                                state.menu_close();
                                state.log = "File → Beep".to_string();
                            }
                            (Some(0), 'q') => {
                                state.menu_close();
                                break;
                            }
                            (Some(1), 'a') => {
                                state.menu_close();
                                state.log = "Help → About".to_string();
                            }
                            (_, 'f') => state.menu_activate(0),
                            (_, 'h') => state.menu_activate(1),
                            _ => {}
                        }
                        _ => {}
                    }
                } else {
                match key.code {
                    KeyCode::F(10) => state.menu_activate(0),
                    KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::ALT) => {
                        match c.to_ascii_lowercase() {
                            'f' => state.menu_activate(0),
                            'h' => state.menu_activate(1),
                            _ => {}
                        }
                    }
                    KeyCode::Esc => {
                        break;
                    }
                    _ => {}
                }
                }
            }
        }
    }
    ratatui::restore();
    Ok(())
}
