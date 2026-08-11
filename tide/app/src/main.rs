//! TIDE — Turbo Pascal–inspired TUI IDE for VBR.

mod compile;
mod files;
mod find;
mod run;
mod theme;
mod ui;

use std::env;
use std::io::{self, stdout, Stdout};
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use crossterm::execute;
use ratatui::layout::Rect;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use tide_editor::{
    is_ctrl, Document, EditorView, Key, KeyEvent, KeyMods, KeywordHighlighter,
};

use files::{
    default_untitled, detect_project, display_name, list_units, open_path, path_enter_dir,
    path_tab_complete, project_entry, project_title, resolve_save_path, save_document, unit_label,
};
use ui::{
    editor_inner_rect, editor_text_area, hit_editor, hit_watch, Dialog, EditCmd, FileCmd, Focus,
    HelpCmd, MenuCmd, MenuHit, MenuId, RunCmd, UiState,
};

type TideTerminal = Terminal<CrosstermBackend<Stdout>>;

fn vbr_highlighter() -> KeywordHighlighter {
    KeywordHighlighter::new([
        "Function",
        "End",
        "Sub",
        "Dim",
        "As",
        "If",
        "Then",
        "Else",
        "ElseIf",
        "For",
        "To",
        "Step",
        "Next",
        "Each",
        "In",
        "Do",
        "Loop",
        "While",
        "Until",
        "Match",
        "Return",
        "Public",
        "Private",
        "Type",
        "Enum",
        "Const",
        "True",
        "False",
        "And",
        "Or",
        "Not",
        "Xor",
        "Mod",
        "ByVal",
        "ByRef",
        "Set",
        "Mut",
        "Nothing",
        "New",
        "Me",
        "Exit",
        "Continue",
        "Use",
        "Rust",
        "Python",
        "Text",
        "Test",
        "Assert",
        "Screen",
        "Window",
        "Page",
        "State",
        "View",
        "Events",
        "On",
        "Every",
        "Await",
        "Result",
        "Option",
        "Ok",
        "Err",
        "Some",
        "None",
        "Integer",
        "Long",
        "LongLong",
        "Single",
        "Double",
        "Boolean",
        "Byte",
        "String",
        "Vec",
        "HashMap",
        "Debug",
        "Print",
        "Log",
        "Sleep",
        "MsgBox",
        "InputBox",
    ])
    .with_line_comment("'")
}

fn main() -> io::Result<()> {
    let mut ui = UiState::default();
    let mut view = EditorView::new();
    let mut doc = match env::args().nth(1) {
        Some(p) => {
            let path = PathBuf::from(&p);
            if path.is_dir() {
                match open_project_into(&path, &mut ui) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("{e}");
                        default_untitled()
                    }
                }
            } else {
                match open_path(&path) {
                    Ok(d) => {
                        attach_project_for_doc(&d, &mut ui);
                        d
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        default_untitled()
                    }
                }
            }
        }
        None => default_untitled(),
    };
    let highlighter = vbr_highlighter();

    let mut terminal = ratatui::init();
    execute!(stdout(), EnableMouseCapture, EnableBracketedPaste)?;
    let result = event_loop(&mut terminal, &mut doc, &mut view, &mut ui, &highlighter);
    let _ = execute!(stdout(), DisableMouseCapture, DisableBracketedPaste);
    ratatui::restore();
    result
}

