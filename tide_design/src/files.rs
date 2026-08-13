//! Filename prompts — browse a folder, Tab-cycle names (not full paths).

use std::path::{Path, PathBuf};

/// Cycle state for Tab filename completion.
#[derive(Debug, Clone, Default)]
pub struct PathTabState {
    candidates: Vec<String>,
    index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameFilter {
    /// Open template: `.vbt` files and folders.
    Templates,
    /// Save: any name.
    All,
}

/// Last path component, for the dialog's "In templates" line.
pub fn folder_label(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| dir.display().to_string())
}

/// If `name` is `..` or a subdirectory of `dir`, the folder to browse next.
pub fn try_enter_dir(dir: &Path, name: &str) -> Option<PathBuf> {
    let name = name.trim().trim_end_matches(['/', '\\']);
    if name.is_empty() {
        return None;
    }
    let next = if name == ".." {
        dir.parent()?.to_path_buf()
    } else {
        dir.join(name)
    };
    next.is_dir().then_some(next)
}

pub fn filename_tab_complete(
    dir: &Path,
    input: &str,
    state: &mut Option<PathTabState>,
    reverse: bool,
    filter: NameFilter,
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
                    format!(" {}/{}  {}", st.index + 1, n, st.candidates[st.index])
                } else {
                    String::new()
                };
                return (st.candidates[st.index].clone(), msg);
            }
            *state = None;
        }
    }

    let candidates = list_filenames(dir, input, filter);
    if candidates.is_empty() {
        *state = None;
        return (input.to_string(), " No matches".into());
    }
    let result = candidates[0].clone();
    let msg = if candidates.len() > 1 {
        format!(" 1/{}  {result}  — Tab to cycle", candidates.len())
    } else {
        String::new()
    };
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

pub fn list_filenames(dir: &Path, partial: &str, filter: NameFilter) -> Vec<String> {
    let partial_lower = partial
        .trim()
        .trim_end_matches(['/', '\\'])
        .to_ascii_lowercase();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut children: Vec<(String, bool)> = Vec::new();
    for ent in rd.flatten() {
        let name = ent.file_name().to_string_lossy().into_owned();
        if name == "." || name == ".." {
            continue;
        }
        let is_dir = ent.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if !is_dir && filter == NameFilter::Templates && !is_vbt(Path::new(&name)) {
            continue;
        }
        if !partial_lower.is_empty() && !name.to_ascii_lowercase().starts_with(&partial_lower) {
            continue;
        }
        children.push((name, is_dir));
    }
    children.sort_by(|a, b| a.0.to_ascii_lowercase().cmp(&b.0.to_ascii_lowercase()));
    let mut out: Vec<String> = children
        .into_iter()
        .map(|(name, is_dir)| {
            if is_dir {
                format!("{name}/")
            } else {
                name
            }
        })
        .collect();
    let want_parent = partial_lower.is_empty() || "..".starts_with(partial_lower.as_str());
    if want_parent && dir.parent().is_some() {
        out.push("..".into());
    }
    out
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

pub fn default_vbt_filename(screen_name: &str) -> String {
    format!("{}.vbt", snake_name(screen_name))
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
        assert_eq!(folder_label(&dir), "templates");
    }

    #[test]
    fn open_lists_basenames_not_paths() {
        let dir = templates_dir();
        let names = list_filenames(&dir, "", NameFilter::Templates);
        assert!(names.iter().any(|n| n == "notes.vbt"), "{names:?}");
        assert!(names.iter().any(|n| n == "calendar.vbt"), "{names:?}");
        assert!(
            names.iter().all(|n| !n.starts_with('/') && !n.contains("templates")),
            "expected filenames only, got {names:?}"
        );
        let mut state = None;
        let (first, msg) = filename_tab_complete(&dir, "", &mut state, false, NameFilter::Templates);
        assert_eq!(first, "calendar.vbt");
        assert!(msg.contains("Tab to cycle"), "{msg}");
    }
}
