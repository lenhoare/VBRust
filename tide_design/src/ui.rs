//! Designer chrome: palette | tree | preview, dialogs, status.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::emit;
use crate::model::{Design, Kind, MenuKind};
use crate::preview::{self, HitList};
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuId {
    File,
    View,
    Run,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileCmd {
    New,
    Open,
    Save,
    SaveAs,
    SaveAsTemplate,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewCmd {
    Screen,
    Menu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunCmd {
    Test,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuCmd {
    File(FileCmd),
    View(ViewCmd),
    Run(RunCmd),
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
                ("New               Ctrl+N", MenuCmd::File(FileCmd::New)),
                ("Open template...  Ctrl+O", MenuCmd::File(FileCmd::Open)),
                ("Save              Ctrl+S", MenuCmd::File(FileCmd::Save)),
                ("Save as...", MenuCmd::File(FileCmd::SaveAs)),
                ("Save as template...", MenuCmd::File(FileCmd::SaveAsTemplate)),
                ("Quit              Ctrl+Q", MenuCmd::File(FileCmd::Quit)),
            ],
            MenuId::View => &[
                ("Screen", MenuCmd::View(ViewCmd::Screen)),
                ("Menu", MenuCmd::View(ViewCmd::Menu)),
            ],
            MenuId::Run => &[("Test             F9", MenuCmd::Run(RunCmd::Test))],
        }
    }

    pub fn top_labels() -> &'static [(MenuId, &'static str)] {
        &[
            (MenuId::File, " File "),
            (MenuId::View, " View "),
            (MenuId::Run, " Run "),
        ]
    }

    pub fn activate(&mut self, id: MenuId) {
        self.open = Some(id);
        self.selected = 0;
    }

    pub fn activate_at(&mut self, id: MenuId, selected: usize) {
        self.open = Some(id);
        let n = Self::items(id).len();
        self.selected = if n == 0 { 0 } else { selected.min(n - 1) };
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

    pub fn current_cmd(&self) -> Option<MenuCmd> {
        let id = self.open?;
        Self::items(id).get(self.selected).map(|(_, c)| *c)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuHit {
    Top(MenuId),
    Item(MenuCmd),
    Dismiss,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    View,
    Menu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Palette,
    Tree,
    Preview,
}

#[derive(Debug, Clone)]
pub enum Dialog {
    Props {
        text: String,
        field: String,
        event: String,
        option: String,
        size: crate::model::SizeHint,
        /// 0 text, 1 field, 2 event, 3 option (Radio) or size, 4 size (Radio)
        field_i: u8,
    },
    MenuProps {
        text: String,
        event: String,
        /// 0 title/label, 1 event (items only)
        field_i: u8,
        has_event: bool,
    },
    Add,
    /// Emitted VBR peek — parked until a View/Help menu exists (F10 is File now).
    #[allow(dead_code)]
    Code { scroll: usize },
    Path {
        mode: PathMode,
        dir: std::path::PathBuf,
        input: String,
    },
    ConfirmOpen { path: String },
    ConfirmNew,
    QuitConfirm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathMode {
    Open,
    SaveVbr,
    SaveVbt,
}

impl PathMode {
    pub fn title(self) -> &'static str {
        match self {
            PathMode::Open => " Open template ",
            PathMode::SaveVbr => " Save as ",
            PathMode::SaveVbt => " Save as template ",
        }
    }

    pub fn hints(self) -> &'static str {
        match self {
            PathMode::Open => "Tab=cycle files  Enter=open  Esc=Cancel",
            PathMode::SaveVbr | PathMode::SaveVbt => {
                "Tab=cycle files  Enter=save  Esc=Cancel"
            }
        }
    }
}

pub struct Ui {
    pub menu: MenuBar,
    pub page: Page,
    pub focus: Focus,
    pub palette_sel: usize,
    pub message: String,
    pub dialog: Option<Dialog>,
    pub preview_hits: HitList,
    pub path_tab: Option<crate::files::PathTabState>,
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            menu: MenuBar::default(),
            page: Page::View,
            focus: Focus::Tree,
            palette_sel: 0,
            message: " F10 File  F9 Test  F2 Add ".into(),
            dialog: None,
            preview_hits: Vec::new(),
            path_tab: None,
        }
    }
}

impl Ui {
    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Palette => Focus::Tree,
            Focus::Tree => Focus::Preview,
            Focus::Preview => Focus::Palette,
        };
    }

    pub fn set_page(&mut self, page: Page) {
        if self.page == page {
            return;
        }
        self.page = page;
        self.palette_sel = 0;
        self.focus = Focus::Tree;
    }

    pub fn toggle_page(&mut self) {
        self.set_page(match self.page {
            Page::View => Page::Menu,
            Page::Menu => Page::View,
        });
    }

    pub fn palette_len(&self) -> usize {
        match self.page {
            Page::View => Kind::palette().len(),
            Page::Menu => MenuKind::palette().len(),
        }
    }
}