fn event_loop(
    terminal: &mut TideTerminal,
    doc: &mut Document,
    view: &mut EditorView,
    ui: &mut UiState,
    highlighter: &KeywordHighlighter,
) -> io::Result<()> {
    loop {
        let mut frame_area = Rect::default();
        let watch_vis = ui.watch_visible();
        let mut decos = compile::decorations_for(&ui.diagnostics);
        decos.extend(find::match_decorations(doc, &ui.find));
        terminal.draw(|f| {
            frame_area = f.area();
            let (h, w) = editor_text_area(f.area(), watch_vis);
            let gw = view.gutter_width(doc) as usize;
            view.ensure_visible(doc, h, w.saturating_sub(gw).max(1));
            ui::draw(f, doc, view, highlighter, ui, &decos);
        })?;

        if !event::poll(Duration::from_millis(50))? {
            continue;
        }

        match event::read()? {
            Event::Paste(text) => {
                if ui.dialog.is_none() && ui.menu.open.is_none() {
                    ui.focus = Focus::Editor;
                    view.insert_text(doc, &text);
                }
            }
            Event::Mouse(mouse) => {
                if ui.dialog.is_some() {
                    continue;
                }

                let menu_hit = ui::hit_test_menu(frame_area, &ui.menu, mouse.column, mouse.row);
                let in_editor = hit_editor(frame_area, watch_vis, mouse.column, mouse.row);
                let editor_area = editor_inner_rect(frame_area, watch_vis);
                let watch_row = hit_watch(frame_area, watch_vis, mouse.column, mouse.row);

                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        match menu_hit {
                            MenuHit::Top(id) => {
                                view.mouse_up();
                                if ui.menu.open == Some(id) {
                                    ui.menu.close();
                                } else {
                                    ui.menu.activate(id);
                                }
                            }
                            MenuHit::Item(cmd) => {
                                view.mouse_up();
                                ui.menu.close();
                                if dispatch(terminal, cmd, doc, view, ui)? {
                                    return Ok(());
                                }
                            }
                            MenuHit::Dismiss => {
                                view.mouse_up();
                                ui.menu.close();
                            }
                            MenuHit::None if ui.menu.open.is_some() => {
                                view.mouse_up();
                                ui.menu.close();
                            }
                            MenuHit::None if watch_row.is_some() => {
                                view.mouse_up();
                                let row = watch_row.unwrap();
                                if row < ui.diagnostics.len() {
                                    ui.watch_selected = row;
                                    ui.focus = Focus::Watch;
                                    jump_to_selected(doc, view, ui);
                                }
                            }
                            MenuHit::None if in_editor => {
                                ui.focus = Focus::Editor;
                                view.mouse_down(doc, editor_area, mouse.column, mouse.row);
                            }
                            MenuHit::None => {}
                        }
                    }
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
                        } else if view.is_mouse_selecting() || in_editor {
                            view.mouse_drag(doc, editor_area, mouse.column, mouse.row);
                        }
                    }
                    MouseEventKind::Up(MouseButton::Left) => {
                        view.mouse_up();
                    }
                    MouseEventKind::ScrollUp if in_editor && ui.menu.open.is_none() => {
                        view.scroll_by(doc, -3);
                    }
                    MouseEventKind::ScrollDown if in_editor && ui.menu.open.is_none() => {
                        view.scroll_by(doc, 3);
                    }
                    _ => {}
                }
            }
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
                    continue;
                }

                if ui.dialog.is_some() {
                    if handle_dialog(terminal, key, doc, view, ui)? {
                        return Ok(());
                    }
                    continue;
                }

                if ui.menu.open.is_some() {
                    if handle_menu(terminal, key, doc, view, ui)? {
                        return Ok(());
                    }
                    continue;
                }

                // Tab toggles Editor ↔ Watch when the watch pane is open
                if matches!(key.code, KeyCode::Tab) && ui.watch_visible() {
                    ui.focus = match ui.focus {
                        Focus::Editor => Focus::Watch,
                        Focus::Watch => Focus::Editor,
                    };
                    continue;
                }

                // Watch focus: navigate errors / jump
                if ui.focus == Focus::Watch && ui.watch_visible() {
                    match key.code {
                        KeyCode::Up | KeyCode::Char('k') => {
                            ui.move_watch(-1);
                            jump_to_selected(doc, view, ui);
                            continue;
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            ui.move_watch(1);
                            jump_to_selected(doc, view, ui);
                            continue;
                        }
                        KeyCode::Enter => {
                            jump_to_selected(doc, view, ui);
                            ui.focus = Focus::Editor;
                            continue;
                        }
                        KeyCode::Esc => {
                            ui.focus = Focus::Editor;
                            continue;
                        }
                        _ => {}
                    }
                }

                match key.code {
                    KeyCode::F(3) if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        if ui.find.has_query() {
                            if find::find_prev(doc, view, &mut ui.find) {
                                ui.message = find_status(&ui.find);
                            } else {
                                ui.message = " No match.".into();
                            }
                        } else {
                            open_find(ui);
                        }
                        continue;
                    }
                    KeyCode::F(3) => {
                        if ui.find.has_query() {
                            if find::find_next(doc, view, &mut ui.find) {
                                ui.message = find_status(&ui.find);
                            } else {
                                ui.message = " No match.".into();
                            }
                        } else {
                            open_find(ui);
                        }
                        continue;
                    }
                    KeyCode::F(10) => {
                        ui.menu.activate(MenuId::File);
                        continue;
                    }
                    KeyCode::F(1) => {
                        ui.dialog = Some(Dialog::Help);
                        continue;
                    }
                    KeyCode::F(9) if key.modifiers.contains(KeyModifiers::ALT) => {
                        do_compile(doc, view, ui);
                        continue;
                    }
                    KeyCode::F(9) => {
                        try_run(terminal, doc, view, ui)?;
                        continue;
                    }
                    _ => {}
                }

                let ev = match map_key(key) {
                    Some(e) => e,
                    None => continue,
                };

                if is_ctrl(&ev, 'q') {
                    if doc.is_dirty() {
                        ui.dialog = Some(Dialog::ConfirmQuit);
                    } else {
                        return Ok(());
                    }
                    continue;
                }
                if is_ctrl(&ev, 's') {
                    do_save(doc, ui, false);
                    continue;
                }
                if is_ctrl(&ev, 'o') {
                    ui.dialog = Some(Dialog::Open {
                        input: String::new(),
                    });
                    continue;
                }
                if is_ctrl(&ev, 'p') {
                    open_project_dialog(ui);
                    continue;
                }
                if is_ctrl(&ev, 'u') {
                    open_units_dialog(doc, ui);
                    continue;
                }
                if is_ctrl(&ev, 'n') {
                    if doc.is_dirty() {
                        ui.message = " Save the file first (Ctrl+S), then New.".into();
                    } else {
                        *doc = default_untitled();
                        *view = EditorView::new();
                        ui.clear_diagnostics();
                        ui.find.clear_matches();
                        ui.clear_project();
                        ui.message = " New file.".into();
                    }
                    continue;
                }
                if is_ctrl(&ev, 'f') {
                    open_find(ui);
                    continue;
                }
                if is_ctrl(&ev, 'h') {
                    open_replace(ui);
                    continue;
                }
                if is_ctrl(&ev, 'r') {
                    try_run(terminal, doc, view, ui)?;
                    continue;
                }

                let copying = (ev.mods.ctrl
                    && matches!(ev.key, Key::Char('c') | Key::Char('C')))
                    || matches!(ev.key, Key::Char('\u{3}'));
                view.handle_key(doc, &ev);
                if copying {
                    ui.message = " Copied.".into();
                }
            }
            _ => {}
        }
    }
}

