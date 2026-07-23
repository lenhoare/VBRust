// The VBR IDE desktop shell.
//
// Prevents an extra console window from opening alongside the app on Windows
// release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use vbr_ide_core::{
    complete, create_form as core_create_form, definition, design_to_vbr, graduate, hover,
    read_file, read_project, run, run_project, test_project, transpile, CompletionItem, Node,
    Project, Range, RunOutput, TranspileResult,
};

/// A file the user opened: its path (so Save can write back to it) and text.
#[derive(serde::Serialize)]
struct OpenedFile {
    path: String,
    content: String,
}

/// Turn the editor's current text into Rust + diagnostics. Everything heavy
/// lives in `vbr-ide-core`; this is just the bridge across the webview boundary.
#[tauri::command]
fn transpile_source(source: String) -> TranspileResult {
    transpile(&source)
}

/// Compile and run the current buffer, returning its output. rustc can take a
/// moment, so this runs on a blocking thread to keep the UI responsive.
#[tauri::command]
async fn run_source(source: String) -> RunOutput {
    tauri::async_runtime::spawn_blocking(move || run(&source))
        .await
        .unwrap_or_else(|e| RunOutput {
            stage: "compile".to_string(),
            rust: String::new(),
            diagnostics: Vec::new(),
            stdout: String::new(),
            stderr: format!("The run task failed to complete: {e}"),
            success: false,
        })
}

/// Show an open dialog and read the chosen `.vbr` file. `Ok(None)` means the
/// user cancelled.
#[tauri::command]
async fn open_file() -> Result<Option<OpenedFile>, String> {
    let Some(handle) = rfd::AsyncFileDialog::new()
        .add_filter("VBR", &["vbr"])
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    let content = std::fs::read_to_string(handle.path()).map_err(|e| e.to_string())?;
    Ok(Some(OpenedFile {
        path: handle.path().to_string_lossy().into_owned(),
        content,
    }))
}

/// Save `content`. With a known `path` it writes straight there; otherwise it
/// shows a Save-As dialog. Returns the path written, or `None` if cancelled.
#[tauri::command]
async fn save_file(path: Option<String>, content: String) -> Result<Option<String>, String> {
    let target: PathBuf = match path {
        Some(p) => PathBuf::from(p),
        None => {
            let Some(handle) = rfd::AsyncFileDialog::new()
                .add_filter("VBR", &["vbr"])
                .set_file_name("untitled.vbr")
                .save_file()
                .await
            else {
                return Ok(None);
            };
            handle.path().to_path_buf()
        }
    };
    std::fs::write(&target, content).map_err(|e| e.to_string())?;
    Ok(Some(target.to_string_lossy().into_owned()))
}

/// Completions at a cursor position (line/column are 1-based, column in UTF-16
/// units as the webview reports them).
#[tauri::command]
fn complete_at(source: String, line: u32, col: u32) -> Vec<CompletionItem> {
    complete(&source, line, col)
}

/// Hover text at a cursor position, or `None`.
#[tauri::command]
fn hover_at(source: String, line: u32, col: u32) -> Option<String> {
    hover(&source, line, col)
}

/// The declaration range for the symbol under the cursor, or `None`.
#[tauri::command]
fn definition_at(source: String, line: u32, col: u32) -> Option<Range> {
    definition(&source, line, col)
}

/// Show a folder picker and read the chosen folder into a project tree.
/// `None` means the user cancelled.
#[tauri::command]
async fn open_folder() -> Option<Project> {
    let handle = rfd::AsyncFileDialog::new().pick_folder().await?;
    Some(read_project(handle.path()))
}

/// Read a file the user clicked in the tree.
#[tauri::command]
fn read_file_at(path: String) -> Result<String, String> {
    read_file(&path)
}

/// Delete a file (from the tree's right-click menu).
#[tauri::command]
fn delete_file(path: String) -> Result<(), String> {
    std::fs::remove_file(&path).map_err(|e| e.to_string())
}

/// Re-read a known folder into a project tree (e.g. after graduation changes
/// the files on disk).
#[tauri::command]
fn read_project_at(root: String) -> Project {
    read_project(Path::new(&root))
}

/// Graduate a module: `vbr graduate <path>`.
#[tauri::command]
async fn graduate_at(path: String) -> RunOutput {
    tauri::async_runtime::spawn_blocking(move || graduate(Path::new(&path)))
        .await
        .unwrap_or_else(|e| RunOutput {
            stage: "compile".to_string(),
            rust: String::new(),
            diagnostics: Vec::new(),
            stdout: String::new(),
            stderr: format!("The graduate task failed to complete: {e}"),
            success: false,
        })
}

/// Run a project's tests: `vbr test <root>`.
#[tauri::command]
async fn test_at(root: String) -> RunOutput {
    tauri::async_runtime::spawn_blocking(move || test_project(Path::new(&root)))
        .await
        .unwrap_or_else(|e| RunOutput {
            stage: "compile".to_string(),
            rust: String::new(),
            diagnostics: Vec::new(),
            stdout: String::new(),
            stderr: format!("The test task failed to complete: {e}"),
            success: false,
        })
}

/// Build and run a whole project folder via `vbr runproject`.
#[tauri::command]
async fn run_project_at(root: String) -> RunOutput {
    tauri::async_runtime::spawn_blocking(move || run_project(Path::new(&root)))
        .await
        .unwrap_or_else(|e| RunOutput {
            stage: "compile".to_string(),
            rust: String::new(),
            diagnostics: Vec::new(),
            stdout: String::new(),
            stderr: format!("The project run failed to complete: {e}"),
            success: false,
        })
}

/// Generate a complete VBR `Window`/`Screen` from a form-designer widget tree
/// (live preview — the real file uses its auto-numbered name). `target` is
/// "gui" or "tui".
#[tauri::command]
fn generate_design(tree: Node, target: String) -> String {
    let name = if target == "tui" { "Screen1" } else { "Form1" };
    design_to_vbr(&tree, name, &target)
}

/// A form file just written to disk.
#[derive(serde::Serialize)]
struct CreatedForm {
    path: String,
    name: String,
}

/// Write the designed form as a new auto-numbered `formN.vbr`/`screenN.vbr` in `dir`.
#[tauri::command]
fn create_form(dir: String, tree: Node, target: String) -> Result<CreatedForm, String> {
    core_create_form(Path::new(&dir), &tree, &target)
        .map(|(p, name)| CreatedForm {
            path: p.to_string_lossy().into_owned(),
            name,
        })
        .map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            transpile_source,
            run_source,
            open_file,
            save_file,
            complete_at,
            hover_at,
            definition_at,
            open_folder,
            read_file_at,
            delete_file,
            read_project_at,
            run_project_at,
            graduate_at,
            test_at,
            generate_design,
            create_form
        ])
        .run(tauri::generate_context!())
        .expect("error while running the VBR IDE");
}
