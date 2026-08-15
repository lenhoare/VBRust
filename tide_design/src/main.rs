//! tide_design — structural TUI Screen designer for Bust.

mod emit;
mod files;
mod load;
mod model;
mod preview;
mod run;
mod theme;
mod ui;

use std::env;
use std::io::{self, stdout, Stdout};
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};
use crossterm::execute;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;

use emit::{design_to_vbr, design_to_vbt};
use files::{
    default_vbt_filename, filename_tab_complete, folder_label, is_vbt, templates_dir, try_enter_dir,
    with_ext, NameFilter,
};
use load::load_template;
use model::{Design, Kind, MenuKind};
use preview::hit_test;
use ui::{
    body_layout, hit_palette, hit_test_menu, hit_tree, Dialog, FileCmd, Focus, MenuCmd, MenuHit,
    MenuId, Page, PathMode, RunCmd, Ui, ViewCmd,
};

type Term = Terminal<CrosstermBackend<Stdout>>;

fn main() -> io::Result<()> {
    let mut design = Design::default();
    let mut ui = Ui::default();
    if let Some(p) = env::args().nth(1) {
        let path = PathBuf::from(&p);
        match load_template(&path) {
            Ok(d) => {
                ui.message = format!(" Opened {}.", path.display());
                design = d;
            }
            Err(e) => ui.message = format!(" {e}"),
        }
    }

    let mut terminal = ratatui::init();
    execute!(stdout(), EnableMouseCapture)?;
    let result = event_loop(&mut terminal, &mut design, &mut ui);
    let _ = execute!(stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut Term, design: &mut Design, ui: &mut Ui) -> io::Result<()> {
    let mut frame_area = ratatui::layout::Rect::default();
    loop {
        terminal.draw(|f| {
            frame_area = f.area();
            ui::draw(f, design, ui);
        })?;

        if !event::poll(Duration::from_millis(50))? {
            continue;
        }

        match event::read()? {
            Event::Mouse(mouse) => {
                if ui.dialog.is_some() {
                    continue;
                }
                let menu_hit = hit_test_menu(frame_area, &ui.menu, mouse.column, mouse.row);
                let (pal, tree, _prev) = body_layout(frame_area);
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => match menu_hit {
                        MenuHit::Top(id) => {
                            if ui.menu.open == Some(id) {
                                ui.menu.close();
                            } else if id == MenuId::View {
                                open_view_menu(ui);
                            } else {
                                ui.menu.activate(id);
                            }
                        }
                        MenuHit::Item(cmd) => {
                            ui.menu.close();
                            if dispatch_menu(cmd, design, ui, terminal)? {
                                return Ok(());
                            }
                        }
                        MenuHit::Dismiss => ui.menu.close(),
                        MenuHit::None if ui.menu.open.is_some() => ui.menu.close(),
                        MenuHit::None => {
                            if let Some(i) = hit_palette(pal, mouse.row, ui.palette_len()) {
                                if mouse.column >= pal.x && mouse.column < pal.x + pal.width {
                                    ui.focus = Focus::Palette;
                                    ui.palette_sel = i;
                                    add_from_palette(design, ui, i);
                                }
                            } else if mouse.column >= tree.x
                                && mouse.column < tree.x + tree.width
                                && mouse.row >= tree.y
                                && mouse.row < tree.y + tree.height
                            {
                                ui.focus = Focus::Tree;
                                if let Some(id) = hit_tree(design, tree, mouse.row, ui.page) {
                                    match ui.page {
                                        Page::View => design.selected = id,
                                        Page::Menu => design.menu_selected = id,
                                    }
                                }
                            } else if ui.page == Page::View {
                                if let Some(id) =
                                    hit_test(&ui.preview_hits, mouse.column, mouse.row)
                                {
                                    ui.focus = Focus::Preview;
                                    design.selected = id;
                                }
                            }
                        }
                    },
                    MouseEventKind::Drag(MouseButton::Left) => {
                        if ui.menu.open.is_some() {
                            if let MenuHit::Item(cmd) = menu_hit {
                                if let Some(id) = ui.menu.open {
                                    if let Some(idx) = ui::MenuBar::items(id)
                                        .iter()
                                        .position(|(_, c)| *c == cmd)
                                    {
                                        ui.menu.selected = idx;
                                    }
                                }
                            }
                        }
                    }
                    MouseEventKind::ScrollUp if ui.menu.open.is_none() => {
                        if ui.focus == Focus::Tree || ui.focus == Focus::Preview {
                            select_next(design, ui, -1);
                        } else {
                            palette_move(ui, -1);
                        }
                    }
                    MouseEventKind::ScrollDown if ui.menu.open.is_none() => {
                        if ui.focus == Focus::Tree || ui.focus == Focus::Preview {
                            select_next(design, ui, 1);
                        } else {
                            palette_move(ui, 1);
                        }
                    }
                    _ => {}
                }
            }
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
                    continue;
                }
                if ui.dialog.is_some() {
                    if handle_dialog(key, design, ui)? {
                        return Ok(());
                    }
                    continue;
                }

                if ui.menu.open.is_some() {
                    if handle_menu(key, design, ui, terminal)? {
                        return Ok(());
                    }
                    continue;
                }

                match key.code {
                    KeyCode::Char('q')
                        if key.modifiers.contains(KeyModifiers::CONTROL)
                            || key.modifiers.contains(KeyModifiers::ALT) =>
                    {
                        if request_quit(design, ui) {
                            return Ok(());
                        }
                    }
                    KeyCode::Esc => {
                        if request_quit(design, ui) {
                            return Ok(());
                        }
                    }
                    KeyCode::Tab => ui.cycle_focus(),
                    KeyCode::F(4) => {
                        ui.toggle_page();
                        ui.message = match ui.page {
                            Page::View => " View → Screen.".into(),
                            Page::Menu => " View → Menu.".into(),
                        };
                    }
                    KeyCode::F(10) => {
                        ui.menu.activate(MenuId::File);
                    }
                    KeyCode::F(9) => {
                        ui.message = run::run_test(terminal, design)?;
                    }
                    KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::ALT) => {
                        ui.menu.activate(MenuId::Run);
                    }
                    KeyCode::F(2) => {
                        ui.palette_sel = 0;
                        ui.dialog = Some(Dialog::Add);
                    }
                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        do_save(design, ui);
                    }
                    KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        request_new(design, ui);
                    }
                    KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        open_path_dialog(ui, PathMode::Open, design);
                    }
                    KeyCode::Char('s') | KeyCode::Char('S')
                        if ui.page == Page::View
                            && ui.focus == Focus::Tree
                            && !key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        if let Some(n) = design.selected_node_mut() {
                            n.size = n.size.cycle();
                            let label = n.size.label();
                            design.dirty = true;
                            ui.message = format!(" Size → {label}");
                        }
                    }
                    KeyCode::Enter => match ui.focus {
                        Focus::Palette => {
                            add_from_palette(design, ui, ui.palette_sel);
                            ui.focus = Focus::Tree;
                        }
                        Focus::Tree | Focus::Preview => open_props(design, ui),
                    },
                    KeyCode::Delete | KeyCode::Backspace => match ui.page {
                        Page::View => {
                            if design.remove_selected() {
                                ui.message = " Removed.".into();
                            } else {
                                ui.message = " Cannot remove root Column.".into();
                            }
                        }
                        Page::Menu => {
                            if design.menu_remove_selected() {
                                ui.message = " Removed.".into();
                            } else {
                                ui.message = " Cannot remove the Menu bar.".into();
                            }
                        }
                    },
                    KeyCode::Up if key.modifiers.contains(KeyModifiers::ALT) => {
                        let ok = match ui.page {
                            Page::View => design.move_sibling(-1),
                            Page::Menu => design.menu_move_sibling(-1),
                        };
                        if ok {
                            ui.message = " Moved up.".into();
                        }
                    }
                    KeyCode::Down if key.modifiers.contains(KeyModifiers::ALT) => {
                        let ok = match ui.page {
                            Page::View => design.move_sibling(1),
                            Page::Menu => design.menu_move_sibling(1),
                        };
                        if ok {
                            ui.message = " Moved down.".into();
                        }
                    }
                    KeyCode::Left if key.modifiers.contains(KeyModifiers::ALT) => {
                        if ui.page == Page::View && design.move_out() {
                            ui.message = " Moved out.".into();
                        }
                    }
                    KeyCode::Right if key.modifiers.contains(KeyModifiers::ALT) => {
                        if ui.page == Page::View && design.move_in() {
                            ui.message = " Nested in.".into();
                        }
                    }
                    KeyCode::Up => match ui.focus {
                        Focus::Palette => palette_move(ui, -1),
                        _ => select_next(design, ui, -1),
                    },
                    KeyCode::Down => match ui.focus {
                        Focus::Palette => palette_move(ui, 1),
                        _ => select_next(design, ui, 1),
                    },
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

