//! Turbo Pascal–style chrome: menu bar, editor, message line, status, dialogs.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;
use tide_editor::{Document, EditorView, EditorWidget, Highlighter};

use crate::files;
use crate::theme::TpTheme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuId {
    File,
    Edit,
    Run,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileCmd {
    New,
    Open,
    Save,
    SaveAs,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditCmd {
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunCmd {
    Compile,
    Run,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpCmd {
    About,
    Keys,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuCmd {
    File(FileCmd),
    Edit(EditCmd),
    Run(RunCmd),
    Help(HelpCmd),
}

pub struct MenuBar {
    pub open: Option<MenuId>,
    pub selected: usize,
}

impl Default for MenuBar {
    fn default() -> Self {
        Self {
            open: None,
            selected: 0,
        }
    }
}

impl MenuBar {
    pub fn items(id: MenuId) -> &'static [(&'static str, MenuCmd)] {
        match id {
            MenuId::File => &[
                ("New        Ctrl+N", MenuCmd::File(FileCmd::New)),
                ("Open...    Ctrl+O", MenuCmd::File(FileCmd::Open)),
                ("Save       Ctrl+S", MenuCmd::File(FileCmd::Save)),
                ("Save as...", MenuCmd::File(FileCmd::SaveAs)),
                ("Quit       Ctrl+Q", MenuCmd::File(FileCmd::Quit)),
            ],
            MenuId::Edit => &[
                ("Undo       Ctrl+Z", MenuCmd::Edit(EditCmd::Undo)),
                ("Redo       Ctrl+Y", MenuCmd::Edit(EditCmd::Redo)),
                ("Cut        Ctrl+X", MenuCmd::Edit(EditCmd::Cut)),
                ("Copy       Ctrl+C", MenuCmd::Edit(EditCmd::Copy)),
                ("Paste      Ctrl+V", MenuCmd::Edit(EditCmd::Paste)),
            ],
            MenuId::Run => &[
                ("Compile    Alt+F9", MenuCmd::Run(RunCmd::Compile)),
                ("Run        F9", MenuCmd::Run(RunCmd::Run)),
            ],
            MenuId::Help => &[
                ("Keys       F1", MenuCmd::Help(HelpCmd::Keys)),
                ("About TIDE", MenuCmd::Help(HelpCmd::About)),
            ],
        }
    }

    pub fn top_labels() -> &'static [(MenuId, &'static str)] {
        &[
            (MenuId::File, " File "),
            (MenuId::Edit, " Edit "),
            (MenuId::Run, " Run "),
            (MenuId::Help, " Help "),
        ]
    }

    pub fn activate(&mut self, id: MenuId) {
        self.open = Some(id);
        self.selected = 0;
    }

    pub fn close(&mut self) {
        self.open = None;
        self.selected = 0;
    }

    pub fn move_sel(&mut self, delta: isize) {
        let Some(id) = self.open else { return };
        let n = Self::items(id).len() as isize;
        if n == 0 {
            return;
        }
        let mut s = self.selected as isize + delta;
        while s < 0 {
            s += n;
        }
        self.selected = (s % n) as usize;
    }

    pub fn next_menu(&mut self, delta: isize) {
        let labels = Self::top_labels();
        let cur = self
            .open
            .and_then(|id| labels.iter().position(|(i, _)| *i == id))
            .unwrap_or(0) as isize;
        let n = labels.len() as isize;
        let mut i = cur + delta;
        while i < 0 {
            i += n;
        }
        let id = labels[(i % n) as usize].0;
        self.activate(id);
    }

    pub fn current_cmd(&self) -> Option<MenuCmd> {
        let id = self.open?;
        Self::items(id).get(self.selected).map(|(_, c)| *c)
    }
}

#[derive(Debug, Clone)]
pub enum Dialog {
    Open { input: String },
    SaveAs { input: String },
    ConfirmQuit,
    Help,
    About,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Editor,
    Watch,
}

pub struct UiState {
    pub menu: MenuBar,
    pub dialog: Option<Dialog>,
    pub message: String,
    pub diagnostics: Vec<crate::compile::TideDiag>,
    pub watch_selected: usize,
    pub focus: Focus,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            menu: MenuBar::default(),
            dialog: None,
            message: " F1 Help  F10 Menu  F9 Run  Alt+F9 Compile ".into(),
            diagnostics: Vec::new(),
            watch_selected: 0,
            focus: Focus::Editor,
        }
    }
}

