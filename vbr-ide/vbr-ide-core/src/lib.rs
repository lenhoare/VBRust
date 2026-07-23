//! The compiler-facing core of the VBR IDE.
//!
//! This crate deliberately knows nothing about Tauri, webviews, or the
//! frontend — it just turns VBR source into the two things the editor needs to
//! show: the generated Rust, and the diagnostics to draw over the source.
//!
//! Keeping it separate from the desktop shell means it builds and unit-tests on
//! any platform (no WebView2/WebKitGTK required), and the same `transpile` a
//! button-press triggers is the same one the tests exercise.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

mod design;
pub use design::{design_to_vbr, Node, NodeProps};

/// A Monaco-ready range: 1-based lines and columns, columns measured in UTF-16
/// code units (what Monaco and the LSP speak). Serialised with the exact field
/// names Monaco's `IMarkerData` expects, so the frontend can use it directly.
#[derive(Debug, Clone, Serialize)]
pub struct Range {
    #[serde(rename = "startLineNumber")]
    pub start_line: u32,
    #[serde(rename = "startColumn")]
    pub start_col: u32,
    #[serde(rename = "endLineNumber")]
    pub end_line: u32,
    #[serde(rename = "endColumn")]
    pub end_col: u32,
}

impl Range {
    fn from_span(source: &str, span: vbr::span::Span) -> Range {
        let (start_line, start_col) = to_position(source, span.start);
        let (end_line, end_col) = to_position(source, span.end);
        Range { start_line, start_col, end_line, end_col }
    }
}

/// Convert a byte offset into `source` to a 1-based `(line, column)`, with the
/// column in UTF-16 code units. Non-ASCII text before the offset shifts byte
/// positions relative to columns, so we count units explicitly rather than
/// assume one byte per column — the same trap the compiler's spans navigate.
fn to_position(source: &str, byte_offset: usize) -> (u32, u32) {
    let mut offset = byte_offset.min(source.len());
    while !source.is_char_boundary(offset) {
        offset -= 1;
    }
    let before = &source[..offset];
    let line = before.bytes().filter(|&b| b == b'\n').count() as u32 + 1;
    let line_start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let col = source[line_start..offset]
        .chars()
        .map(|c| c.len_utf16() as u32)
        .sum::<u32>()
        + 1;
    (line, col)
}

/// One diagnostic, flattened for the frontend: a level string the UI can style
/// on, the message, the 1-based VBR line, and a Monaco-ready range when the
/// compiler pinned a span (line-only diagnostics leave `range` as `None`).
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub level: String,
    pub message: String,
    pub line: Option<usize>,
    pub range: Option<Range>,
}

/// Everything the editor needs from one compile: the Rust the source became,
/// and the diagnostics to render.
#[derive(Debug, Clone, Serialize)]
pub struct TranspileResult {
    pub rust: String,
    pub diagnostics: Vec<Diagnostic>,
}

/// Transpile a single VBR source string to Rust, collecting diagnostics.
///
/// A pure function of the source — the same call the playground makes in the
/// browser, minus the browser.
fn map_diagnostics(source: &str, items: &[vbr::diagnostics::Diagnostic]) -> Vec<Diagnostic> {
    items
        .iter()
        .map(|d| Diagnostic {
            level: match d.level {
                vbr::diagnostics::Level::Error => "error",
                vbr::diagnostics::Level::Warning => "warning",
                vbr::diagnostics::Level::Note => "note",
            }
            .to_string(),
            message: d.message.clone(),
            line: d.line,
            range: d.span.map(|s| Range::from_span(source, s)),
        })
        .collect()
}

pub fn transpile(source: &str) -> TranspileResult {
    let compiled = vbr::compile(source);
    TranspileResult {
        diagnostics: map_diagnostics(source, &compiled.diagnostic_items),
        rust: compiled.rust,
    }
}