fn handle_menu(
    terminal: &mut TideTerminal,
    key: crossterm::event::KeyEvent,
    doc: &mut Document,
    view: &mut EditorView,
    ui: &mut UiState,
) -> io::Result<bool> {
    match key.code {
        KeyCode::Esc => {
            ui.menu.close();
        }
        KeyCode::Left => ui.menu.next_menu(-1),
        KeyCode::Right => ui.menu.next_menu(1),
        KeyCode::Up => ui.menu.move_sel(-1),
        KeyCode::Down => ui.menu.move_sel(1),
        KeyCode::Enter => {
            if let Some(cmd) = ui.menu.current_cmd() {
                ui.menu.close();
                return dispatch(terminal, cmd, doc, view, ui);
            }
        }
        KeyCode::Char(c) => {
            let c = c.to_ascii_lowercase();
            let id = match c {
                'f' => Some(MenuId::File),
                'e' => Some(MenuId::Edit),
                'r' => Some(MenuId::Run),
                'h' => Some(MenuId::Help),
                _ => None,
            };
            if let Some(id) = id {
                if ui.menu.open != Some(id) {
                    ui.menu.activate(id);
                }
            }
        }
        _ => {}
    }
    Ok(false)
}

fn handle_dialog(
    terminal: &mut TideTerminal,
    key: crossterm::event::KeyEvent,
    doc: &mut Document,
    view: &mut EditorView,
    ui: &mut UiState,
) -> io::Result<bool> {
    let Some(dialog) = ui.dialog.clone() else {
        return Ok(false);
    };

    match dialog {
        Dialog::ConfirmQuit => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => return Ok(true),
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                ui.dialog = None;
            }
            _ => {}
        },
        Dialog::Help | Dialog::About => {
            if matches!(key.code, KeyCode::Esc | KeyCode::Enter) {
                ui.dialog = None;
            }
        }
        Dialog::Find {
            mut input,
            mut case_sensitive,
        } => match key.code {
            KeyCode::Esc => ui.dialog = None,
            KeyCode::Enter => {
                ui.find.query = input.trim().to_string();
                ui.find.case_sensitive = case_sensitive;
                ui.dialog = None;
                if ui.find.query.is_empty() {
                    ui.find.clear_matches();
                    ui.message = " Find cancelled.".into();
                } else if find::find_next(doc, view, &mut ui.find) {
                    ui.message = find_status(&ui.find);
                } else {
                    ui.message = format!(" '{}' not found.", ui.find.query);
                }
            }
            KeyCode::Backspace => {
                input.pop();
                ui.dialog = Some(Dialog::Find {
                    input,
                    case_sensitive,
                });
            }
            KeyCode::Char('c') | KeyCode::Char('C')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                case_sensitive = !case_sensitive;
                ui.dialog = Some(Dialog::Find {
                    input,
                    case_sensitive,
                });
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                input.push(c);
                ui.dialog = Some(Dialog::Find {
                    input,
                    case_sensitive,
                });
            }
            _ => {}
        },
        Dialog::Replace {
            mut find,
            mut replace,
            mut field,
            mut case_sensitive,
        } => match key.code {
            KeyCode::Esc => ui.dialog = None,
            KeyCode::Tab => {
                field = 1 - field;
                ui.dialog = Some(Dialog::Replace {
                    find,
                    replace,
                    field,
                    case_sensitive,
                });
            }
            KeyCode::Enter => {
                // Keep the dialog open — closing it made the next Enter type a newline.
                ui.find.query = find.trim().to_string();
                ui.find.replace = replace.clone();
                ui.find.case_sensitive = case_sensitive;
                apply_replace_one(doc, view, ui);
                ui.dialog = Some(Dialog::Replace {
                    find,
                    replace,
                    field,
                    case_sensitive,
                });
            }
            KeyCode::Char('a') | KeyCode::Char('A')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                ui.find.query = find.trim().to_string();
                ui.find.replace = replace.clone();
                ui.find.case_sensitive = case_sensitive;
                let n = find::replace_all(doc, view, &mut ui.find);
                ui.message = format!(" Replaced {n} occurrence(s).");
                ui.dialog = Some(Dialog::Replace {
                    find,
                    replace,
                    field,
                    case_sensitive,
                });
            }
            KeyCode::Char('c') | KeyCode::Char('C')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                case_sensitive = !case_sensitive;
                ui.dialog = Some(Dialog::Replace {
                    find,
                    replace,
                    field,
                    case_sensitive,
                });
            }
            KeyCode::Backspace => {
                if field == 0 {
                    find.pop();
                } else {
                    replace.pop();
                }
                ui.dialog = Some(Dialog::Replace {
                    find,
                    replace,
                    field,
                    case_sensitive,
                });
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if field == 0 {
                    find.push(c);
                } else {
                    replace.push(c);
                }
                ui.dialog = Some(Dialog::Replace {
                    find,
                    replace,
                    field,
                    case_sensitive,
                });
            }
            _ => {}
        },
        Dialog::Open { mut input } => match key.code {
            KeyCode::Esc => {
                ui.path_tab = None;
                ui.dialog = None;
            }
            KeyCode::Enter => {
                ui.path_tab = None;
                let path = PathBuf::from(input.trim());
                if let Some(dir) = path_enter_dir(&input) {
                    // Browse into the folder; keep the dialog open.
                    ui.message = format!(" In {}  (Tab lists, Enter opens a file)", dir);
                    ui.dialog = Some(Dialog::Open { input: dir });
                } else {
                    ui.dialog = None;
                    match open_path(&path) {
                        Ok(d) => {
                            attach_project_for_doc(&d, ui);
                            *doc = d;
                            *view = EditorView::new();
                            ui.clear_diagnostics();
                            ui.find.clear_matches();
                            ui.message = format!(" Opened {}.", display_name(doc));
                        }
                        Err(e) => ui.message = e,
                    }
                }
            }
            KeyCode::Tab | KeyCode::BackTab => {
                let reverse = matches!(key.code, KeyCode::BackTab)
                    || key.modifiers.contains(KeyModifiers::SHIFT);
                let (next, msg) = path_tab_complete(&input, &mut ui.path_tab, reverse, false);
                input = next;
                if !msg.is_empty() {
                    ui.message = msg;
                }
                ui.dialog = Some(Dialog::Open { input });
            }
            KeyCode::Backspace => {
                ui.path_tab = None;
                input.pop();
                ui.dialog = Some(Dialog::Open { input });
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                ui.path_tab = None;
                input.push(c);
                ui.dialog = Some(Dialog::Open { input });
            }
            _ => {}
        },
        Dialog::OpenProject { mut input } => match key.code {
            KeyCode::Esc => {
                ui.path_tab = None;
                ui.dialog = None;
            }
            KeyCode::Enter => {
                ui.path_tab = None;
                let path = PathBuf::from(input.trim());
                // Browse into folders that aren't (yet) a project; open when they are.
                if files::is_project_dir(&path) {
                    ui.dialog = None;
                    match open_project_into(&path, ui) {
                        Ok(d) => {
                            *doc = d;
                            *view = EditorView::new();
                            ui.clear_diagnostics();
                            ui.find.clear_matches();
                            ui.message = format!(
                                " Project {} — {} unit(s). Ctrl+U to switch.",
                                ui.project_dir
                                    .as_ref()
                                    .map(|p| project_title(p))
                                    .unwrap_or_default(),
                                ui.units.len()
                            );
                        }
                        Err(e) => ui.message = e,
                    }
                } else if let Some(dir) = path_enter_dir(&input) {
                    ui.message = format!(" In {}  (Enter opens a project folder)", dir);
                    ui.dialog = Some(Dialog::OpenProject { input: dir });
                } else {
                    ui.dialog = None;
                    ui.message = format!(" Not a project folder: {}", path.display());
                }
            }
            KeyCode::Tab | KeyCode::BackTab => {
                let reverse = matches!(key.code, KeyCode::BackTab)
                    || key.modifiers.contains(KeyModifiers::SHIFT);
                let (next, msg) = path_tab_complete(&input, &mut ui.path_tab, reverse, true);
                input = next;
                if !msg.is_empty() {
                    ui.message = msg;
                }
                ui.dialog = Some(Dialog::OpenProject { input });
            }
            KeyCode::Backspace => {
                ui.path_tab = None;
                input.pop();
                ui.dialog = Some(Dialog::OpenProject { input });
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                ui.path_tab = None;
                input.push(c);
                ui.dialog = Some(Dialog::OpenProject { input });
            }
            _ => {}
        },
        Dialog::Units { mut selected } => match key.code {
            KeyCode::Esc => ui.dialog = None,
            KeyCode::Up => {
                if !ui.units.is_empty() {
                    selected = if selected == 0 {
                        ui.units.len() - 1
                    } else {
                        selected - 1
                    };
                    ui.dialog = Some(Dialog::Units { selected });
                }
            }
            KeyCode::Down => {
                if !ui.units.is_empty() {
                    selected = (selected + 1) % ui.units.len();
                    ui.dialog = Some(Dialog::Units { selected });
                }
            }
            KeyCode::Enter => {
                ui.dialog = None;
                if let Some(path) = ui.units.get(selected).cloned() {
                    switch_unit(doc, view, ui, &path);
                }
            }
            _ => {}
        },
        Dialog::SaveAs { mut input } => match key.code {
            KeyCode::Esc => {
                ui.path_tab = None;
                ui.dialog = None;
            }
            KeyCode::Enter => {
                ui.path_tab = None;
                if let Some(dir) = path_enter_dir(&input) {
                    ui.message = format!(" In {}  (type a name, Enter saves)", dir);
                    ui.dialog = Some(Dialog::SaveAs { input: dir });
                } else {
                    ui.dialog = None;
                    let path = resolve_save_path(&input);
                    match save_document(doc, Some(&path)) {
                        Ok(()) => {
                            attach_project_for_doc(doc, ui);
                            ui.message = format!(" Saved {}.", display_name(doc));
                        }
                        Err(e) => ui.message = e,
                    }
                }
            }
            KeyCode::Tab | KeyCode::BackTab => {
                let reverse = matches!(key.code, KeyCode::BackTab)
                    || key.modifiers.contains(KeyModifiers::SHIFT);
                let (next, msg) = path_tab_complete(&input, &mut ui.path_tab, reverse, false);
                input = next;
                if !msg.is_empty() {
                    ui.message = msg;
                }
                ui.dialog = Some(Dialog::SaveAs { input });
            }
            KeyCode::Backspace => {
                ui.path_tab = None;
                input.pop();
                ui.dialog = Some(Dialog::SaveAs { input });
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                ui.path_tab = None;
                input.push(c);
                ui.dialog = Some(Dialog::SaveAs { input });
            }
            _ => {}
        },
    }

    let _ = terminal;
    Ok(false)
}