impl UiState {
    pub fn set_diagnostics(&mut self, diags: Vec<crate::compile::TideDiag>) {
        self.diagnostics = diags;
        self.watch_selected = 0;
        if !self.diagnostics.is_empty() {
            self.focus = Focus::Watch;
        } else {
            self.focus = Focus::Editor;
        }
    }

    pub fn clear_diagnostics(&mut self) {
        self.diagnostics.clear();
        self.watch_selected = 0;
        self.focus = Focus::Editor;
    }

    pub fn watch_visible(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    pub fn move_watch(&mut self, delta: isize) {
        let n = self.diagnostics.len() as isize;
        if n == 0 {
            return;
        }
        let mut s = self.watch_selected as isize + delta;
        while s < 0 {
            s += n;
        }
        self.watch_selected = (s % n) as usize;
    }

    pub fn selected_diag(&self) -> Option<&crate::compile::TideDiag> {
        self.diagnostics.get(self.watch_selected)
    }
}

pub fn draw(
    f: &mut Frame,
    doc: &Document,
    view: &EditorView,
    highlighter: &dyn Highlighter,
    ui: &UiState,
    decorations: &[tide_editor::Decoration],
) {
    let area = f.area();
    f.render_widget(Block::default().style(TpTheme::desktop()), area);

    let watch_h = if ui.watch_visible() { 7u16 } else { 0 };
    let chunks = if watch_h > 0 {
        Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(watch_h),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area)
    } else {
        Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area)
    };

    draw_menu(f, chunks[0], &ui.menu);
    let editor_focused = ui.focus == Focus::Editor && ui.menu.open.is_none() && ui.dialog.is_none();
    draw_editor(
        f,
        chunks[1],
        doc,
        view,
        highlighter,
        decorations,
        editor_focused,
    );

    let (msg_i, status_i) = if watch_h > 0 {
        draw_watch(f, chunks[2], ui);
        (3usize, 4usize)
    } else {
        (2, 3)
    };
    draw_message(f, chunks[msg_i], &ui.message);
    draw_status(f, chunks[status_i], doc, view, ui);

    if let Some(d) = &ui.dialog {
        draw_dialog(f, area, d);
    }

    if let Some(id) = ui.menu.open {
        draw_dropdown(f, chunks[0], &ui.menu, id);
    }
}

fn draw_menu(f: &mut Frame, area: Rect, menu: &MenuBar) {
    let mut spans = Vec::new();
    for (id, label) in MenuBar::top_labels() {
        let selected = menu.open == Some(*id);
        let style = if selected {
            TpTheme::menu_selected()
        } else {
            TpTheme::menu()
        };
        // Hot key letter bold/white
        let text = label.to_string();
        spans.push(Span::styled(text, style));
    }
    f.render_widget(Paragraph::new(Line::from(spans)).style(TpTheme::menu()), area);
}

fn draw_dropdown(f: &mut Frame, menu_area: Rect, menu: &MenuBar, id: MenuId) {
    let rect = dropdown_rect(menu_area, id);
    let items = MenuBar::items(id);
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(TpTheme::frame())
        .style(TpTheme::menu());
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    for (i, (label, _)) in items.iter().enumerate() {
        let style = if i == menu.selected {
            TpTheme::menu_selected()
        } else {
            TpTheme::menu()
        };
        let row = Rect {
            x: inner.x,
            y: inner.y + i as u16,
            width: inner.width,
            height: 1,
        };
        f.render_widget(Paragraph::new(format!(" {label}")).style(style), row);
    }
}

/// Screen rect for the menu bar row (full width, top line).
pub fn menu_bar_rect(frame: Rect) -> Rect {
    Rect {
        x: frame.x,
        y: frame.y,
        width: frame.width,
        height: 1,
    }
}

/// Bounding box of a top-level menu label.
pub fn top_menu_label_rect(menu_area: Rect, id: MenuId) -> Rect {
    let mut x = menu_area.x;
    for (mid, lab) in MenuBar::top_labels() {
        let w = lab.len() as u16;
        if *mid == id {
            return Rect {
                x,
                y: menu_area.y,
                width: w,
                height: 1,
            };
        }
        x += w;
    }
    menu_area
}