pub fn draw(f: &mut Frame, design: &Design, ui: &mut Ui) {
    let area = f.area();
    f.render_widget(Block::default().style(Theme::desktop()), area);

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);

    draw_menu(f, chunks[0], &ui.menu);
    draw_body(f, chunks[1], design, ui);
    draw_message(f, chunks[2], &ui.message);
    draw_status(f, chunks[3], design, ui);

    if let Some(id) = ui.menu.open {
        draw_dropdown(f, chunks[0], &ui.menu, id);
    }

    if let Some(d) = &ui.dialog {
        let pal_sel = ui.palette_sel;
        draw_dialog(f, area, design, d, pal_sel, ui.page);
    }
}

fn draw_menu(f: &mut Frame, area: Rect, menu: &MenuBar) {
    let mut spans = Vec::new();
    for (id, label) in MenuBar::top_labels() {
        let style = if menu.open == Some(*id) {
            Theme::menu_selected()
        } else {
            Theme::menu()
        };
        spans.push(Span::styled(*label, style));
    }
    f.render_widget(Paragraph::new(Line::from(spans)).style(Theme::menu()), area);
}

fn draw_dropdown(f: &mut Frame, menu_area: Rect, menu: &MenuBar, id: MenuId) {
    let rect = dropdown_rect(menu_area, id);
    let items = MenuBar::items(id);
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Theme::frame())
        .style(Theme::menu());
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    for (i, (label, _)) in items.iter().enumerate() {
        let style = if i == menu.selected {
            Theme::menu_selected()
        } else {
            Theme::menu()
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

fn menu_bar_rect(frame: Rect) -> Rect {
    Rect {
        x: frame.x,
        y: frame.y,
        width: frame.width,
        height: 1,
    }
}

fn top_menu_label_rect(menu_area: Rect, id: MenuId) -> Rect {
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

pub fn hit_test_menu(frame: Rect, menu: &MenuBar, column: u16, row: u16) -> MenuHit {
    let bar = menu_bar_rect(frame);

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
            if row > drop.y && row < drop.y + drop.height - 1 {
                let idx = (row - drop.y - 1) as usize;
                if let Some((_, cmd)) = MenuBar::items(id).get(idx) {
                    return MenuHit::Item(*cmd);
                }
            }
            return MenuHit::Dismiss;
        }
        return MenuHit::Dismiss;
    }

    MenuHit::None
}

fn draw_body(f: &mut Frame, area: Rect, design: &Design, ui: &mut Ui) {
    let cols = Layout::horizontal([
        Constraint::Length(18),
        Constraint::Length(28),
        Constraint::Min(20),
    ])
    .split(area);

    draw_palette(f, cols[0], ui);
    draw_tree(f, cols[1], design, ui);
    preview::draw_preview(f, cols[2], design, &mut ui.preview_hits);
}

fn draw_palette(f: &mut Frame, area: Rect, ui: &Ui) {
    let focused = ui.focus == Focus::Palette;
    let border = if focused {
        ratatui::style::Style::default().fg(ratatui::style::Color::Yellow)
    } else {
        Theme::frame()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border)
        .title(if ui.page == Page::Menu {
            " Menu "
        } else {
            " Components "
        })
        .style(Theme::panel());
    let inner = block.inner(area);
    f.render_widget(block, area);

    match ui.page {
        Page::View => {
            for (i, kind) in Kind::palette().iter().enumerate() {
                if i as u16 >= inner.height {
                    break;
                }
                paint_palette_row(f, inner, i, kind.label(), i == ui.palette_sel, focused);
            }
        }
        Page::Menu => {
            for (i, kind) in MenuKind::palette().iter().enumerate() {
                if i as u16 >= inner.height {
                    break;
                }
                paint_palette_row(f, inner, i, kind.label(), i == ui.palette_sel, focused);
            }
        }
    }
}

fn paint_palette_row(f: &mut Frame, inner: Rect, i: usize, label: &str, selected: bool, focused: bool) {
    let style = if selected && focused {
        Theme::selected()
    } else if selected {
        Theme::highlight()
    } else {
        Theme::panel()
    };
    let row = Rect {
        x: inner.x,
        y: inner.y + i as u16,
        width: inner.width,
        height: 1,
    };
    f.render_widget(Paragraph::new(format!(" {label} ")).style(style), row);
}

fn draw_tree(f: &mut Frame, area: Rect, design: &Design, ui: &Ui) {
    let focused = ui.focus == Focus::Tree;
    let border = if focused {
        ratatui::style::Style::default().fg(ratatui::style::Color::Yellow)
    } else {
        Theme::frame()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border)
        .title(if ui.page == Page::Menu {
            " Menu "
        } else {
            " Structure "
        })
        .style(Theme::panel());
    let inner = block.inner(area);
    f.render_widget(block, area);

    let flat = match ui.page {
        Page::View => design.flat_tree(),
        Page::Menu => design.menu_flat_tree(),
    };
    let selected_id = match ui.page {
        Page::View => design.selected,
        Page::Menu => design.menu_selected,
    };
    let sel_i = flat
        .iter()
        .position(|(id, _, _)| *id == selected_id)
        .unwrap_or(0);
    let visible = inner.height as usize;
    let start = sel_i.saturating_sub(visible.saturating_sub(1));

    for (row, (id, depth, label)) in flat.iter().skip(start).take(visible).enumerate() {
        let selected = *id == selected_id;
        let style = if selected && focused {
            Theme::selected()
        } else if selected {
            Theme::highlight()
        } else {
            Theme::panel()
        };
        let pad = "  ".repeat(*depth);
        let mark = if selected { ">" } else { " " };
        let text = truncate(&format!("{mark}{pad}{label}"), inner.width as usize);
        let row_area = Rect {
            x: inner.x,
            y: inner.y + row as u16,
            width: inner.width,
            height: 1,
        };
        f.render_widget(Paragraph::new(text).style(style), row_area);
    }
}

fn draw_message(f: &mut Frame, area: Rect, message: &str) {
    f.render_widget(Paragraph::new(message).style(Theme::panel()), area);
}

fn draw_status(f: &mut Frame, area: Rect, design: &Design, ui: &Ui) {
    let page = match ui.page {
        Page::View => "SCREEN",
        Page::Menu => "MENU",
    };
    let focus = match ui.focus {
        Focus::Palette => "PALETTE",
        Focus::Tree => "TREE",
        Focus::Preview => "PREVIEW",
    };
    let dirty = if design.dirty { "*" } else { " " };
    let path = design
        .path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| format!("{} (unsaved)", design.screen_name));
    let left = format!(" {path}{dirty} ");
    let right = format!(" {page} {focus} ");
    let mid_w = area
        .width
        .saturating_sub((left.len() + right.len()) as u16);
    let line = Line::from(vec![
        Span::raw(left),
        Span::raw(" ".repeat(mid_w as usize)),
        Span::raw(right),
    ]);
    f.render_widget(Paragraph::new(line).style(Theme::status()), area);
}