fn dispatch(
    terminal: &mut TideTerminal,
    cmd: MenuCmd,
    doc: &mut Document,
    view: &mut EditorView,
    ui: &mut UiState,
) -> io::Result<bool> {
    match cmd {
        MenuCmd::File(FileCmd::New) => {
            if doc.is_dirty() {
                ui.message = " Save the file first, then New.".into();
            } else {
                *doc = default_untitled();
                *view = EditorView::new();
                ui.clear_diagnostics();
                ui.find.clear_matches();
                ui.clear_project();
                ui.message = " New file.".into();
            }
        }
        MenuCmd::File(FileCmd::Open) => {
            ui.dialog = Some(Dialog::Open {
                input: String::new(),
            });
        }
        MenuCmd::File(FileCmd::OpenProject) => open_project_dialog(ui),
        MenuCmd::File(FileCmd::Units) => open_units_dialog(doc, ui),
        MenuCmd::File(FileCmd::Save) => do_save(doc, ui, false),
        MenuCmd::File(FileCmd::SaveAs) => {
            let initial = doc
                .path()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            ui.dialog = Some(Dialog::SaveAs { input: initial });
        }
        MenuCmd::File(FileCmd::Quit) => {
            if doc.is_dirty() {
                ui.dialog = Some(Dialog::ConfirmQuit);
            } else {
                return Ok(true);
            }
        }
        MenuCmd::Edit(EditCmd::Undo) => {
            view.handle_key(doc, &KeyEvent::new(Key::Char('z'), KeyMods::ctrl()));
        }
        MenuCmd::Edit(EditCmd::Redo) => {
            view.handle_key(doc, &KeyEvent::new(Key::Char('y'), KeyMods::ctrl()));
        }
        MenuCmd::Edit(EditCmd::Cut) => {
            view.handle_key(doc, &KeyEvent::new(Key::Char('x'), KeyMods::ctrl()));
        }
        MenuCmd::Edit(EditCmd::Copy) => {
            view.handle_key(doc, &KeyEvent::new(Key::Char('c'), KeyMods::ctrl()));
            ui.message = " Copied.".into();
        }
        MenuCmd::Edit(EditCmd::Paste) => {
            view.handle_key(doc, &KeyEvent::new(Key::Char('v'), KeyMods::ctrl()));
        }
        MenuCmd::Edit(EditCmd::Find) => open_find(ui),
        MenuCmd::Edit(EditCmd::Replace) => open_replace(ui),
        MenuCmd::Run(RunCmd::Compile) => {
            do_compile(doc, view, ui);
        }
        MenuCmd::Run(RunCmd::Run) => {
            try_run(terminal, doc, view, ui)?;
        }
        MenuCmd::Help(HelpCmd::Keys) => ui.dialog = Some(Dialog::Help),
        MenuCmd::Help(HelpCmd::About) => ui.dialog = Some(Dialog::About),
    }
    Ok(false)
}