pub fn dropdown_rect(menu_area: Rect, id: MenuId) -> Rect {
    let labels = MenuBar::top_labels();
    let idx = labels.iter().position(|(i, _)| *i == id).unwrap_or(0);
    let mut x = menu_area.x;
    for (i, (_, lab)) in labels.iter().enumerate() {
        if i == idx {
            break;
        }
        x += lab.len() as u16;
    }
    let items = MenuBar::items(id);
    let width = items
        .iter()
        .map(|(s, _)| s.len())
        .max()
        .unwrap_or(10)
        .max(14) as u16
        + 4;
    let height = items.len() as u16 + 2;
    Rect {
        x,
        y: menu_area.y + 1,
        width: width.min(menu_area.width.saturating_sub(x.saturating_sub(menu_area.x))),
        height,
    }
}

/// What a mouse click hit in the menu chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuHit {
    /// Clicked a top-level menu title.
    Top(MenuId),
    /// Clicked an item inside the open dropdown.
    Item(MenuCmd),
    /// Clicked elsewhere while a menu was open (dismiss).
    Dismiss,
    /// Missed the menu UI entirely (and no menu was open).
    None,
}

pub fn hit_test_menu(frame: Rect, menu: &MenuBar, column: u16, row: u16) -> MenuHit {
    let bar = menu_bar_rect(frame);

    // Top labels
    if row == bar.y {
        for (id, _) in MenuBar::top_labels() {
            let r = top_menu_label_rect(bar, *id);
            if column >= r.x && column < r.x + r.width {
                return MenuHit::Top(*id);
            }
        }
        if menu.open.is_some() {
            return MenuHit::Dismiss;
        }
        return MenuHit::None;
    }

    if let Some(id) = menu.open {
        let drop = dropdown_rect(bar, id);
        if column >= drop.x
            && column < drop.x + drop.width
            && row >= drop.y
            && row < drop.y + drop.height
        {
            // Inside border: item rows are inner.y .. 
            if row > drop.y && row < drop.y + drop.height - 1 {
                let idx = (row - drop.y - 1) as usize;
                if let Some((_, cmd)) = MenuBar::items(id).get(idx) {
                    return MenuHit::Item(*cmd);
                }
            }
            return MenuHit::Dismiss; // border / empty
        }
        return MenuHit::Dismiss;
    }

    MenuHit::None
}

fn draw_editor(
    f: &mut Frame,
    area: Rect,
    doc: &Document,
    view: &EditorView,
    highlighter: &dyn Highlighter,
    decorations: &[tide_editor::Decoration],
    show_cursor: bool,
) {
    let title = format!(" {} ", files::display_name(doc));
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(TpTheme::frame())
        .title(title)
        .style(TpTheme::editor());
    let inner = block.inner(area);
    f.render_widget(block, area);

    let widget = EditorWidget::new(doc, view)
        .highlighter(highlighter)
        .decorations(decorations)
        .style(TpTheme::editor())
        .show_cursor(show_cursor);
    f.render_widget(widget, inner);
}

fn draw_watch(f: &mut Frame, area: Rect, ui: &UiState) {
    let focused = ui.focus == Focus::Watch;
    let title = if focused {
        " Watch  (↑↓ select  Enter jump  Tab editor) "
    } else {
        " Watch  (Tab to focus) "
    };
    let border = if focused {
        Style::default().fg(ratatui::style::Color::Yellow)
    } else {
        TpTheme::frame()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border)
        .title(title)
        .style(TpTheme::menu());
    let inner = block.inner(area);
    f.render_widget(block, area);

    let visible = inner.height as usize;
    let sel = ui.watch_selected;
    let start = sel.saturating_sub(visible.saturating_sub(1));
    for (row, idx) in (start..ui.diagnostics.len()).take(visible).enumerate() {
        let d = &ui.diagnostics[idx];
        let style = if idx == sel && focused {
            TpTheme::menu_selected()
        } else {
            TpTheme::menu()
        };
        let row_area = Rect {
            x: inner.x,
            y: inner.y + row as u16,
            width: inner.width,
            height: 1,
        };
        let text = truncate(&d.render_line(), inner.width as usize);
        f.render_widget(Paragraph::new(text).style(style), row_area);
    }
}

