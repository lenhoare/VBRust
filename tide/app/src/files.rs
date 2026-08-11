//! Open / save helpers.

use std::path::{Path, PathBuf};
use tide_editor::Document;

pub fn open_path(path: impl AsRef<Path>) -> Result<Document, String> {
    Document::open(path.as_ref()).map_err(|e| format!("Cannot open: {e}"))
}

pub fn save_document(doc: &mut Document, path: Option<&Path>) -> Result<(), String> {
    match path {
        Some(p) => doc.save_as(p).map_err(|e| format!("Cannot save: {e}")),
        None => {
            if doc.path().is_some() {
                doc.save().map_err(|e| format!("Cannot save: {e}"))
            } else {
                Err("No file name — use Save As".into())
            }
        }
    }
}

pub fn default_untitled() -> Document {
    Document::from_str(
        "' TIDE — Turbo Pascal vibes for VBR\n\
         ' F10 Menu  F1 Help  F9 Run  Ctrl+S Save\n\
         \n\
         Function Main()\n\
             Debug.Print \"Hello from TIDE\"\n\
         End Function\n",
    )
}

pub fn display_name(doc: &Document) -> String {
    doc.path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "NONAME.VBR".into())
}

pub fn resolve_save_path(input: &str) -> PathBuf {
    let p = input.trim();
    if p.is_empty() {
        return PathBuf::from("NONAME.VBR");
    }
    let mut path = PathBuf::from(p);
    if path.extension().is_none() {
        path.set_extension("vbr");
    }
    path
}
