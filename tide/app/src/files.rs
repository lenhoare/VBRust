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
        "' TIDE — Turbo Pascal vibes for Bust\n\
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
        .unwrap_or_else(|| "NONAME.Bust".into())
}

pub fn resolve_save_path(input: &str) -> PathBuf {
    let p = input.trim();
    if p.is_empty() {
        return PathBuf::from("NONAME.Bust");
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

/// A folder is a Bust project if it has `main.vbr` or more than one `.vbr` file.
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

/// Cycle state for Tab path completion in Open / Save dialogs.
#[derive(Debug, Clone, Default)]
pub struct PathTabState {
    /// Full replacement strings for the text box.
    candidates: Vec<String>,
    index: usize,
}

/// If `input` names an existing directory, return it with a trailing separator
/// (components like `..` normalized). Used when Enter browses into a folder.
pub fn path_enter_dir(input: &str) -> Option<String> {
    let trimmed = input.trim();
    let path = if trimmed.is_empty() {
        PathBuf::from(".")
    } else {
        PathBuf::from(trimmed)
    };
    if !path.is_dir() {
        return None;
    }
    let normalized = normalize_path_components(&path);
    let mut s = normalized.to_string_lossy().into_owned();
    if s.is_empty() {
        s.push('.');
    }
    let sep = preferred_sep(trimmed);
    if !s.ends_with('/') && !s.ends_with('\\') {
        s.push(sep);
    }
    Some(s)
}

fn preferred_sep(input: &str) -> char {
    if input.contains('\\') && !input.contains('/') {
        '\\'
    } else {
        '/'
    }
}

fn normalize_path_components(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::Prefix(_) | Component::RootDir => out.push(c.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            Component::Normal(s) => out.push(s),
        }
    }
    out
}

/// Tab-complete a filesystem path. Tab again cycles; Shift+Tab goes backward.
///
/// `dirs_only` filters to directories (for Open Project). Directory matches get a
/// trailing `/` so the next Tab can list their children (after a unique match).
/// A `../` entry is included so you can climb out of a folder.
pub fn path_tab_complete(
    input: &str,
    state: &mut Option<PathTabState>,
    reverse: bool,
    dirs_only: bool,
) -> (String, String) {
    // Continue an active cycle when the box still shows that completion.
    if let Some(st) = state.as_mut() {
        if st.candidates.get(st.index).map(String::as_str) == Some(input) {
            let n = st.candidates.len();
            // Unique directory: next Tab descends into it instead of no-oping.
            let unique_dir = n == 1 && (input.ends_with('/') || input.ends_with('\\'));
            if !unique_dir && n > 0 {
                st.index = if reverse {
                    if st.index == 0 {
                        n - 1
                    } else {
                        st.index - 1
                    }
                } else {
                    (st.index + 1) % n
                };
                let msg = if n > 1 {
                    format!(" {}/{} matches", st.index + 1, n)
                } else {
                    String::new()
                };
                return (st.candidates[st.index].clone(), msg);
            }
            // Fall through to a fresh completion (list children of this dir).
            *state = None;
        }
    }

    let candidates = list_path_completions(input, dirs_only);
    if candidates.is_empty() {
        *state = None;
        return (input.to_string(), " No matches".into());
    }
    let msg = if candidates.len() > 1 {
        format!(" 1/{} matches — Tab to cycle", candidates.len())
    } else {
        String::new()
    };
    let result = candidates[0].clone();
    // Unique dir: drop cycle state so the next Tab lists children.
    *state = if candidates.len() == 1 && (result.ends_with('/') || result.ends_with('\\')) {
        None
    } else {
        Some(PathTabState {
            candidates,
            index: 0,
        })
    };
    (result, msg)
}

fn list_path_completions(input: &str, dirs_only: bool) -> Vec<String> {
    let raw = input;
    let trimmed = raw.trim_start();
    let lead_ws_len = raw.len() - trimmed.len();
    let lead_ws = &raw[..lead_ws_len];

    let (dir, partial, prefix) = split_path_prefix(trimmed);
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };

    let partial_lower = partial.to_ascii_lowercase();
    let sep = preferred_sep(if prefix.is_empty() { trimmed } else { &prefix });
    let mut out: Vec<String> = Vec::new();

    let mut children: Vec<(String, bool)> = Vec::new();
    for ent in rd.flatten() {
        let name = ent.file_name();
        let name = name.to_string_lossy();
        if name == "." || name == ".." {
            continue;
        }
        if !partial_lower.is_empty() && !name.to_ascii_lowercase().starts_with(&partial_lower) {
            continue;
        }
        let is_dir = ent.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if dirs_only && !is_dir {
            continue;
        }
        children.push((name.into_owned(), is_dir));
    }
    children.sort_by(|a, b| a.0.to_ascii_lowercase().cmp(&b.0.to_ascii_lowercase()));

    for (name, is_dir) in children {
        let mut s = String::new();
        s.push_str(lead_ws);
        s.push_str(&prefix);
        s.push_str(&name);
        if is_dir {
            s.push(sep);
        }
        out.push(s);
    }

    // `../` last when listing a folder or typing `.` / `..` — easy to find, doesn't
    // steal the first Tab from real names.
    let want_parent = partial_lower.is_empty()
        || "..".starts_with(partial_lower.as_str())
        || partial_lower == ".";
    if want_parent {
        if let Some(up) = parent_completion(lead_ws, &prefix, &dir, sep) {
            out.push(up);
        }
    }
    out
}