fn handle_menu(
    key: crossterm::event::KeyEvent,
    design: &mut Design,
    ui: &mut Ui,
    terminal: &mut Term,
) -> io::Result<bool> {
    match key.code {
        KeyCode::Esc => ui.menu.close(),
        KeyCode::Up => ui.menu.move_sel(-1),
        KeyCode::Down => ui.menu.move_sel(1),
        KeyCode::Left => switch_top_menu(ui, -1),
        KeyCode::Right => switch_top_menu(ui, 1),
        KeyCode::F(9) => {
            ui.menu.close();
            return dispatch_menu(MenuCmd::Run(RunCmd::Test), design, ui, terminal);
        }
        KeyCode::Enter => {
            if let Some(cmd) = ui.menu.current_cmd() {
                ui.menu.close();
                return dispatch_menu(cmd, design, ui, terminal);
            }
        }
        KeyCode::Char(c) if c.eq_ignore_ascii_case(&'f') => {
            if ui.menu.open != Some(MenuId::File) {
                ui.menu.activate(MenuId::File);
            }
        }
        KeyCode::Char(c) if c.eq_ignore_ascii_case(&'v') => {
            if ui.menu.open != Some(MenuId::View) {
                open_view_menu(ui);
            }
        }
        KeyCode::Char(c) if c.eq_ignore_ascii_case(&'r') => {
            if ui.menu.open != Some(MenuId::Run) {
                ui.menu.activate(MenuId::Run);
            }
        }
        _ => {}
    }
    Ok(false)
}