fn draw_dialog(
    f: &mut Frame,
    area: Rect,
    design: &Design,
    dialog: &Dialog,
    palette_sel: usize,
    page: Page,
) {
    match dialog {
        Dialog::Add => {
            let body = match page {
                Page::View => Kind::palette()
                    .iter()
                    .enumerate()
                    .map(|(i, k)| {
                        let mark = if i == palette_sel { ">" } else { " " };
                        format!("{mark} {}", k.label())
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                Page::Menu => MenuKind::palette()
                    .iter()
                    .enumerate()
                    .map(|(i, k)| {
                        let mark = if i == palette_sel { ">" } else { " " };
                        format!("{mark} {}", k.label())
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            };
            let h = match page {
                Page::View => 23,
                Page::Menu => 8,
            };
            popup(
                f,
                area,
                if page == Page::Menu {
                    " Add menu "
                } else {
                    " Add component "
                },
                &format!("{body}\n\n↑↓  Enter=insert  Esc=cancel"),
                18,
                h,
            );
        }
        Dialog::Props {
            text,
            field,
            event,
            option,
            size,
            field_i,
        } => {
            let is_radio = matches!(
                design.selected_node().map(|n| n.kind),
                Some(crate::model::Kind::Radio)
            );
            let size_i: u8 = if is_radio { 4 } else { 3 };
            let marks = |i: u8| if *field_i == i { ">" } else { " " };
            let size_hint = if *field_i == size_i {
                "  ←→ cycle"
            } else {
                ""
            };
            let auto = match size {
                crate::model::SizeHint::Default => {
                    format!(" (= {})", design.selected_node().map(|n| n.kind.auto_size().label()).unwrap_or_default())
                }
                _ => String::new(),
            };
            let text_lbl = match design.selected_node().map(|n| n.kind) {
                Some(crate::model::Kind::Frame | crate::model::Kind::Tab) => "Title",
                Some(crate::model::Kind::Button | crate::model::Kind::Checkbox | crate::model::Kind::Radio) => {
                    "Label"
                }
                _ => "Text ",
            };
            let mut body = format!(
                "{} {text_lbl}  [{text}_]\n\
                 {} Field  [{field}_]\n\
                 {} Event  [{event}_]\n",
                marks(0),
                marks(1),
                marks(2),
            );
            if is_radio {
                body.push_str(&format!("{} Option [{option}_]\n", marks(3)));
            }
            body.push_str(&format!(
                "{} Size   [{}]{auto}{size_hint}\n\n\
                 Tab=next field   Enter=apply   Esc=cancel",
                marks(size_i),
                size.label(),
            ));
            popup(f, area, " Properties ", &body, 56, if is_radio { 12 } else { 11 });
        }
        Dialog::MenuProps {
            text,
            event,
            field_i,
            has_event,
        } => {
            let marks = |i: u8| if *field_i == i { ">" } else { " " };
            let mut body = format!("{} Title  [{text}_]\n", marks(0));
            if *has_event {
                body.push_str(&format!("{} Event  [{event}_]\n", marks(1)));
            }
            body.push_str("\nTab=next field   Enter=apply   Esc=cancel");
            popup(f, area, " Menu properties ", &body, 56, if *has_event { 8 } else { 7 });
        }
        Dialog::Code { scroll } => {
            let code = emit::design_to_vbr(design);
            let lines: Vec<&str> = code.lines().collect();
            let view: String = lines
                .iter()
                .skip(*scroll)
                .take(18)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n");
            popup(
                f,
                area,
                " Emitted VBR (F10) ",
                &format!("{view}\n\n↑↓ scroll  Esc=close"),
                70,
                22,
            );
        }
        Dialog::Path { mode, dir, input } => {
            let folder = crate::files::folder_label(dir);
            popup(
                f,
                area,
                mode.title(),
                &format!(
                    "In {folder}\n\n [{input}_]\n\n{}",
                    mode.hints()
                ),
                52,
                10,
            );
        }
        Dialog::ConfirmOpen { .. } => {
            popup(
                f,
                area,
                " Open ",
                "Design not saved. Open template anyway?\n\n Y = Yes   N / Esc = No",
                48,
                7,
            );
        }
        Dialog::ConfirmNew => {
            popup(
                f,
                area,
                " New ",
                "Design not saved. Start a new design anyway?\n\n Y = Yes   N / Esc = No",
                52,
                7,
            );
        }
        Dialog::QuitConfirm => {
            popup(
                f,
                area,
                " Quit ",
                "Design not saved. Quit anyway?\n\n Y = Yes   N / Esc = No",
                40,
                7,
            );
        }
    }
}

fn popup(f: &mut Frame, area: Rect, title: &str, body: &str, w: u16, h: u16) {
    let w = w.min(area.width.saturating_sub(2));
    let h = h.min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Theme::frame())
        .style(Theme::dialog());
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    f.render_widget(Paragraph::new(body).style(Theme::dialog()), inner);
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

/// Layout rects for mouse hit-testing (matching draw_body).
pub fn body_layout(frame: Rect) -> (Rect, Rect, Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(frame);
    let cols = Layout::horizontal([
        Constraint::Length(18),
        Constraint::Length(28),
        Constraint::Min(20),
    ])
    .split(chunks[1]);
    (inset(cols[0]), inset(cols[1]), inset(cols[2]))
}

fn inset(r: Rect) -> Rect {
    Rect {
        x: r.x.saturating_add(1),
        y: r.y.saturating_add(1),
        width: r.width.saturating_sub(2),
        height: r.height.saturating_sub(2),
    }
}

pub fn hit_palette(inner: Rect, row: u16, n: usize) -> Option<usize> {
    if row < inner.y || row >= inner.y + inner.height {
        return None;
    }
    let i = (row - inner.y) as usize;
    if i < n {
        Some(i)
    } else {
        None
    }
}

pub fn hit_tree(design: &Design, inner: Rect, row: u16, page: Page) -> Option<usize> {
    if row < inner.y || row >= inner.y + inner.height {
        return None;
    }
    let (flat, selected) = match page {
        Page::View => (design.flat_tree(), design.selected),
        Page::Menu => (design.menu_flat_tree(), design.menu_selected),
    };
    let sel_i = flat
        .iter()
        .position(|(id, _, _)| *id == selected)
        .unwrap_or(0);
    let visible = inner.height as usize;
    let start = sel_i.saturating_sub(visible.saturating_sub(1));
    let idx = start + (row - inner.y) as usize;
    flat.get(idx).map(|(id, _, _)| *id)
}
