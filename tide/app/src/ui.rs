//! Turbo Pascal–style chrome: menu bar, editor, message line, status, dialogs.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
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
    OpenProject,
    Units,
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
    Find,
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunCmd {
    Compile,
    Run,
    ToggleRust,
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
                ("New           Ctrl+N", MenuCmd::File(FileCmd::New)),
                ("Open file...  Ctrl+O", MenuCmd::File(FileCmd::Open)),
                (
                    "Open project... Ctrl+P",
                    MenuCmd::File(FileCmd::OpenProject),
                ),
                ("Units...      Ctrl+U", MenuCmd::File(FileCmd::Units)),
                ("Save          Ctrl+S", MenuCmd::File(FileCmd::Save)),
                ("Save as...", MenuCmd::File(FileCmd::SaveAs)),
                ("Quit          Ctrl+Q", MenuCmd::File(FileCmd::Quit)),
            ],
            MenuId::Edit => &[
                ("Undo       Ctrl+Z", MenuCmd::Edit(EditCmd::Undo)),
                ("Redo       Ctrl+Y", MenuCmd::Edit(EditCmd::Redo)),
                ("Cut        Ctrl+X", MenuCmd::Edit(EditCmd::Cut)),
                ("Copy       Ctrl+C", MenuCmd::Edit(EditCmd::Copy)),
                ("Paste      Ctrl+V", MenuCmd::Edit(EditCmd::Paste)),
                ("Find...    Ctrl+F", MenuCmd::Edit(EditCmd::Find)),
                ("Replace... Ctrl+H", MenuCmd::Edit(EditCmd::Replace)),
            ],
            MenuId::Run => &[
                ("Compile    Alt+F9", MenuCmd::Run(RunCmd::Compile)),
                ("Run        F9", MenuCmd::Run(RunCmd::Run)),
                ("Rust pane  F4", MenuCmd::Run(RunCmd::ToggleRust)),
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
    Open {
        input: String,
        cwd: std::path::PathBuf,
    },
    OpenProject {
        input: String,
        cwd: std::path::PathBuf,
    },
    Units {
        selected: usize,
    },
    SaveAs {
        input: String,
        cwd: std::path::PathBuf,
    },
    ConfirmQuit,
    Help,
    About,
    Find {
        input: String,
        case_sensitive: bool,
    },
    Replace {
        find: String,
        replace: String,
        /// 0 = find field, 1 = replace field
        field: u8,
        case_sensitive: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Editor,
    Rust,
    Watch,
}

pub struct UiState {
    pub menu: MenuBar,
    pub dialog: Option<Dialog>,
    pub message: String,
    pub diagnostics: Vec<crate::compile::TideDiag>,
    pub watch_selected: usize,
    pub focus: Focus,
    pub find: crate::find::FindState,
    /// Project folder when editing a multifile / main.vbr project.
    pub project_dir: Option<std::path::PathBuf>,
    /// `.vbr` units in the project (main first).
    pub units: Vec<std::path::PathBuf>,
    /// Tab path-completion cycle for Open / Open Project / Save As.
    pub path_tab: Option<crate::files::PathTabState>,
    /// Turbo Debugger–style generated Rust strip (bottom).
    pub show_rust: bool,
    /// True when the Bust buffer changed since the last Rust refresh.
    pub rust_stale: bool,
    /// `(rust_line, vbr_line)` 1-based map from the last compile.
    pub line_map: Vec<(usize, usize)>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            menu: MenuBar::default(),
            dialog: None,
            message: " F1 Help  F10 Menu  Ctrl+P Project  Ctrl+U Units ".into(),
            diagnostics: Vec::new(),
            watch_selected: 0,
            focus: Focus::Editor,
            find: crate::find::FindState::default(),
            project_dir: None,
            units: Vec::new(),
            path_tab: None,
            show_rust: false,
            rust_stale: false,
            line_map: Vec::new(),
        }
    }
}

impl UiState {
    pub fn set_diagnostics(&mut self, diags: Vec<crate::compile::TideDiag>) {
        self.diagnostics = diags;
        self.watch_selected = 0;
        if !self.diagnostics.is_empty() {
            self.focus = Focus::Watch;
        } else if self.focus == Focus::Watch {
            self.focus = Focus::Editor;
        }
    }

