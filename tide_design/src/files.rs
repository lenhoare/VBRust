//! Path prompts — Tab completion copied from TIDE's Open / Save As.

use std::path::{Path, PathBuf};

/// Cycle state for Tab path completion.
#[derive(Debug, Clone, Default)]
pub struct PathTabState {
    candidates: Vec<String>,
    index: usize,
}

/// If `input` names an existing directory, return it with a trailing separator.
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

pub fn path_tab_complete(
    input: &str,
    state: &mut Option<PathTabState>,
    reverse: bool,
) -> (String, String) {
    if let Some(st) = state.as_mut() {
        if st.candidates.get(st.index).map(String::as_str) == Some(input) {
            let n = st.candidates.len();
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
            *state = None;
        }
    }

    let candidates = list_path_completions(input);
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

fn list_path_completions(input: &str) -> Vec<String> {
    let trimmed = input.trim_start();
    let lead_ws_len = input.len() - trimmed.len();
    let lead_ws = &input[..lead_ws_len];
    let (dir, partial, prefix) = split_path_prefix(trimmed);
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let partial_lower = partial.to_ascii_lowercase();
    let sep = preferred_sep(if prefix.is_empty() { trimmed } else { &prefix });
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
        children.push((name.into_owned(), is_dir));
    }
    children.sort_by(|a, b| a.0.to_ascii_lowercase().cmp(&b.0.to_ascii_lowercase()));
    let mut out: Vec<String> = Vec::new();
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

fn split_path_prefix(input: &str) -> (PathBuf, String, String) {
    if input.is_empty() {
        return (PathBuf::from("."), String::new(), String::new());
    }
    if input.ends_with('/') || input.ends_with('\\') {
        return (PathBuf::from(input), String::new(), input.to_string());
    }
    let path = Path::new(input);
    match path.file_name() {
        Some(name) if path.parent().is_some_and(|p| !p.as_os_str().is_empty()) => {
            let parent = path.parent().unwrap();
            let mut prefix = parent.to_string_lossy().into_owned();
            if !prefix.ends_with('/') && !prefix.ends_with('\\') {
                prefix.push(preferred_sep(input));
            }
            (
                parent.to_path_buf(),
                name.to_string_lossy().into_owned(),
                prefix,
            )
        }
        Some(name) => (
            PathBuf::from("."),
            name.to_string_lossy().into_owned(),
            String::new(),
        ),
        None => (PathBuf::from("."), String::new(), String::new()),
    }
}

/// Directory Open / Save-as-template start in.
///
/// Prefers the crate's bundled `templates/` (when running via `cargo run`),
/// then a `templates/` next to the binary, then `./templates` or
/// `./tide_design/templates` from the current directory.
pub fn templates_dir() -> PathBuf {
    let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates");
    if bundled.is_dir() {
        return bundled;
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let next_to_exe = dir.join("templates");
            if next_to_exe.is_dir() {
                return next_to_exe;
            }
        }
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for candidate in [cwd.join("templates"), cwd.join("tide_design").join("templates")] {
        if candidate.is_dir() {
            return candidate;
        }
    }
    bundled
}

/// `templates_dir()` with a trailing separator so Tab lists the files inside.
pub fn templates_dir_slash() -> String {
    with_trailing_sep(&templates_dir())
}

pub fn with_trailing_sep(path: &Path) -> String {
    let mut s = path.to_string_lossy().into_owned();
    if s.is_empty() {
        s.push('.');
    }
    if !s.ends_with('/') && !s.ends_with('\\') {
        s.push('/');
    }
    s
}

/// Suggested path when saving a new template: `templates/<screen>.vbt`.
pub fn default_vbt_path(screen_name: &str) -> PathBuf {
    templates_dir().join(format!("{}.vbt", snake_name(screen_name)))
}

fn snake_name(name: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for c in name.chars() {
        if c.is_uppercase() && prev_lower {
            out.push('_');
        }
        out.extend(c.to_lowercase());
        prev_lower = c.is_lowercase();
    }
    if out.is_empty() {
        "screen".into()
    } else {
        out
    }
}

pub fn is_vbt(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("vbt"))
}

pub fn is_vbr(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("vbr"))
}

pub fn with_ext(mut path: PathBuf, ext: &str) -> PathBuf {
    if path.extension().is_none() {
        path.set_extension(ext);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_name_splits_pascal() {
        assert_eq!(snake_name("Notes"), "notes");
        assert_eq!(snake_name("MasterDetail"), "master_detail");
        assert_eq!(snake_name("FileBrowser"), "file_browser");
        assert_eq!(snake_name("Crud"), "crud");
    }

    #[test]
    fn bundled_templates_dir_exists() {
        let dir = templates_dir();
        assert!(dir.is_dir(), "{}", dir.display());
        let n = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| is_vbt(&e.path()))
            .count();
        assert!(n >= 20, "expected 20 templates in {}, found {n}", dir.display());
        let slash = templates_dir_slash();
        assert!(slash.ends_with('/') || slash.ends_with('\\'));
    }
}