fn open_project_dialog(ui: &mut UiState) {
    let initial = ui
        .project_dir
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    ui.dialog = Some(Dialog::OpenProject { input: initial });
}

fn open_units_dialog(doc: &Document, ui: &mut UiState) {
    if !ui.has_project() {
        // Try to detect from the current file
        if let Some(path) = doc.path() {
            attach_project_for_path(path, ui);
        }
    }
    if !ui.has_project() {
        ui.message = " No project — open a project folder (Ctrl+P) or a unit in one.".into();
        return;
    }
    let selected = ui.current_unit_index(doc).unwrap_or(0);
    ui.dialog = Some(Dialog::Units { selected });
}

fn open_project_into(dir: &PathBuf, ui: &mut UiState) -> Result<Document, String> {
    let dir = if dir.as_os_str().is_empty() {
        return Err("Enter a project folder path.".into());
    } else {
        dir
    };
    if !dir.is_dir() {
        return Err(format!("Not a folder: {}", dir.display()));
    }
    let units = list_units(dir)?;
    if units.is_empty() {
        return Err(format!("No .vbr files in {}", dir.display()));
    }
    let entry = project_entry(&units)
        .ok_or_else(|| "No entry unit.".to_string())?
        .to_path_buf();
    let doc = open_path(&entry)?;
    ui.set_project(dir.clone(), units);
    Ok(doc)
}