/// The outcome of a Run: which stage it reached, and the output there.
///
/// `stage` is one of `"diagnostics"` (VBR errors blocked it), `"compile"`
/// (rustc rejected the generated Rust), or `"run"` (it built and executed).
#[derive(Debug, Clone, Serialize)]
pub struct RunOutput {
    pub stage: String,
    pub rust: String,
    pub diagnostics: Vec<Diagnostic>,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

impl RunOutput {
    fn blocked(stage: &str, rust: String, diagnostics: Vec<Diagnostic>, stderr: String) -> RunOutput {
        RunOutput {
            stage: stage.to_string(),
            rust,
            diagnostics,
            stdout: String::new(),
            stderr,
            success: false,
        }
    }
}

/// Transpile, compile, and run a single self-contained VBR program, capturing
/// its output. This mirrors what `vbr run` does for a one-file program: it does
/// **not** wire up the standard library or external crates — a program that
/// needs those is a project, and rustc will say so. Kept in the core (not the
/// Tauri shell) so it's exercised by real tests on any platform with rustc.
pub fn run(source: &str) -> RunOutput {
    let compiled = vbr::compile(source);
    let diagnostics = map_diagnostics(source, &compiled.diagnostic_items);

    // VBR errors block the run before we ever reach rustc.
    if compiled.has_errors {
        return RunOutput::blocked("diagnostics", compiled.rust, diagnostics, String::new());
    }

    // A program that pulls the standard library or an external crate isn't a
    // single file — it's a project. Say so kindly rather than letting rustc
    // fail on an unresolved import.
    if !compiled.dependencies.is_empty() || !compiled.stdlib_used.is_empty() {
        return RunOutput::blocked(
            "project",
            compiled.rust,
            diagnostics,
            "This program uses the standard library or an external crate, so it \
             needs the project runner (a folder-based build). That's coming to \
             the IDE; for now, run it from the CLI with `vbr runproject`."
                .to_string(),
        );
    }

    // A private temp directory for this run.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("vbr-ide-run-{nanos}"));
    if std::fs::create_dir_all(&dir).is_err() {
        return RunOutput::blocked(
            "compile",
            compiled.rust,
            diagnostics,
            "Could not create a temporary build directory.".to_string(),
        );
    }
    let src_path = dir.join("main.rs");
    let bin_path = dir.join(format!("vbr-prog{}", std::env::consts::EXE_SUFFIX));

    let result = (|| {
        std::fs::write(&src_path, &compiled.rust)
            .map_err(|e| format!("Could not write the generated Rust: {e}"))?;

        // Compile the single file with rustc (edition 2021, as VBR emits).
        let compile = Command::new("rustc")
            .arg(&src_path)
            .arg("--edition")
            .arg("2021")
            .arg("-o")
            .arg(&bin_path)
            .output()
            .map_err(|e| {
                format!("Could not run rustc — is the Rust toolchain installed? ({e})")
            })?;
        if !compile.status.success() {
            return Err(String::from_utf8_lossy(&compile.stderr).into_owned());
        }

        // Run the built program and capture its output.
        let run = Command::new(&bin_path)
            .output()
            .map_err(|e| format!("Could not launch the built program: {e}"))?;
        Ok((
            String::from_utf8_lossy(&run.stdout).into_owned(),
            String::from_utf8_lossy(&run.stderr).into_owned(),
            run.status.success(),
        ))
    })();

    let _ = std::fs::remove_dir_all(&dir);

    match result {
        Ok((stdout, stderr, success)) => RunOutput {
            stage: "run".to_string(),
            rust: compiled.rust,
            diagnostics,
            stdout,
            stderr,
            success,
        },
        Err(stderr) => RunOutput::blocked("compile", compiled.rust, diagnostics, stderr),
    }
}

// --- Projects ---------------------------------------------------------------

/// One node in the file tree: a file, or a directory with `children`.
#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Vec<FileEntry>,
}

/// An opened folder. `is_project` (a `main.vbr` at the root) drives whether Run
/// builds the whole project or just the current file.
#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub root: String,
    pub name: String,
    pub is_project: bool,
    pub entry: Option<String>,
    pub files: Vec<FileEntry>,
}

/// Directories that are build output or tooling noise — never worth showing.
const SKIP_DIRS: &[&str] = &["build", "target", "node_modules", "dist", ".git"];

/// File extensions worth showing in the tree.
fn is_shown_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|x| x.to_str()),
        Some("vbr" | "rs" | "md" | "toml")
    )
}