fn truncate(s: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i + 1 >= width {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

fn draw_message(f: &mut Frame, area: Rect, message: &str) {
    f.render_widget(
        Paragraph::new(message).style(TpTheme::menu()),
        area,
    );
}

fn draw_status(f: &mut Frame, area: Rect, doc: &Document, view: &EditorView, ui: &UiState) {
    let (line, col) = view.cursor_position(doc);
    let dirty = if doc.is_dirty() { "*" } else { " " };
    let name = files::display_name(doc);
    let err_n = ui
        .diagnostics
        .iter()
        .filter(|d| d.level == crate::compile::DiagLevel::Error)
        .count();
    let left = if err_n > 0 {
        format!(" {name}{dirty}  {err_n} error(s) ")
    } else {
        format!(" {name}{dirty} ")
    };
    let focus = match ui.focus {
        Focus::Editor => "EDIT",
        Focus::Watch => "WATCH",
    };
    let right = format!(" {focus}  Ln {}, Col {} ", line + 1, col + 1);
    let mid_w = area.width.saturating_sub((left.len() + right.len()) as u16);
    let mid = " ".repeat(mid_w as usize);
    let line = Line::from(vec![
        Span::raw(left),
        Span::raw(mid),
        Span::raw(right),
    ]);
    f.render_widget(Paragraph::new(line).style(TpTheme::status()), area);
}

fn draw_dialog(f: &mut Frame, area: Rect, dialog: &Dialog) {
    let (title, body) = match dialog {
        Dialog::Open { input } => (
            " Open file ",
            format!("File name\n\n [{input}_]\n\nEnter=OK  Esc=Cancel"),
        ),
        Dialog::SaveAs { input } => (
            " Save as ",
            format!("File name\n\n [{input}_]\n\nEnter=OK  Esc=Cancel"),
        ),
        Dialog::ConfirmQuit => (
            " Quit ",
            "File not saved. Quit anyway?\n\n Y = Yes   N / Esc = No".into(),
        ),
        Dialog::Help => (
            " Keys ",
            "F10  Menus          F1   This help\n\
             F9   Run            Alt+F9 Compile\n\
             Ctrl+O Open         Ctrl+N New\n\
             Ctrl+Q Quit         Ctrl+Z/Y Undo/Redo\n\
             Ctrl+C/X/V Copy/Cut/Paste\n\
             Tab  Editor/Watch   Enter jump to error\n\
             Mouse: menus + drag to select text"
                .into(),
        ),
        Dialog::About => (
            " About ",
            "TIDE — TUI IDE for VBR\n\
             Turbo Pascal vibes. Built on tide-editor.\n\
             \n\
             Esc to close"
                .into(),
        ),
    };

    let width = 50u16.min(area.width.saturating_sub(4));
    let height = 10u16.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let rect = Rect {
        x,
        y,
        width,
        height,
    };
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(TpTheme::DIALOG_FG))
        .style(TpTheme::dialog());
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    f.render_widget(
        Paragraph::new(body).style(TpTheme::dialog().add_modifier(Modifier::BOLD)),
        inner,
    );
}

/// Viewport size for scrolling (inside editor border).
pub fn editor_text_area(frame_area: Rect, watch_visible: bool) -> (usize, usize) {
    let inner = editor_inner_rect(frame_area, watch_visible);
    (inner.height.max(1) as usize, inner.width.max(1) as usize)
}

/// Screen rect of the editor content (inside the double border).
pub fn editor_inner_rect(frame: Rect, watch_visible: bool) -> Rect {
    let watch_h = if watch_visible { 7u16 } else { 0 };
    let chunks = if watch_h > 0 {
        Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(watch_h),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(frame)
    } else {
        Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(frame)
    };
    let area = chunks[1];
    Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    }
}

/// Watch window inner rect (for mouse hits), or None if hidden.
pub fn watch_inner_rect(frame: Rect, watch_visible: bool) -> Option<Rect> {
    if !watch_visible {
        return None;
    }
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(7),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(frame);
    let area = chunks[2];
    Some(Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    })
}

pub fn hit_editor(frame: Rect, watch_visible: bool, column: u16, row: u16) -> bool {
    let r = editor_inner_rect(frame, watch_visible);
    column >= r.x && column < r.x + r.width && row >= r.y && row < r.y + r.height
}

pub fn hit_watch(frame: Rect, watch_visible: bool, column: u16, row: u16) -> Option<usize> {
    let inner = watch_inner_rect(frame, watch_visible)?;
    if column < inner.x
        || column >= inner.x + inner.width
        || row < inner.y
        || row >= inner.y + inner.height
    {
        return None;
    }
    Some((row - inner.y) as usize)
}