fn open_view_menu(ui: &mut Ui) {
    let sel = match ui.page {
        Page::View => 0,
        Page::Menu => 1,
    };
    ui.menu.activate_at(MenuId::View, sel);
}

fn switch_top_menu(ui: &mut Ui, delta: isize) {
    let labels = ui::MenuBar::top_labels();
    let Some(cur) = ui.menu.open else { return };
    let i = labels.iter().position(|(id, _)| *id == cur).unwrap_or(0) as isize;
    let n = labels.len() as isize;
    let mut j = i + delta;
    while j < 0 {
        j += n;
    }
    let id = labels[(j as usize) % labels.len()].0;
    match id {
        MenuId::View => open_view_menu(ui),
        other => ui.menu.activate(other),
    }
}

fn dispatch_menu(
    cmd: MenuCmd,
    design: &mut Design,
    ui: &mut Ui,
    terminal: &mut Term,
) -> io::Result<bool> {
    match cmd {
        MenuCmd::File(FileCmd::New) => request_new(design, ui),
        MenuCmd::File(FileCmd::Open) => open_path_dialog(ui, PathMode::Open, design),
        MenuCmd::File(FileCmd::Save) => do_save(design, ui),
        MenuCmd::File(FileCmd::SaveAs) => open_path_dialog(ui, PathMode::SaveVbr, design),
        MenuCmd::File(FileCmd::SaveAsTemplate) => {
            open_path_dialog(ui, PathMode::SaveVbt, design)
        }
        MenuCmd::File(FileCmd::Quit) => return Ok(request_quit(design, ui)),
        MenuCmd::View(ViewCmd::Screen) => {
            ui.set_page(Page::View);
            ui.message = " View → Screen.".into();
        }
        MenuCmd::View(ViewCmd::Menu) => {
            ui.set_page(Page::Menu);
            ui.message = " View → Menu.".into();
        }
        MenuCmd::Run(RunCmd::Test) => {
            ui.message = run::run_test(terminal, design)?;
        }
    }
    Ok(false)
}

