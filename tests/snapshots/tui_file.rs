// GetOpenFilename / GetSaveAsFilename — VBA-style path prompts on a Screen.
// Tab completes, Enter opens a file or enters a folder (Save As returns even
// a new name), Esc cancels and returns "". Then FileSystem.Read / Write.

use ratatui::widgets::{Block, Paragraph, Clear};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::Frame;

struct Scratch {
    notes: String,
    path: String,
    notes_state: tui_textarea::TextArea<'static>,
    menu_open: Option<usize>,
    menu_sel: usize,
}

impl Default for Scratch {
    fn default() -> Self {
        let notes = "F10 → File → Open, or Save As. Esc quits.".to_string();
        let path = "".to_string();
        let notes_state = if notes.is_empty() { tui_textarea::TextArea::default() } else { tui_textarea::TextArea::from(notes.lines()) };
        Scratch {
            notes,
            path,
            notes_state,
            menu_open: None,
            menu_sel: 0,
        }
    }
}

impl Scratch {
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
            0 => 4,
            _ => 0,
        }
    }
    fn menu_is_sep(menu: usize, item: usize) -> bool {
        match (menu, item) {
            (0, 2) => true,
            _ => false,
        }
    }
    fn menu_first_item(menu: usize) -> usize {
        let n = Self::menu_len(menu);
        (0..n).find(|&i| !Self::menu_is_sep(menu, i)).unwrap_or(0)
    }
    fn menu_next(&mut self, delta: isize) {
        let n = 1isize;
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

fn view(state: &mut Scratch, frame: &mut Frame) {
    let area = frame.area();
    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Fill(1), Constraint::Length(1)]).split(area);
    let menu_area = chunks[0];
    let menu_style = ratatui::style::Style::new().bg(ratatui::style::Color::Cyan).fg(ratatui::style::Color::Black);
    let mut menu_spans = Vec::new();
    menu_spans.push(Span::styled(" File ", if state.menu_open == Some(0) { menu_style.add_modifier(ratatui::style::Modifier::REVERSED) } else { menu_style }));
    frame.render_widget(Paragraph::new(Line::from(menu_spans)).style(menu_style), menu_area);
    let view_area = chunks[1];
    let block = Block::bordered().title("Notes");
    let inner = block.inner(view_area);
    frame.render_widget(block, view_area);
    state.notes_state.set_block(Block::bordered().title("notes"));
    state.notes_state.set_cursor_style(ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED));
    state.notes_state.set_cursor_line_style(ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::UNDERLINED));
    frame.render_widget(&state.notes_state, inner);
    let status_area = chunks[2];
    frame.render_widget(Paragraph::new(Line::from(vec![Span::raw(format!(" {}  ", "F10 File — Open / Save As")), Span::styled(" Esc ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" quit  "), Span::styled(" F10 ", ratatui::style::Style::new().add_modifier(ratatui::style::Modifier::REVERSED)), Span::raw(" menu  ")])).style(ratatui::style::Style::new().bg(ratatui::style::Color::Cyan).fg(ratatui::style::Color::Black)), status_area);
    if let Some(open) = state.menu_open {
        let labels: [&str; 1] = [" File "];
        let mut x = menu_area.x;
        for (i, lab) in labels.iter().enumerate() {
            if i == open {
                break;
            }
            x = x.saturating_add(lab.len() as u16);
        }
        match open {
            0 => {
                let items: [&str; 4] = [" Open", " Save As", "────────", " Quit"];
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


mod file_dialog {
    const DIALOG_BG: ratatui::style::Color = ratatui::style::Color::Cyan;
    const DIALOG_FG: ratatui::style::Color = ratatui::style::Color::Black;
    use std::path::{Path, PathBuf};
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
    use ratatui::layout::Rect;
    use ratatui::style::{Modifier, Style};
    use ratatui::widgets::{Block, Borders, Clear, Paragraph};

    #[derive(Default)]
    struct PathTabState {
        candidates: Vec<String>,
        index: usize,
    }

    pub fn prompt<B, F>(
        terminal: &mut ratatui::Terminal<B>,
        title: &str,
        initial: &str,
        save: bool,
        mut draw: F,
    ) -> std::io::Result<String>
    where
        B: ratatui::backend::Backend,
        F: FnMut(&mut ratatui::Frame),
    {
        let mut input = initial.to_string();
        let mut hint = String::new();
        let mut tab: Option<PathTabState> = None;
        let hints = if save {
            "Tab=complete  Enter=save / enter folder  Esc=Cancel"
        } else {
            "Tab=complete  Enter=open file / enter folder  Esc=Cancel"
        };
        loop {
            terminal.draw(|frame| {
                draw(frame);
                overlay(frame, title, &input, save, hints, &hint);
            })?;
            let Event::Key(key) = event::read()? else { continue };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Esc => return Ok(String::new()),
                KeyCode::Enter => {
                    tab = None;
                    if let Some(dir) = path_enter_dir(&input) {
                        hint = format!(" In {dir}  (Tab lists, Enter continues)");
                        input = dir;
                    } else if save {
                        return Ok(input.trim().to_string());
                    } else {
                        let path = PathBuf::from(input.trim());
                        if path.is_file() {
                            return Ok(input.trim().to_string());
                        }
                        hint = " No such file".into();
                    }
                }
                KeyCode::Tab | KeyCode::BackTab => {
                    let reverse = matches!(key.code, KeyCode::BackTab)
                        || key.modifiers.contains(KeyModifiers::SHIFT);
                    let (next, msg) = path_tab_complete(&input, &mut tab, reverse);
                    input = next;
                    if !msg.is_empty() {
                        hint = msg;
                    }
                }
                KeyCode::Backspace => {
                    tab = None;
                    input.pop();
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    tab = None;
                    input.push(c);
                }
                _ => {}
            }
        }
    }

    pub fn prompt_folder<B, F>(
        terminal: &mut ratatui::Terminal<B>,
        title: &str,
        initial: &str,
        mut draw: F,
    ) -> std::io::Result<String>
    where
        B: ratatui::backend::Backend,
        F: FnMut(&mut ratatui::Frame),
    {
        let mut input = initial.to_string();
        let mut hint = String::new();
        let mut tab: Option<PathTabState> = None;
        let hints = "Tab=complete  Enter=choose folder  Esc=Cancel";
        loop {
            terminal.draw(|frame| {
                draw(frame);
                overlay(frame, title, &input, false, hints, &hint);
            })?;
            let Event::Key(key) = event::read()? else { continue };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Esc => return Ok(String::new()),
                KeyCode::Enter => {
                    tab = None;
                    if let Some(dir) = path_enter_dir(&input) {
                        let trimmed = dir.trim_end_matches(|c| c == '/' || c == '\\').to_string();
                        return Ok(if trimmed.is_empty() { dir } else { trimmed });
                    }
                    hint = " Not a folder".into();
                }
                KeyCode::Tab | KeyCode::BackTab => {
                    let reverse = matches!(key.code, KeyCode::BackTab)
                        || key.modifiers.contains(KeyModifiers::SHIFT);
                    let (next, msg) = path_tab_complete(&input, &mut tab, reverse);
                    input = next;
                    if !msg.is_empty() {
                        hint = msg;
                    }
                }
                KeyCode::Backspace => {
                    tab = None;
                    input.pop();
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    tab = None;
                    input.push(c);
                }
                _ => {}
            }
        }
    }

    fn overlay(frame: &mut ratatui::Frame, title: &str, input: &str, save: bool, hints: &str, hint: &str) {
        let area = frame.area();
        let width = 56u16.min(area.width.saturating_sub(4));
        let height = 10u16.min(area.height.saturating_sub(4));
        let rect = Rect {
            x: area.x + (area.width.saturating_sub(width)) / 2,
            y: area.y + (area.height.saturating_sub(height)) / 2,
            width,
            height,
        };
        frame.render_widget(Clear, rect);
        let style = Style::new()
            .bg(DIALOG_BG)
            .fg(DIALOG_FG);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(DIALOG_FG))
            .style(style);
        let inner = block.inner(rect);
        frame.render_widget(block, rect);
        let kind = if save { "File name" } else { "File or folder" };
        let body = format!("{kind}\n\n [{input}_]\n\n{hints}{hint}");
        frame.render_widget(
            Paragraph::new(body).style(style.add_modifier(Modifier::BOLD)),
            inner,
        );
    }

    fn path_enter_dir(input: &str) -> Option<String> {
        let trimmed = input.trim();
        let path = if trimmed.is_empty() {
            PathBuf::from(".")
        } else {
            PathBuf::from(trimmed)
        };
        if !path.is_dir() {
            return None;
        }
        let normalized = normalize_path_components(&path);
        let mut s = normalized.to_string_lossy().into_owned();
        if s.is_empty() {
            s.push('.');
        }
        let sep = preferred_sep(trimmed);
        if !s.ends_with('/') && !s.ends_with('\\') {
            s.push(sep);
        }
        Some(s)
    }

    fn preferred_sep(input: &str) -> char {
        if input.contains('\\') && !input.contains('/') {
            '\\'
        } else {
            '/'
        }
    }

    fn normalize_path_components(path: &Path) -> PathBuf {
        use std::path::Component;
        let mut out = PathBuf::new();
        for c in path.components() {
            match c {
                Component::Prefix(_) | Component::RootDir => out.push(c.as_os_str()),
                Component::CurDir => {}
                Component::ParentDir => {
                    if !out.pop() {
                        out.push("..");
                    }
                }
                Component::Normal(s) => out.push(s),
            }
        }
        out
    }

    fn path_tab_complete(
        input: &str,
        state: &mut Option<PathTabState>,
        reverse: bool,
    ) -> (String, String) {
        if let Some(st) = state.as_mut() {
            if st.candidates.get(st.index).map(String::as_str) == Some(input) {
                let n = st.candidates.len();
                let unique_dir = n == 1 && (input.ends_with('/') || input.ends_with('\\'));
                if !unique_dir && n > 0 {
                    st.index = if reverse {
                        if st.index == 0 { n - 1 } else { st.index - 1 }
                    } else {
                        (st.index + 1) % n
                    };
                    let msg = if n > 1 {
                        format!(" {}/{} matches", st.index + 1, n)
                    } else {
                        String::new()
                    };
                    return (st.candidates[st.index].clone(), msg);
                }
                *state = None;
            }
        }
        let candidates = list_path_completions(input);
        if candidates.is_empty() {
            *state = None;
            return (input.to_string(), " No matches".into());
        }
        let msg = if candidates.len() > 1 {
            format!(" 1/{} matches — Tab to cycle", candidates.len())
        } else {
            String::new()
        };
        let result = candidates[0].clone();
        *state = if candidates.len() == 1 && (result.ends_with('/') || result.ends_with('\\')) {
            None
        } else {
            Some(PathTabState { candidates, index: 0 })
        };
        (result, msg)
    }

    fn list_path_completions(input: &str) -> Vec<String> {
        let trimmed = input.trim_start();
        let lead_ws_len = input.len() - trimmed.len();
        let lead_ws = &input[..lead_ws_len];
        let (dir, partial, prefix) = split_path_prefix(trimmed);
        let Ok(rd) = std::fs::read_dir(&dir) else {
            return Vec::new();
        };
        let partial_lower = partial.to_ascii_lowercase();
        let sep = preferred_sep(if prefix.is_empty() { trimmed } else { &prefix });
        let mut children: Vec<(String, bool)> = Vec::new();
        for ent in rd.flatten() {
            let name = ent.file_name();
            let name = name.to_string_lossy();
            if name == "." || name == ".." {
                continue;
            }
            if !partial_lower.is_empty() && !name.to_ascii_lowercase().starts_with(&partial_lower) {
                continue;
            }
            let is_dir = ent.file_type().map(|t| t.is_dir()).unwrap_or(false);
            children.push((name.into_owned(), is_dir));
        }
        children.sort_by(|a, b| a.0.to_ascii_lowercase().cmp(&b.0.to_ascii_lowercase()));
        let mut out: Vec<String> = Vec::new();
        for (name, is_dir) in children {
            let mut s = String::new();
            s.push_str(lead_ws);
            s.push_str(&prefix);
            s.push_str(&name);
            if is_dir {
                s.push(sep);
            }
            out.push(s);
        }
        let want_parent = partial_lower.is_empty()
            || "..".starts_with(partial_lower.as_str())
            || partial_lower == ".";
        if want_parent {
            if let Some(up) = parent_completion(lead_ws, &prefix, &dir, sep) {
                out.push(up);
            }
        }
        out
    }

    fn parent_completion(lead_ws: &str, prefix: &str, dir: &Path, sep: char) -> Option<String> {
        if let Ok(canon) = std::fs::canonicalize(dir) {
            if canon.parent().is_none() {
                return None;
            }
        }
        let mut s = String::new();
        s.push_str(lead_ws);
        s.push_str(prefix);
        s.push_str("..");
        s.push(sep);
        Some(s)
    }

    fn split_path_prefix(input: &str) -> (PathBuf, String, String) {
        if input.is_empty() {
            return (PathBuf::from("."), String::new(), String::new());
        }
        if input.ends_with('/') || input.ends_with('\\') {
            return (PathBuf::from(input), String::new(), input.to_string());
        }
        let path = Path::new(input);
        match path.file_name() {
            Some(name) if path.parent().is_some_and(|p| !p.as_os_str().is_empty()) => {
                let parent = path.parent().unwrap();
                let mut prefix = parent.to_string_lossy().into_owned();
                if !prefix.ends_with('/') && !prefix.ends_with('\\') {
                    prefix.push(preferred_sep(input));
                }
                (parent.to_path_buf(), name.to_string_lossy().into_owned(), prefix)
            }
            Some(name) => (
                PathBuf::from("."),
                name.to_string_lossy().into_owned(),
                String::new(),
            ),
            None => (PathBuf::from("."), String::new(), String::new()),
        }
    }
}

fn main() -> std::io::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
    let mut state = Scratch::default();
    let mut terminal = ratatui::init();
    loop {
        terminal.draw(|frame| view(&mut state, frame))?;
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
                                        {
                                            let __vbr_event: Result<(), String> = (|| {
                                                let picked: String = file_dialog::prompt(&mut terminal, " Open file ", &(state.path).to_string(), false, |frame| view(&mut state, frame))?;
                                                if picked != "" {
                                                    #[allow(unused_mut)]
                                                    let mut text: String;
                                                    text = match FileSystem::read(&picked) {
                                                        Ok(__vbr_ok) => __vbr_ok,
                                                        Err(e) => {
                                                            state.notes = format!("Could not read: {}", e);
                                                            return Ok(());
                                                        }
                                                    };
                                                    state.notes = text;
                                                    state.path = picked;
                                                }
                                                Ok(())
                                            })();
                                            if let Err(__e) = __vbr_event {
                                                eprintln!("Error: {}", __e);
                                            }
                                        }
                                    }
                                    (0, 1) => {
                                        {
                                            let __vbr_event: Result<(), String> = (|| {
                                                let picked: String = file_dialog::prompt(&mut terminal, " Save as ", &(state.path).to_string(), true, |frame| view(&mut state, frame))?;
                                                if picked != "" {
                                                    if let Err(e) = FileSystem::write(&picked, &state.notes) {
                                                        state.notes = format!("Could not save: {}", e);
                                                        return Ok(());
                                                    }
                                                    state.path = picked;
                                                }
                                                Ok(())
                                            })();
                                            if let Err(__e) = __vbr_event {
                                                eprintln!("Error: {}", __e);
                                            }
                                        }
                                    }
                                    (0, 3) => {
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        }
                        KeyCode::Char(c) => match (state.menu_open, c.to_ascii_lowercase()) {
                            (Some(0), 'o') => {
                                state.menu_close();
                                {
                                    let __vbr_event: Result<(), String> = (|| {
                                        let picked: String = file_dialog::prompt(&mut terminal, " Open file ", &(state.path).to_string(), false, |frame| view(&mut state, frame))?;
                                        if picked != "" {
                                            #[allow(unused_mut)]
                                            let mut text: String;
                                            text = match FileSystem::read(&picked) {
                                                Ok(__vbr_ok) => __vbr_ok,
                                                Err(e) => {
                                                    state.notes = format!("Could not read: {}", e);
                                                    return Ok(());
                                                }
                                            };
                                            state.notes = text;
                                            state.path = picked;
                                        }
                                        Ok(())
                                    })();
                                    if let Err(__e) = __vbr_event {
                                        eprintln!("Error: {}", __e);
                                    }
                                }
                            }
                            (Some(0), 's') => {
                                state.menu_close();
                                {
                                    let __vbr_event: Result<(), String> = (|| {
                                        let picked: String = file_dialog::prompt(&mut terminal, " Save as ", &(state.path).to_string(), true, |frame| view(&mut state, frame))?;
                                        if picked != "" {
                                            if let Err(e) = FileSystem::write(&picked, &state.notes) {
                                                state.notes = format!("Could not save: {}", e);
                                                return Ok(());
                                            }
                                            state.path = picked;
                                        }
                                        Ok(())
                                    })();
                                    if let Err(__e) = __vbr_event {
                                        eprintln!("Error: {}", __e);
                                    }
                                }
                            }
                            (Some(0), 'q') => {
                                state.menu_close();
                                break;
                            }
                            (_, 'f') => state.menu_activate(0),
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
                            _ => {}
                        }
                    }
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
    }
    ratatui::restore();
    Ok(())
}