/// Build a `../` completion for the directory being listed.
fn parent_completion(lead_ws: &str, prefix: &str, dir: &Path, sep: char) -> Option<String> {
    if let Ok(canon) = std::fs::canonicalize(dir) {
        if canon.parent().is_none() {
            return None;
        }
    }
    let mut s = String::new();
    s.push_str(lead_ws);
    s.push_str(prefix);
    s.push_str("..");
    s.push(sep);
    Some(s)
}

/// Split typed path into `(directory to list, partial name, display prefix for that dir)`.
fn split_path_prefix(input: &str) -> (PathBuf, String, String) {
    if input.is_empty() {
        return (PathBuf::from("."), String::new(), String::new());
    }

    let ends_with_sep = input.ends_with('/') || input.ends_with('\\');
    if ends_with_sep {
        let dir = PathBuf::from(input);
        return (dir, String::new(), input.to_string());
    }

    let path = Path::new(input);
    match path.file_name() {
        Some(name) if path.parent().is_some_and(|p| !p.as_os_str().is_empty()) => {
            let parent = path.parent().unwrap();
            let mut prefix = parent.to_string_lossy().into_owned();
            if !prefix.ends_with('/') && !prefix.ends_with('\\') {
                let sep = if input.contains('\\') && !input.contains('/') {
                    '\\'
                } else {
                    '/'
                };
                prefix.push(sep);
            }
            (parent.to_path_buf(), name.to_string_lossy().into_owned(), prefix)
        }
        Some(name) => {
            // Bare name in cwd, or a single-component relative path.
            (PathBuf::from("."), name.to_string_lossy().into_owned(), String::new())
        }
        None => (PathBuf::from("."), String::new(), String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn tab_completes_and_cycles() {
        let dir = std::env::temp_dir().join(format!("tide_tab_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("alpha.vbr"), "").unwrap();
        fs::write(dir.join("alpine.vbr"), "").unwrap();
        fs::create_dir(dir.join("algae")).unwrap();

        let prefix = format!("{}/al", dir.display());
        let mut state = None;
        let (a, msg) = path_tab_complete(&prefix, &mut state, false, false);
        assert!(msg.contains("matches"), "{msg}");
        assert!(a.contains("algae") || a.contains("alpha") || a.contains("alpine"), "{a}");

        let (b, _) = path_tab_complete(&a, &mut state, false, false);
        assert_ne!(a, b);

        let (c, _) = path_tab_complete(&b, &mut state, false, false);
        let (d, _) = path_tab_complete(&c, &mut state, false, false);
        // three matches → third Tab wraps to first
        assert_eq!(d, a);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn enter_dir_keeps_trailing_sep_and_resolves_dotdot() {
        let dir = std::env::temp_dir().join(format!("tide_enter_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();

        let entered = path_enter_dir(&format!("{}/sub", dir.display())).unwrap();
        assert!(entered.ends_with('/'), "{entered}");
        assert!(entered.contains("sub"), "{entered}");

        let up = path_enter_dir(&format!("{}/sub/../", dir.display())).unwrap();
        assert!(!up.contains("sub"), "{up}");
        assert!(up.ends_with('/'), "{up}");

        assert!(path_enter_dir("/no/such/tide_path_xyz").is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tab_lists_dotdot_when_browsing_dir() {
        let dir = std::env::temp_dir().join(format!("tide_dotdot_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.vbr"), "").unwrap();

        let input = format!("{}/", dir.display());
        let mut state = None;
        let (first, msg) = path_tab_complete(&input, &mut state, false, false);
        assert!(msg.contains("matches") || first.ends_with("a.vbr") || first.ends_with("../"), "{first} {msg}");

        // Cycle until we see ../
        let mut cur = first;
        let mut saw_up = cur.ends_with("../");
        for _ in 0..8 {
            let (n, _) = path_tab_complete(&cur, &mut state, false, false);
            if n.ends_with("../") {
                saw_up = true;
                break;
            }
            if n == cur {
                break;
            }
            cur = n;
        }
        assert!(saw_up, "expected a ../ completion");
        let _ = fs::remove_dir_all(&dir);
    }
}