    pub fn clear_diagnostics(&mut self) {
        self.diagnostics.clear();
        self.watch_selected = 0;
        if self.focus == Focus::Watch {
            self.focus = Focus::Editor;
        }
    }

    pub fn watch_visible(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Editor if self.show_rust => Focus::Rust,
            Focus::Editor if self.watch_visible() => Focus::Watch,
            Focus::Editor => Focus::Editor,
            Focus::Rust if self.watch_visible() => Focus::Watch,
            Focus::Rust => Focus::Editor,
            Focus::Watch => Focus::Editor,
        };
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

    pub fn clear_project(&mut self) {
        self.project_dir = None;
        self.units.clear();
    }

    pub fn set_project(&mut self, dir: std::path::PathBuf, units: Vec<std::path::PathBuf>) {
        self.project_dir = Some(dir);
        self.units = units;
    }

    pub fn has_project(&self) -> bool {
        self.project_dir.is_some() && !self.units.is_empty()
    }

    pub fn current_unit_index(&self, doc: &Document) -> Option<usize> {
        let path = doc.path()?;
        self.units.iter().position(|u| u == path)
    }
}

pub fn draw(
    f: &mut Frame,
    doc: &Document,
    view: &EditorView,
    highlighter: &dyn Highlighter,
    rust_doc: &Document,
    rust_view: &EditorView,
    rust_highlighter: &dyn Highlighter,
    rust_decorations: &[tide_editor::Decoration],
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

    let panes = split_editor_panes(chunks[1], ui.show_rust);
    let vbr_focus = ui.focus == Focus::Editor && ui.menu.open.is_none() && ui.dialog.is_none();
    draw_editor(
        f,
        panes.vbr,
        doc,
        view,
        highlighter,
        decorations,
        vbr_focus,
        false,
        ui.rust_stale && ui.show_rust,
    );
    if let Some(rust_area) = panes.rust {
        let rust_focus = ui.focus == Focus::Rust && ui.menu.open.is_none() && ui.dialog.is_none();
        draw_editor(
            f,
            rust_area,
            rust_doc,
            rust_view,
            rust_highlighter,
            rust_decorations,
            rust_focus,
            true,
            ui.rust_stale,
        );
    }

    let (msg_i, status_i) = if watch_h > 0 {
        draw_watch(f, chunks[2], ui);
        (3usize, 4usize)
    } else {
        (2, 3)
    };
    draw_message(f, chunks[msg_i], &ui.message);
    draw_status(f, chunks[status_i], doc, view, ui);

    if let Some(d) = &ui.dialog {
        match d {
            Dialog::Units { selected } => {
                draw_units_dialog(f, area, &ui.units, *selected);
            }
            _ => draw_dialog(f, area, d),
        }
    }

    if let Some(id) = ui.menu.open {
        draw_dropdown(f, chunks[0], &ui.menu, id);
    }
}

fn draw_menu(f: &mut Frame, area: Rect, menu: &MenuBar) {
    let mut spans = Vec::new();
    for (id, label) in MenuBar::top_labels() {
        let selected = menu.open == Some(*id);
        spans.extend(hot_spans(label, selected));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(TpTheme::menu()),
        area,
    );
}

fn draw_dropdown(f: &mut Frame, menu_area: Rect, menu: &MenuBar, id: MenuId) {
    let rect = dropdown_rect(menu_area, id);
    let items = MenuBar::items(id);
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ratatui::style::Color::Black).bg(TpTheme::MENU_BG))
        .style(TpTheme::menu());
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    for (i, (label, _)) in items.iter().enumerate() {
        let selected = i == menu.selected;
        let row = Rect {
            x: inner.x,
            y: inner.y + i as u16,
            width: inner.width,
            height: 1,
        };
        f.render_widget(Paragraph::new(Line::from(hot_spans(&format!(" {label}"), selected))), row);
    }
}