fn read_dir_entries(dir: &Path) -> Vec<FileEntry> {
    let mut entries: Vec<FileEntry> = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return entries;
    };
    for e in rd.flatten() {
        let path = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            if SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            let children = read_dir_entries(&path);
            if children.is_empty() {
                continue; // nothing relevant inside — don't clutter the tree
            }
            entries.push(FileEntry {
                name,
                path: path.to_string_lossy().into_owned(),
                is_dir: true,
                children,
            });
        } else if is_shown_file(&path) {
            entries.push(FileEntry {
                name,
                path: path.to_string_lossy().into_owned(),
                is_dir: false,
                children: Vec::new(),
            });
        }
    }
    // Directories first, then files; each alphabetical.
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    entries
}

/// Read a folder into a `Project`: its file tree, and whether it's a VBR project
/// (has a `main.vbr` entry point).
pub fn read_project(root: &Path) -> Project {
    let main = root.join("main.vbr");
    let is_project = main.is_file();
    Project {
        root: root.to_string_lossy().into_owned(),
        name: root
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "folder".to_string()),
        is_project,
        entry: is_project.then(|| main.to_string_lossy().into_owned()),
        files: read_dir_entries(root),
    }
}

/// Locate the `vbr` binary: an explicit `VBR_BIN`, else rely on `PATH`.
fn vbr_binary() -> PathBuf {
    std::env::var("VBR_BIN")
        .ok()
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("vbr"))
}

/// Run a `vbr <subcommand> <target>` and capture its output. Shared by the
/// project actions (runproject / graduate / test). Needs the `vbr` binary on
/// `PATH` or in `VBR_BIN`.
fn run_vbr(subcommand: &str, target: &Path) -> RunOutput {
    match Command::new(vbr_binary()).arg(subcommand).arg(target).output() {
        Ok(o) => RunOutput {
            stage: "run".to_string(),
            rust: String::new(),
            diagnostics: Vec::new(),
            stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
            success: o.status.success(),
        },
        Err(e) => RunOutput::blocked(
            "compile",
            String::new(),
            Vec::new(),
            format!(
                "Couldn't launch the `vbr` binary ({e}). Put `vbr` on your PATH, \
                 or set the VBR_BIN environment variable to its path."
            ),
        ),
    }
}

/// Build and run a whole project via `vbr runproject` (the folder-based runner
/// that wires up the stdlib and crates).
pub fn run_project(root: &Path) -> RunOutput {
    run_vbr("runproject", root)
}

/// Promote a module's generated Rust to source via `vbr graduate` — retires the
/// `.vbr` (kept as `.vbr.graduated`) and drops a `.rs` beside it.
pub fn graduate(target: &Path) -> RunOutput {
    run_vbr("graduate", target)
}

/// Run a project's tests via `vbr test`.
pub fn test_project(target: &Path) -> RunOutput {
    run_vbr("test", target)
}

/// Read a single file's text (for opening a node from the tree).
pub fn read_file(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

/// Write a new auto-numbered `formN.vbr` (a complete Window) into `dir`,
/// returning its path and the Window's name. Numbers up from `form1.vbr` so it
/// never clobbers an existing form.
pub fn create_form(dir: &Path, tree: &Node, target: &str) -> std::io::Result<(PathBuf, String)> {
    let tui = target.eq_ignore_ascii_case("tui") || target.eq_ignore_ascii_case("screen");
    let (file_prefix, name_prefix) = if tui { ("screen", "Screen") } else { ("form", "Form") };
    let mut n = 1;
    loop {
        let file = dir.join(format!("{file_prefix}{n}.vbr"));
        if !file.exists() {
            let name = format!("{name_prefix}{n}");
            std::fs::write(&file, design_to_vbr(tree, &name, target))?;
            return Ok((file, name));
        }
        n += 1;
    }
}

/// Convert a 1-based `(line, column)` — column in UTF-16 units, as Monaco
/// reports — back to a byte offset into `source`. The inverse of `to_position`.
fn to_offset(source: &str, line: u32, col: u32) -> usize {
    // Byte offset of the start of the target line.
    let mut line_start = 0usize;
    if line > 1 {
        let mut current = 1u32;
        let mut found = false;
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                current += 1;
                if current == line {
                    line_start = i + 1;
                    found = true;
                    break;
                }
            }
        }
        if !found {
            return source.len();
        }
    }
    // Walk `col - 1` UTF-16 units into the line.
    let mut remaining = col.saturating_sub(1);
    let mut idx = line_start;
    for ch in source[line_start..].chars() {
        if remaining == 0 || ch == '\n' {
            break;
        }
        let units = ch.len_utf16() as u32;
        if units > remaining {
            break;
        }
        remaining -= units;
        idx += ch.len_utf8();
    }
    idx
}

