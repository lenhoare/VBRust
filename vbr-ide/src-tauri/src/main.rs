// The VBR IDE desktop shell.
//
// Prevents an extra console window from opening alongside the app on Windows
// release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use vbr_ide_core::{
    complete, definition, hover, run, transpile, CompletionItem, Range, RunOutput, TranspileResult,
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
    tauri::async_runtime::spawn_blocking(|| {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("VBR", &["vbr"])
            .pick_file()
        else {
            return Ok(None);
        };
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        Ok(Some(OpenedFile {
            path: path.to_string_lossy().into_owned(),
            content,
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Save `content`. With a known `path` it writes straight there; otherwise it
/// shows a Save-As dialog. Returns the path written, or `None` if cancelled.
#[tauri::command]
async fn save_file(path: Option<String>, content: String) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let target: Option<PathBuf> = match path {
            Some(p) => Some(PathBuf::from(p)),
            None => rfd::FileDialog::new()
                .add_filter("VBR", &["vbr"])
                .set_file_name("untitled.vbr")
                .save_file(),
        };
        let Some(target) = target else {
            return Ok(None);
        };
        std::fs::write(&target, content).map_err(|e| e.to_string())?;
        Ok(Some(target.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|e| e.to_string())?
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

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            transpile_source,
            run_source,
            open_file,
            save_file,
            complete_at,
            hover_at,
            definition_at
        ])
        .run(tauri::generate_context!())
        .expect("error while running the VBR IDE");
}