/// First non-space letter is the accelerator (red), rest black on grey.
fn hot_spans(text: &str, selected: bool) -> Vec<Span<'static>> {
    let base = if selected {
        TpTheme::menu_selected()
    } else {
        TpTheme::menu()
    };
    let hot = TpTheme::menu_hot(selected);
    let mut spans = Vec::new();
    let mut chars = text.chars();
    let mut prefix = String::new();
    let mut first = None;
    for ch in chars.by_ref() {
        if first.is_none() && !ch.is_whitespace() {
            first = Some(ch);
            break;
        }
        prefix.push(ch);
    }
    if !prefix.is_empty() {
        spans.push(Span::styled(prefix, base));
    }
    if let Some(ch) = first {
        spans.push(Span::styled(ch.to_string(), hot));
        let rest: String = chars.collect();
        if !rest.is_empty() {
            spans.push(Span::styled(rest, base));
        }
    }
    spans
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
        width: width.min(
            menu_area
                .width
                .saturating_sub(x.saturating_sub(menu_area.x)),
        ),
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
    is_rust: bool,
    stale: bool,
) {
    let (title, pane_style, border) = if is_rust {
        let stale_mark = if stale { " stale" } else { "" };
        let focus_hint = if show_cursor { "  Ctrl+C copy" } else { "" };
        (
            format!(" Rust{stale_mark}{focus_hint} "),
            TpTheme::rust_pane(),
            if show_cursor {
                Style::default().fg(ratatui::style::Color::Yellow)
            } else {
                TpTheme::frame()
            },
        )
    } else {
        (
            format!(" {} ", files::display_name(doc)),
            TpTheme::editor(),
            if show_cursor {
                Style::default().fg(ratatui::style::Color::Yellow)
            } else {
                TpTheme::frame()
            },
        )
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border)
        .title(title)
        .style(pane_style);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let widget = EditorWidget::new(doc, view)
        .highlighter(highlighter)
        .decorations(decorations)
        .style(pane_style)
        .show_cursor(show_cursor);
    f.render_widget(widget, inner);
}