fn attach_project_for_doc(doc: &Document, ui: &mut UiState) {
    if let Some(path) = doc.path() {
        attach_project_for_path(path, ui);
    } else {
        ui.clear_project();
    }
}

fn attach_project_for_path(path: &std::path::Path, ui: &mut UiState) {
    if let Some(dir) = detect_project(path) {
        match list_units(&dir) {
            Ok(units) if !units.is_empty() => ui.set_project(dir, units),
            _ => ui.clear_project(),
        }
    } else {
        ui.clear_project();
    }
}

fn switch_unit(
    doc: &mut Document,
    view: &mut EditorView,
    ui: &mut UiState,
    path: &std::path::Path,
) {
    if doc.path() == Some(path) {
        ui.message = format!(" Already editing {}.", unit_label(path));
        return;
    }
    if doc.is_dirty() {
        ui.message = " Save the unit first (Ctrl+S), then switch (Ctrl+U).".into();
        return;
    }
    match open_path(path) {
        Ok(d) => {
            *doc = d;
            *view = EditorView::new();
            ui.clear_diagnostics();
            ui.find.clear_matches();
            ui.message = format!(" Unit {}.", display_name(doc));
        }
        Err(e) => ui.message = e,
    }
}

fn open_find(ui: &mut UiState) {
    ui.dialog = Some(Dialog::Find {
        input: ui.find.query.clone(),
        case_sensitive: ui.find.case_sensitive,
    });
}