fn open_path_dialog(ui: &mut Ui, mode: PathMode, design: &Design) {
    ui.path_tab = None;
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let (dir, input) = match mode {
        PathMode::Open => {
            let dir = design
                .path
                .as_ref()
                .filter(|p| is_vbt(p))
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .filter(|d| d.is_dir())
                .unwrap_or_else(templates_dir);
            (dir, String::new())
        }
        PathMode::SaveVbr => {
            let dir = design
                .path
                .as_ref()
                .filter(|p| !is_vbt(p))
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .filter(|d| d.is_dir())
                .unwrap_or(cwd);
            let name = design
                .path
                .as_ref()
                .filter(|p| !is_vbt(p))
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| format!("{}.vbr", design.screen_name.to_ascii_lowercase()));
            (dir, name)
        }
        PathMode::SaveVbt => {
            let dir = design
                .path
                .as_ref()
                .filter(|p| is_vbt(p))
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .filter(|d| d.is_dir())
                .unwrap_or_else(templates_dir);
            let name = design
                .path
                .as_ref()
                .filter(|p| is_vbt(p))
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| default_vbt_filename(&design.screen_name));
            (dir, name)
        }
    };
    ui.message = format!(" In {}.", folder_label(&dir));
    ui.dialog = Some(Dialog::Path { mode, dir, input });
}

fn palette_move(ui: &mut Ui, delta: isize) {
    let n = ui.palette_len() as isize;
    if n == 0 {
        return;
    }
    let mut i = ui.palette_sel as isize + delta;
    while i < 0 {
        i += n;
    }
    ui.palette_sel = (i % n) as usize;
}

fn select_next(design: &mut Design, ui: &Ui, delta: isize) {
    match ui.page {
        Page::View => design.select_next(delta),
        Page::Menu => design.menu_select_next(delta),
    }
}

fn add_from_palette(design: &mut Design, ui: &mut Ui, i: usize) {
    match ui.page {
        Page::View => {
            let kind = Kind::palette()[i];
            if design.add_child(kind) {
                ui.message = format!(" Added {}.", kind.label());
            }
        }
        Page::Menu => {
            let kind = MenuKind::palette()[i];
            if design.menu_add(kind) {
                ui.message = format!(" Added {}.", kind.label());
            }
        }
    }
}

fn open_props(design: &Design, ui: &mut Ui) {
    match ui.page {
        Page::View => {
            let Some(n) = design.selected_node() else {
                return;
            };
            ui.dialog = Some(Dialog::Props {
                text: n.text.clone(),
                field: n.field.clone(),
                event: n.event.clone(),
                option: n.option.clone(),
                size: n.size.clone(),
                field_i: 0,
            });
        }
        Page::Menu => {
            let Some(n) = design.menu_selected_node() else {
                return;
            };
            match n.kind {
                crate::model::MenuKind::Bar | crate::model::MenuKind::Separator => {
                    ui.message = " Nothing to edit on this row.".into();
                }
                crate::model::MenuKind::Menu => {
                    ui.dialog = Some(Dialog::MenuProps {
                        text: n.text.clone(),
                        event: String::new(),
                        field_i: 0,
                        has_event: false,
                    });
                }
                crate::model::MenuKind::Item => {
                    ui.dialog = Some(Dialog::MenuProps {
                        text: n.text.clone(),
                        event: n.event.clone(),
                        field_i: 0,
                        has_event: true,
                    });
                }
            }
        }
    }
}