fn draw_watch(f: &mut Frame, area: Rect, ui: &UiState) {
    let focused = ui.focus == Focus::Watch;
    let title = if focused {
        " Watch  (↑↓ select  Enter jump  Tab cycle) "
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
        .style(TpTheme::editor());
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
            TpTheme::editor()
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
    f.render_widget(Paragraph::new(message).style(TpTheme::menu()), area);
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
    let proj = ui
        .project_dir
        .as_ref()
        .map(|d| format!("[{}] ", files::project_title(d)))
        .unwrap_or_default();
    let unit = ui
        .current_unit_index(doc)
        .map(|i| format!("{}/{} ", i + 1, ui.units.len()))
        .unwrap_or_default();
    let left = if err_n > 0 {
        format!(" {proj}{unit}{name}{dirty}  {err_n} error(s) ")
    } else {
        format!(" {proj}{unit}{name}{dirty} ")
    };
    let focus = match ui.focus {
        Focus::Editor => "EDIT",
        Focus::Rust => "RUST",
        Focus::Watch => "WATCH",
    };
    let right = format!(" {focus}  Ln {}, Col {} ", line + 1, col + 1);
    let mid_w = area.width.saturating_sub((left.len() + right.len()) as u16);
    let mid = " ".repeat(mid_w as usize);
    let line = Line::from(vec![Span::raw(left), Span::raw(mid), Span::raw(right)]);
    f.render_widget(Paragraph::new(line).style(TpTheme::status()), area);
}

fn draw_dialog(f: &mut Frame, area: Rect, dialog: &Dialog) {
    if matches!(dialog, Dialog::Units { .. }) {
        return;
    }

    if let Some((title, lines, height)) = path_dialog_content(dialog) {
        render_dialog_box(f, area, title, Paragraph::new(lines), height);
        return;
    }

    let (title, body, height) = match dialog {
        Dialog::Open { .. }
        | Dialog::OpenProject { .. }
        | Dialog::SaveAs { .. }
        | Dialog::Units { .. } => {
            unreachable!("path/units dialogs drawn above")
        }
        Dialog::ConfirmQuit => (
            " Quit ",
            "File not saved. Quit anyway?\n\n Y = Yes   N / Esc = No".into(),
            10u16,
        ),
        Dialog::Help => (
            " Keys ",
            "F10  Menus          F1   This help\n\
             F9   Run            Alt+F9 Compile\n\
             F4   Rust pane      (TD-style, read-only + copy)\n\
             Ctrl+P Open project Ctrl+U Units list\n\
             Ctrl+F Find         F3 / Shift+F3 next/prev\n\
             Ctrl+H Replace      Ctrl+A = Replace all (in dlg)\n\
             Ctrl+O Open file    Ctrl+N New\n\
             Ctrl+Q Quit         Ctrl+Z/Y Undo/Redo\n\
             Tab  cycle panes    Enter jump to error\n\
             Mouse: menus + drag to select text"
                .into(),
            12u16,
        ),
        Dialog::About => (
            " About ",
            "TIDE — TUI IDE for Bust\n\
             Turbo Pascal vibes. Built on tide-editor.\n\
             \n\
             Esc to close"
                .into(),
            10u16,
        ),
        Dialog::Find {
            input,
            case_sensitive,
        } => {
            let cs = if *case_sensitive { "ON " } else { "off" };
            (
                " Find ",
                format!(
                    "Text to find\n\n [{input}_]\n\n\
                     Case sensitive: {cs}  (Ctrl+C toggles)\n\
                     Enter=Find  Esc=Cancel"
                ),
                11u16,
            )
        }
        Dialog::Replace {
            find,
            replace,
            field,
            case_sensitive,
        } => {
            let cs = if *case_sensitive { "ON " } else { "off" };
            let (fmark, rmark) = if *field == 0 {
                ("►", " ")
            } else {
                (" ", "►")
            };
            (
                " Replace ",
                format!(
                    "{fmark} Find    [{find}_]\n\
                     {rmark} Replace [{replace}_]\n\n\
                     Case sensitive: {cs}  (Ctrl+C toggles)\n\
                     Tab=field  Enter=Replace/Find next  Ctrl+A=All  Esc=Close"
                ),
                12u16,
            )
        }
    };

    render_dialog_box(
        f,
        area,
        title,
        Paragraph::new(body).style(TpTheme::dialog().add_modifier(Modifier::BOLD)),
        height,
    );
}

fn path_dialog_content(dialog: &Dialog) -> Option<(&'static str, Vec<Line<'_>>, u16)> {
    let (title, kind, hints, input, cwd) = match dialog {
        Dialog::Open { input, cwd } => (
            " Open file ",
            "File or folder",
            "Tab=complete  Enter=open file / enter folder  Esc=Cancel",
            input.as_str(),
            cwd,
        ),
        Dialog::OpenProject { input, cwd } => (
            " Open project ",
            "Project folder (main.vbr or several .vbr files)",
            "Tab=complete  Enter=open project / enter folder  Esc=Cancel",
            input.as_str(),
            cwd,
        ),
        Dialog::SaveAs { input, cwd } => (
            " Save as ",
            "File name",
            "Tab=complete  Enter=save / enter folder  Esc=Cancel",
            input.as_str(),
            cwd,
        ),
        _ => return None,
    };
    let bold = TpTheme::dialog().add_modifier(Modifier::BOLD);
    let grey = Style::default().bg(TpTheme::DIALOG_BG).fg(Color::DarkGray);
    Some((
        title,
        vec![
            Line::from(Span::styled(kind, bold)),
            Line::from(""),
            Line::from(Span::styled(files::display_cwd(cwd), grey)),
            Line::from(Span::styled(format!(" [{input}_]"), bold)),
            Line::from(""),
            Line::from(Span::styled(hints, bold)),
        ],
        12,
    ))
}

fn render_dialog_box(f: &mut Frame, area: Rect, title: &str, body: Paragraph, height: u16) {
    let width = 56u16.min(area.width.saturating_sub(4));
    let height = height.min(area.height.saturating_sub(4));
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
    f.render_widget(body, inner);
}

pub fn draw_units_dialog(f: &mut Frame, area: Rect, units: &[std::path::PathBuf], selected: usize) {
    let height = ((units.len() as u16) + 4).clamp(6, area.height.saturating_sub(4));
    let width = 40u16.min(area.width.saturating_sub(4));
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
        .title(" Units ")
        .border_style(Style::default().fg(TpTheme::DIALOG_FG))
        .style(TpTheme::dialog());
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    if units.is_empty() {
        f.render_widget(
            Paragraph::new(" No .vbr units.\n Open a project first (Ctrl+P).")
                .style(TpTheme::dialog()),
            inner,
        );
        return;
    }

    let visible = inner.height.saturating_sub(1) as usize;
    let start = selected.saturating_sub(visible.saturating_sub(1));
    for (row, idx) in (start..units.len()).take(visible).enumerate() {
        let label = files::unit_label(&units[idx]);
        let style = if idx == selected {
            TpTheme::menu_selected()
        } else {
            TpTheme::dialog()
        };
        let row_area = Rect {
            x: inner.x,
            y: inner.y + row as u16,
            width: inner.width,
            height: 1,
        };
        f.render_widget(Paragraph::new(format!(" {label}")).style(style), row_area);
    }
    let hint = Rect {
        x: inner.x,
        y: inner.bottom().saturating_sub(1),
        width: inner.width,
        height: 1,
    };
    f.render_widget(
        Paragraph::new(" ↑↓  Enter=Open  Esc=Cancel").style(TpTheme::dialog()),
        hint,
    );
}

/// Viewport size for scrolling (inside editor border).
pub fn editor_text_area(
    frame_area: Rect,
    watch_visible: bool,
    rust_visible: bool,
) -> (usize, usize) {
    let inner = vbr_inner_rect(frame_area, watch_visible, rust_visible);
    (inner.height.max(1) as usize, inner.width.max(1) as usize)
}

pub fn rust_text_area(
    frame_area: Rect,
    watch_visible: bool,
    rust_visible: bool,
) -> Option<(usize, usize)> {
    let inner = rust_inner_rect(frame_area, watch_visible, rust_visible)?;
    Some((inner.height.max(1) as usize, inner.width.max(1) as usize))
}

struct EditorPanes {
    vbr: Rect,
    rust: Option<Rect>,
}

fn split_editor_panes(area: Rect, rust_visible: bool) -> EditorPanes {
    if !rust_visible {
        return EditorPanes {
            vbr: area,
            rust: None,
        };
    }
    let parts =
        Layout::vertical([Constraint::Percentage(58), Constraint::Percentage(42)]).split(area);
    EditorPanes {
        vbr: parts[0],
        rust: Some(parts[1]),
    }
}

fn main_editor_chunk(frame: Rect, watch_visible: bool) -> Rect {
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
    chunks[1]
}

fn inset_border(area: Rect) -> Rect {
    Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    }
}