fn open_replace(ui: &mut UiState) {
    ui.dialog = Some(Dialog::Replace {
        find: ui.find.query.clone(),
        replace: ui.find.replace.clone(),
        field: 0,
        case_sensitive: ui.find.case_sensitive,
    });
}

fn find_status(find: &find::FindState) -> String {
    if find.matches.is_empty() || find.current == usize::MAX {
        " No match.".into()
    } else {
        format!(
            " Match {} of {}.",
            find.current + 1,
            find.matches.len()
        )
    }
}

fn apply_replace_one(doc: &mut Document, view: &mut EditorView, ui: &mut UiState) {
    use find::ReplaceResult::*;
    match find::replace_one(doc, view, &mut ui.find) {
        EmptyQuery => ui.message = " Nothing to find.".into(),
        NotFound => ui.message = format!(" '{}' not found.", ui.find.query),
        Found => ui.message = format!(" Found. Enter again to replace. {}", find_status(&ui.find)),
        ReplacedAndFound => {
            ui.message = format!(" Replaced. {}", find_status(&ui.find));
        }
        ReplacedLast => ui.message = " Replaced last match.".into(),
    }
}

fn do_save(doc: &mut Document, ui: &mut UiState, force_as: bool) {
    if force_as || doc.path().is_none() {
        let initial = doc
            .path()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        ui.dialog = Some(Dialog::SaveAs { input: initial });
        return;
    }
    match save_document(doc, None) {
        Ok(()) => {
            attach_project_for_doc(doc, ui);
            ui.message = format!(" Saved {}.", display_name(doc));
        }
        Err(e) => ui.message = e,
    }
}