/// One completion candidate for the frontend: the text to insert, a detail
/// string (VB-facing signature / type), and a lowercased kind the UI maps to an
/// icon.
#[derive(Debug, Clone, Serialize)]
pub struct CompletionItem {
    pub label: String,
    pub detail: String,
    pub kind: String,
}

/// Completions at a cursor position, straight from the compiler's completion
/// engine (the same one the LSP uses) — receiver-typed members after `.`,
/// in-scope names in bare position.
pub fn complete(source: &str, line: u32, col: u32) -> Vec<CompletionItem> {
    let offset = to_offset(source, line, col);
    vbr::complete::completions_at(source, offset)
        .into_iter()
        .map(|c| CompletionItem {
            label: c.label,
            detail: c.detail,
            kind: format!("{:?}", c.kind).to_lowercase(),
        })
        .collect()
}

/// The hover text at a position: the narrowest recorded hover span covering the
/// cursor (VB type · Rust type), or `None`.
pub fn hover(source: &str, line: u32, col: u32) -> Option<String> {
    let offset = to_offset(source, line, col);
    vbr::compile(source)
        .hovers
        .into_iter()
        .filter(|(span, _)| span.start <= offset && offset < span.end)
        .min_by_key(|(span, _)| span.end - span.start)
        .map(|(_, text)| text)
}

/// Go-to-definition: if the cursor is on a use whose declaration the compiler
/// recorded (e.g. a variable → its `Dim`), return the declaration's range.
pub fn definition(source: &str, line: u32, col: u32) -> Option<Range> {
    let offset = to_offset(source, line, col);
    vbr::compile(source)
        .defs
        .into_iter()
        .find(|(use_span, _)| use_span.start <= offset && offset < use_span.end)
        .map(|(_, decl)| Range::from_span(source, decl))
}

#[cfg(test)]
mod tests {
    use super::*;

    // A VBR program's statements live inside `Function Main()`; bare top-level
    // statements are themselves an error (correctly).
    fn in_main(body: &str) -> String {
        format!("Function Main()\n{body}\nEnd Function\n")
    }

    #[test]
    fn transpiles_to_rust() {
        let out = transpile(&in_main("    Debug.Print \"hello\""));
        assert!(
            out.rust.contains("println!"),
            "expected a println! in the generated Rust, got:\n{}",
            out.rust
        );
    }

    #[test]
    fn reports_missing_type_as_error() {
        // `Dim x = 5` with no `As` is the canonical teaching error — it must
        // surface as an error the editor can squiggle.
        let out = transpile(&in_main("    Dim x = 5"));
        assert!(
            out.diagnostics.iter().any(|d| d.level == "error"),
            "expected an error diagnostic, got: {:?}",
            out.diagnostics
        );
    }

    #[test]
    fn clean_source_has_no_errors() {
        let out = transpile(&in_main("    Dim x As Integer = 5\n    Debug.Print x"));
        assert!(
            !out.diagnostics.iter().any(|d| d.level == "error"),
            "clean source should not error, got: {:?}",
            out.diagnostics
        );
    }

    #[test]
    fn position_mapping_is_1_based_and_utf16() {
        let s = "ab\ncd";
        assert_eq!(to_position(s, 0), (1, 1)); // start of file
        assert_eq!(to_position(s, 3), (2, 1)); // 'c' — first col of line 2
        assert_eq!(to_position(s, 4), (2, 2)); // 'd'
        // `é` is 2 UTF-8 bytes but 1 UTF-16 unit, so a byte offset past it must
        // not over-count the column.
        let s2 = "é=x"; // é(2 bytes) = x → 'x' begins at byte 3
        assert_eq!(to_position(s2, 3), (1, 3));
    }

