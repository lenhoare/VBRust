//! Open / save / project (unit list) helpers.

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
         ' Ctrl+P Open project   Ctrl+U Units\n\
         \n\
         Function Main()\n\
             Debug.Print \"Hello from TIDE\"\n\
         End Function\n",
    )
}

pub fn display_name(doc: &Document) -> String {
    doc.path()
        .map(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.display().to_string())
        })
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

/// List `.vbr` units in a directory (sorted, `main.vbr` first).
pub fn list_units(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let rd = std::fs::read_dir(dir).map_err(|e| format!("Cannot read {}: {e}", dir.display()))?;
    let mut units = Vec::new();
    for ent in rd {
        let ent = ent.map_err(|e| format!("Cannot read dir entry: {e}"))?;
        let path = ent.path();
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("vbr"))
            && path.is_file()
        {
            units.push(path);
        }
    }
    units.sort_by(|a, b| {
        let a_main = a
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("main.vbr"));
        let b_main = b
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("main.vbr"));
        match (a_main, b_main) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a
                .file_name()
                .unwrap_or_default()
                .cmp(b.file_name().unwrap_or_default()),
        }
    });
    Ok(units)
}

/// A folder is a VBR project if it has `main.vbr` or more than one `.vbr` file.
pub fn is_project_dir(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    let Ok(units) = list_units(dir) else {
        return false;
    };
    units
        .iter()
        .any(|p| {
            p.file_name()
                .is_some_and(|n| n.eq_ignore_ascii_case("main.vbr"))
        })
        || units.len() > 1
}

/// If `path` is a file, return its parent when that parent looks like a project.
/// If `path` is a directory that looks like a project, return it.
pub fn detect_project(path: &Path) -> Option<PathBuf> {
    if path.is_dir() {
        return is_project_dir(path).then(|| path.to_path_buf());
    }
    let dir = path.parent()?;
    is_project_dir(dir).then(|| dir.to_path_buf())
}

/// Entry unit for a project folder: `main.vbr` if present, else first unit.
pub fn project_entry(units: &[PathBuf]) -> Option<&Path> {
    units
        .iter()
        .find(|p| {
            p.file_name()
                .is_some_and(|n| n.eq_ignore_ascii_case("main.vbr"))
        })
        .or_else(|| units.first())
        .map(|p| p.as_path())
}

pub fn unit_label(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

pub fn project_title(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir.display().to_string())
}