fn do_compile(doc: &mut Document, view: &mut EditorView, ui: &mut UiState) {
    let outcome = compile::compile_buffer(&doc.text());
    ui.set_diagnostics(outcome.diagnostics);
    if outcome.has_errors {
        let n = ui
            .diagnostics
            .iter()
            .filter(|d| d.level == compile::DiagLevel::Error)
            .count();
        ui.message = format!(" Compile: {n} error(s). Tab/↑↓ Watch, Enter jumps.");
        jump_to_selected(doc, view, ui);
    } else if ui.diagnostics.is_empty() {
        ui.message = " Compile: OK — no diagnostics.".into();
    } else {
        let n = ui.diagnostics.len();
        ui.message = format!(" Compile: OK, {n} note(s)/warning(s).");
        jump_to_selected(doc, view, ui);
    }
}

fn jump_to_selected(doc: &mut Document, view: &mut EditorView, ui: &UiState) {
    let Some(diag) = ui.selected_diag() else {
        return;
    };
    let Some((line, col)) = compile::jump_target(diag) else {
        return;
    };
    view.goto(doc, line, col);
}

fn try_run(
    terminal: &mut TideTerminal,
    doc: &mut Document,
    view: &mut EditorView,
    ui: &mut UiState,
) -> io::Result<()> {
    // TP loop: compile first; don't Run if the front-end already failed.
    let outcome = compile::compile_buffer(&doc.text());
    ui.set_diagnostics(outcome.diagnostics.clone());
    if outcome.has_errors {
        let n = outcome
            .diagnostics
            .iter()
            .filter(|d| d.level == compile::DiagLevel::Error)
            .count();
        ui.message = format!(" Run blocked: {n} error(s). Fix, then F9 again.");
        jump_to_selected(doc, view, ui);
        return Ok(());
    }

    if doc.path().is_none() || doc.is_dirty() {
        if doc.path().is_none() {
            ui.dialog = Some(Dialog::SaveAs {
                input: String::new(),
            });
            ui.message = " Save the file before Run.".into();
            return Ok(());
        }
        if let Err(e) = save_document(doc, None) {
            ui.message = e;
            return Ok(());
        }
    }
    let path = doc.path().map(PathBuf::from).unwrap();
    execute!(stdout(), DisableMouseCapture, DisableBracketedPaste)?;
    let msg = run::run_vbr(terminal, &path)?;
    execute!(stdout(), EnableMouseCapture, EnableBracketedPaste)?;
    ui.clear_diagnostics();
    ui.message = format!(" {msg}");
    Ok(())
}

fn map_key(key: crossterm::event::KeyEvent) -> Option<KeyEvent> {
    let mods = KeyMods {
        ctrl: key.modifiers.contains(KeyModifiers::CONTROL),
        alt: key.modifiers.contains(KeyModifiers::ALT),
        shift: key.modifiers.contains(KeyModifiers::SHIFT),
    };
    let k = match key.code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Enter => Key::Enter,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Delete => Key::Delete,
        KeyCode::Tab => Key::Tab,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Esc => Key::Esc,
        _ => return None,
    };
    Some(KeyEvent::new(k, mods))
}