    #[test]
    fn runs_a_simple_program() {
        let out = run(&in_main("    Debug.Print \"hi from vbr\""));
        assert_eq!(out.stage, "run", "should reach the run stage: {out:?}");
        assert!(out.success, "program should exit cleanly: {out:?}");
        assert!(
            out.stdout.contains("hi from vbr"),
            "stdout should carry the printed line, got: {:?}",
            out.stdout
        );
    }

    #[test]
    fn run_is_blocked_by_vbr_errors() {
        let out = run(&in_main("    Dim x = 5")); // missing `As`
        assert_eq!(out.stage, "diagnostics");
        assert!(!out.success);
    }

    #[test]
    fn run_defers_stdlib_programs_to_the_project_runner() {
        // DateTime pulls a crate/stdlib in, so a single-file run can't build it.
        let out = run(&in_main(
            "    Dim now As DateTime = DateTime.Now()\n    Debug.Print now.Year()",
        ));
        assert_eq!(out.stage, "project", "expected the project nudge, got: {out:?}");
        assert!(!out.success);
    }

    #[test]
    fn reads_a_project_folder() {
        // The repo's geometry_project has main.vbr + shapes.vbr + a build/ dir.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/geometry_project");
        let proj = read_project(&root);
        assert!(proj.is_project, "main.vbr present → it's a project");
        assert_eq!(proj.entry.is_some(), true);
        let names: Vec<&str> = proj.files.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"main.vbr"), "tree should list main.vbr: {names:?}");
        assert!(names.contains(&"shapes.vbr"), "tree should list shapes.vbr: {names:?}");
        assert!(
            !proj.files.iter().any(|f| f.name == "build"),
            "the build/ dir should be skipped"
        );
    }

    #[test]
    fn create_form_writes_an_autonumbered_window_file() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("vbr-ide-form-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        let tree = Node {
            kind: "Column".to_string(),
            props: NodeProps::default(),
            children: vec![],
        };
        let (path, name) = create_form(&dir, &tree, "gui").unwrap();
        assert!(path.exists(), "form file should be written");
        assert_eq!(name, "Form1");
        assert_eq!(path.file_name().unwrap(), "form1.vbr");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("Window Form1"));
        assert!(content.contains("Function Main"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn offset_round_trips_with_position() {
        let s = "Function Main()\n    Dim total As Long = 0\nEnd Function\n";
        // Pick a byte offset, map to a position, map back — should be stable.
        let off = s.find("total").unwrap();
        let (line, col) = to_position(s, off);
        assert_eq!(to_offset(s, line, col), off);
    }

    #[test]
    fn completion_offers_members_after_dot() {
        let src = "Function Main()\n    Dim s As String = \"hi\"\n    s.\nEnd Function\n";
        // Line 3 is "    s." → the cursor sits just after the dot, at column 7.
        let items = complete(src, 3, 7);
        assert!(
            !items.is_empty(),
            "expected member completions after `.` on a String"
        );
    }

    #[test]
    fn definition_points_at_the_dim() {
        let src =
            "Function Main()\n    Dim total As Long = 0\n    Debug.Print total\nEnd Function\n";
        // Go-to-def from the use of `total` on line 3.
        let def = definition(src, 3, 19);
        assert!(def.is_some(), "expected a definition for a used variable");
        assert_eq!(
            def.unwrap().start_line,
            2,
            "the declaration is the Dim on line 2"
        );
    }

    #[test]
    fn hover_reports_a_variable_type() {
        let src =
            "Function Main()\n    Dim total As Long = 0\n    Debug.Print total\nEnd Function\n";
        // `total` on line 3 spans columns 17..21; hover inside it.
        let h = hover(src, 3, 19);
        assert!(h.is_some(), "expected hover text over a known variable");
        assert!(
            h.as_deref().unwrap().contains("Long"),
            "hover should mention the VB type, got: {h:?}"
        );
    }

    #[test]
    fn syntax_error_carries_a_range() {
        // A trailing token with no operator is a span-pinned parse error.
        let out = transpile(&in_main("    Debug.Print 1 2"));
        let pinned = out.diagnostics.iter().find(|d| d.range.is_some());
        let r = &pinned
            .expect("a syntax error should pin a range")
            .range
            .as_ref()
            .unwrap();
        assert_eq!(r.start_line, 2, "the error is on line 2 (inside Main)");
        assert!(r.start_col >= 1 && r.end_col >= r.start_col);
    }
}