fn handle_dialog(
    key: crossterm::event::KeyEvent,
    design: &mut Design,
    ui: &mut Ui,
) -> io::Result<bool> {
    let Some(dialog) = ui.dialog.clone() else {
        return Ok(false);
    };
    match dialog {
        Dialog::QuitConfirm => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => return Ok(true),
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => ui.dialog = None,
            _ => {}
        },
        Dialog::Add => match key.code {
            KeyCode::Esc => ui.dialog = None,
            KeyCode::Up => {
                palette_move(ui, -1);
                ui.dialog = Some(Dialog::Add);
            }
            KeyCode::Down => {
                palette_move(ui, 1);
                ui.dialog = Some(Dialog::Add);
            }
            KeyCode::Enter => {
                ui.dialog = None;
                add_from_palette(design, ui, ui.palette_sel);
                ui.focus = Focus::Tree;
            }
            _ => {}
        },
        Dialog::Code { mut scroll } => match key.code {
            KeyCode::Esc => ui.dialog = None,
            KeyCode::Up => {
                scroll = scroll.saturating_sub(1);
                ui.dialog = Some(Dialog::Code { scroll });
            }
            KeyCode::Down => {
                scroll += 1;
                ui.dialog = Some(Dialog::Code { scroll });
            }
            _ => {}
        },
        Dialog::ConfirmNew => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                ui.dialog = None;
                do_new(design, ui);
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => ui.dialog = None,
            _ => {}
        },
        Dialog::ConfirmOpen { path } => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                ui.dialog = None;
                ui.path_tab = None;
                do_open(design, ui, PathBuf::from(path));
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => ui.dialog = None,
            _ => {}
        },
        Dialog::Path {
            mode,
            dir,
            mut input,
        } => match key.code {
            KeyCode::Esc => {
                ui.path_tab = None;
                ui.dialog = None;
            }
            KeyCode::Enter => {
                ui.path_tab = None;
                if let Some(next) = try_enter_dir(&dir, &input) {
                    ui.message = format!(" In {}.", folder_label(&next));
                    ui.dialog = Some(Dialog::Path {
                        mode,
                        dir: next,
                        input: String::new(),
                    });
                } else if input.trim().is_empty() {
                    ui.message = " Tab to pick a file, or type a name.".into();
                    ui.dialog = Some(Dialog::Path { mode, dir, input });
                } else {
                    ui.dialog = None;
                    let mut path = dir.join(input.trim());
                    match mode {
                        PathMode::Open => {
                            path = with_ext(path, "vbt");
                            if design.dirty {
                                ui.dialog = Some(Dialog::ConfirmOpen {
                                    path: path.display().to_string(),
                                });
                            } else {
                                do_open(design, ui, path);
                            }
                        }
                        PathMode::SaveVbr => {
                            path.set_extension("vbr");
                            design.path = Some(path);
                            do_save(design, ui);
                        }
                        PathMode::SaveVbt => {
                            path.set_extension("vbt");
                            design.path = Some(path);
                            do_save(design, ui);
                        }
                    }
                }
            }
            KeyCode::Tab | KeyCode::BackTab => {
                let reverse = matches!(key.code, KeyCode::BackTab)
                    || key.modifiers.contains(KeyModifiers::SHIFT);
                let filter = match mode {
                    PathMode::Open => NameFilter::Templates,
                    PathMode::SaveVbr | PathMode::SaveVbt => NameFilter::All,
                };
                let (next, msg) =
                    filename_tab_complete(&dir, &input, &mut ui.path_tab, reverse, filter);
                input = next;
                if !msg.is_empty() {
                    ui.message = msg;
                }
                ui.dialog = Some(Dialog::Path { mode, dir, input });
            }
            KeyCode::Backspace => {
                ui.path_tab = None;
                input.pop();
                ui.dialog = Some(Dialog::Path { mode, dir, input });
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                ui.path_tab = None;
                input.push(c);
                ui.dialog = Some(Dialog::Path { mode, dir, input });
            }
            _ => {}
        },
        Dialog::Props {
            mut text,
            mut field,
            mut event,
            mut option,
            mut size,
            mut field_i,
        } => {
            let is_radio = design
                .selected_node()
                .map(|n| n.kind == Kind::Radio)
                .unwrap_or(false);
            let n_fields: u8 = if is_radio { 5 } else { 4 };
            let size_i = n_fields - 1;
            let rebuild = |text, field, event, option, size, field_i| Dialog::Props {
                text,
                field,
                event,
                option,
                size,
                field_i,
            };
            match key.code {
                KeyCode::Esc => ui.dialog = None,
                KeyCode::Tab => {
                    field_i = (field_i + 1) % n_fields;
                    ui.dialog = Some(rebuild(text, field, event, option, size, field_i));
                }
                KeyCode::BackTab => {
                    field_i = (field_i + n_fields - 1) % n_fields;
                    ui.dialog = Some(rebuild(text, field, event, option, size, field_i));
                }
                KeyCode::Left | KeyCode::Right if field_i == size_i => {
                    size = if matches!(key.code, KeyCode::Left) {
                        size.cycle_back()
                    } else {
                        size.cycle()
                    };
                    ui.dialog = Some(rebuild(text, field, event, option, size, field_i));
                }
                KeyCode::Enter => {
                    ui.dialog = None;
                    if let Some(n) = design.selected_node_mut() {
                        n.text = text;
                        n.field = field;
                        n.event = event;
                        n.option = option;
                        n.size = size;
                        design.dirty = true;
                        ui.message = " Properties updated.".into();
                    }
                }
                KeyCode::Backspace if field_i < size_i => {
                    match field_i {
                        0 => {
                            text.pop();
                        }
                        1 => {
                            field.pop();
                        }
                        2 => {
                            event.pop();
                        }
                        _ => {
                            option.pop();
                        }
                    }
                    ui.dialog = Some(rebuild(text, field, event, option, size, field_i));
                }
                KeyCode::Char(c)
                    if field_i < size_i && !key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    match field_i {
                        0 => text.push(c),
                        1 => field.push(c),
                        2 => event.push(c),
                        _ => option.push(c),
                    }
                    ui.dialog = Some(rebuild(text, field, event, option, size, field_i));
                }
                _ => {}
            }
        }
        Dialog::MenuProps {
            mut text,
            mut event,
            mut field_i,
            has_event,
        } => {
            let n_fields: u8 = if has_event { 2 } else { 1 };
            let rebuild = |text, event, field_i| Dialog::MenuProps {
                text,
                event,
                field_i,
                has_event,
            };
            match key.code {
                KeyCode::Esc => ui.dialog = None,
                KeyCode::Tab => {
                    field_i = (field_i + 1) % n_fields;
                    ui.dialog = Some(rebuild(text, event, field_i));
                }
                KeyCode::BackTab => {
                    field_i = (field_i + n_fields - 1) % n_fields;
                    ui.dialog = Some(rebuild(text, event, field_i));
                }
                KeyCode::Enter => {
                    ui.dialog = None;
                    if let Some(n) = design.menu_selected_node_mut() {
                        n.text = text;
                        if has_event {
                            n.event = event;
                        }
                        design.dirty = true;
                        ui.message = " Properties updated.".into();
                    }
                }
                KeyCode::Backspace => {
                    if field_i == 0 {
                        text.pop();
                    } else {
                        event.pop();
                    }
                    ui.dialog = Some(rebuild(text, event, field_i));
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if field_i == 0 {
                        text.push(c);
                    } else {
                        event.push(c);
                    }
                    ui.dialog = Some(rebuild(text, event, field_i));
                }
                _ => {}
            }
        }
    }
    Ok(false)
}