/// Screen rect of the Bust editor content (inside its border).
pub fn vbr_inner_rect(frame: Rect, watch_visible: bool, rust_visible: bool) -> Rect {
    let chunk = main_editor_chunk(frame, watch_visible);
    let panes = split_editor_panes(chunk, rust_visible);
    inset_border(panes.vbr)
}

/// Screen rect of the Rust pane content, if shown.
pub fn rust_inner_rect(frame: Rect, watch_visible: bool, rust_visible: bool) -> Option<Rect> {
    let chunk = main_editor_chunk(frame, watch_visible);
    let panes = split_editor_panes(chunk, rust_visible);
    Some(inset_border(panes.rust?))
}

/// Back-compat alias — Bust pane only (no Rust strip).
#[allow(dead_code)]
pub fn editor_inner_rect(frame: Rect, watch_visible: bool) -> Rect {
    vbr_inner_rect(frame, watch_visible, false)
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
    Some(inset_border(area))
}

pub fn hit_editor(
    frame: Rect,
    watch_visible: bool,
    rust_visible: bool,
    column: u16,
    row: u16,
) -> bool {
    let r = vbr_inner_rect(frame, watch_visible, rust_visible);
    column >= r.x && column < r.x + r.width && row >= r.y && row < r.y + r.height
}

pub fn hit_rust(
    frame: Rect,
    watch_visible: bool,
    rust_visible: bool,
    column: u16,
    row: u16,
) -> bool {
    let Some(r) = rust_inner_rect(frame, watch_visible, rust_visible) else {
        return false;
    };
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
