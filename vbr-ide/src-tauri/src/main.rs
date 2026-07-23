// The VBR IDE desktop shell.
//
// Prevents an extra console window from opening alongside the app on Windows
// release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use vbr_ide_core::{transpile, TranspileResult};

/// The one command slice 1 needs: turn the editor's current text into Rust +
/// diagnostics. Everything heavy lives in `vbr-ide-core`; this is just the
/// bridge across the webview boundary.
#[tauri::command]
fn transpile_source(source: String) -> TranspileResult {
    transpile(&source)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![transpile_source])
        .run(tauri::generate_context!())
        .expect("error while running the VBR IDE");
}