fn request_quit(design: &Design, ui: &mut Ui) -> bool {
    if design.dirty {
        ui.dialog = Some(Dialog::QuitConfirm);
        false
    } else {
        true
    }
}

fn request_new(design: &mut Design, ui: &mut Ui) {
    if design.dirty {
        ui.dialog = Some(Dialog::ConfirmNew);
    } else {
        do_new(design, ui);
    }
}

fn do_new(design: &mut Design, ui: &mut Ui) {
    *design = Design::default();
    ui.set_page(Page::View);
    ui.focus = Focus::Tree;
    ui.palette_sel = 0;
    ui.message = " New design.".into();
}

fn do_open(design: &mut Design, ui: &mut Ui, path: PathBuf) {
    match load_template(&path) {
        Ok(d) => {
            ui.message = format!(" Opened {}.", path.display());
            *design = d;
        }
        Err(e) => ui.message = format!(" {e}"),
    }
}

fn do_save(design: &mut Design, ui: &mut Ui) {
    if design.path.is_none() {
        open_path_dialog(ui, PathMode::SaveVbr, design);
        return;
    }
    let path = design.path.clone().unwrap();
    let code = if is_vbt(&path) {
        design_to_vbt(design)
    } else {
        design_to_vbr(design)
    };
    match std::fs::write(&path, code) {
        Ok(()) => {
            design.dirty = false;
            ui.message = format!(" Saved {}.", path.display());
        }
        Err(e) => ui.message = format!(" Save failed: {e}"),
    }
}
