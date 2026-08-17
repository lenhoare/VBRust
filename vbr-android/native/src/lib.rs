//! Compiler-facing core of the Android VBR editor.
//!
//! Same job as `vbr-ide-core`, aimed at a phone: transpile to **C**, run with
//! TinyCC in-process. JNI is feature-gated (`jni-bridge`); host tests call
//! [`compile`] / [`run`] directly.

use serde::Serialize;
use std::sync::Mutex;

mod jni_api;
mod screen;

pub use screen::{detect_surface, run_main, screen_dispatch, screen_start, screen_stop};

static TCC_DIR: Mutex<Option<String>> = Mutex::new(None);

/// Directory containing TinyCC's `libtcc1.a` / `runmain.o` for this ABI.
pub fn set_tcc_dir(path: String) {
    *TCC_DIR.lock().unwrap() = Some(path);
}

fn tcc_dir() -> String {
    TCC_DIR
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| option_env!("VBR_TCCDIR").unwrap_or(".").to_string())
}

/// Monaco-style 1-based range; columns in UTF-16 code units.
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

fn to_offset(source: &str, line: u32, col: u32) -> usize {
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

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub level: String,
    pub message: String,
    pub line: Option<usize>,
    pub range: Option<Range>,
}

fn map_diagnostics(source: &str, items: &[vbr::diagnostics::Diagnostic]) -> Vec<Diagnostic> {
    items
        .iter()
        .map(|d| Diagnostic {
            level: match d.level {
                vbr::diagnostics::Level::Error => "error".into(),
                vbr::diagnostics::Level::Warning => "warning".into(),
                vbr::diagnostics::Level::Note => "note".into(),
            },
            message: d.message.clone(),
            line: d.line,
            range: d.span.map(|s| Range::from_span(source, s)),
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct CompileResult {
    pub code: String,
    pub diagnostics: Vec<Diagnostic>,
    pub has_errors: bool,
    /// A stdlib/surface this phone build won't run, if any.
    pub blocked: Option<String>,
    /// `(c_line, vbr_line)` 1-based checkpoints for the generated-C pane.
    pub line_map: Vec<(usize, usize)>,
    /// `"screen"` / `"window"` / `"page"` when the source launches a surface.
    pub surface: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunResult {
    pub stage: String,
    pub code: String,
    pub diagnostics: Vec<Diagnostic>,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
    pub blocked: Option<String>,
    /// `"screen"` when Run should open the TUI host instead of TinyCC.
    pub surface: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompletionItem {
    pub label: String,
    pub detail: String,
    pub kind: String,
}

/// Namespaces / headers the in-process TinyCC runner does not ship yet.
fn blocked_reason(c: &vbr::CCompiled) -> Option<String> {
    if c.has_errors {
        return None;
    }
    if c.is_project() {
        let extra = if !c.vendored.is_empty() {
            c.vendored.join(", ")
        } else {
            c.link_flags
                .iter()
                .filter(|f| *f != "m")
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        };
        return Some(format!(
            "This program needs extra C libraries ({extra}). The phone runner is TinyCC \
             in-process, which covers the core language — Json/Database/Http stay on \
             the desktop C project build for now."
        ));
    }
    let code = &c.code;
    let hit = [
        ("sqlite3.h", "Database"),
        ("curl/curl.h", "Http"),
        ("cJSON.h", "Json"),
        ("sys/stat.h", "FileSystem"),
        ("<time.h>", "DateTime"),
        ("regex.h", "Regex"),
        ("sys/wait.h", "Shell"),
    ];
    for (needle, name) in hit {
        if code.contains(needle) {
            return Some(format!(
                "{name} isn't in the Android runner yet — it needs POSIX/system \
                 headers whose layouts have to match the phone's libc. Core language \
                 programs (Debug.Print, maths, types, Match, Vec, Result) run here."
            ));
        }
    }
    None
}

pub fn compile(source: &str) -> CompileResult {
    let base = vbr::compile(source);
    let mut diagnostics = map_diagnostics(source, &base.diagnostic_items);
    let surface = detect_surface(source).map(|s| s.to_string());
    if surface.as_deref() == Some("screen") {
        diagnostics.retain(|d| {
            !d.message.contains("GUI/TUI/Web surfaces")
        });
        return CompileResult {
            code: String::new(),
            diagnostics,
            has_errors: base.has_errors,
            blocked: None,
            line_map: Vec::new(),
            surface,
        };
    }
    if surface.as_deref() == Some("window") {
        return CompileResult {
            code: String::new(),
            diagnostics,
            has_errors: base.has_errors,
            blocked: Some(
                "Window (the graphical GUI) isn't on the phone yet. Screen — the \
                 same State/View/Events model as the desktop TUI — is: tap F9 on a \
                 Screen program."
                    .into(),
            ),
            line_map: Vec::new(),
            surface,
        };
    }
    if surface.as_deref() == Some("page") {
        return CompileResult {
            code: String::new(),
            diagnostics,
            has_errors: base.has_errors,
            blocked: Some(
                "Page (the browser target) isn't on the phone. A Screen is the \
                 TUI that runs here."
                    .into(),
            ),
            line_map: Vec::new(),
            surface,
        };
    }
    let out = vbr::compile_c(source);
    for w in &out.warnings {
        diagnostics.push(Diagnostic {
            level: "note".into(),
            message: w.trim_start_matches(['⚠', ' ']).to_string(),
            line: None,
            range: None,
        });
    }
    let blocked = blocked_reason(&out);
    CompileResult {
        code: out.code,
        diagnostics,
        has_errors: out.has_errors || base.has_errors,
        blocked,
        line_map: out.line_map,
        surface,
    }
}

pub fn run(source: &str) -> RunResult {
    let compiled = compile(source);
    if compiled.has_errors {
        return RunResult {
            stage: "diagnostics".into(),
            code: compiled.code,
            diagnostics: compiled.diagnostics,
            stdout: String::new(),
            stderr: String::new(),
            success: false,
            blocked: compiled.blocked,
            surface: compiled.surface,
        };
    }
    if compiled.surface.as_deref() == Some("screen") {
        return RunResult {
            stage: "screen".into(),
            code: compiled.code,
            diagnostics: compiled.diagnostics,
            stdout: String::new(),
            stderr: String::new(),
            success: true,
            blocked: None,
            surface: compiled.surface,
        };
    }
    if let Some(reason) = compiled.blocked.clone() {
        return RunResult {
            stage: "blocked".into(),
            code: compiled.code,
            diagnostics: compiled.diagnostics,
            stdout: String::new(),
            stderr: reason.clone(),
            success: false,
            blocked: Some(reason),
            surface: compiled.surface,
        };
    }
    // Phone apps cannot mprotect JIT pages executable. TinyCC's tcc_relocate
    // hangs there (S7 and S20 FE). Interpret Main() instead; the C pane still
    // shows what `vbr c` would emit. Host tests keep TinyCC.
    if cfg!(target_os = "android") {
        return match run_main(source) {
            Ok(stdout) => RunResult {
                stage: "run".into(),
                code: compiled.code,
                diagnostics: compiled.diagnostics,
                stdout,
                stderr: String::new(),
                success: true,
                blocked: None,
                surface: compiled.surface,
            },
            Err(stderr) => RunResult {
                stage: "run".into(),
                code: compiled.code,
                diagnostics: compiled.diagnostics,
                stdout: String::new(),
                stderr,
                success: false,
                blocked: None,
                surface: compiled.surface,
            },
        };
    }
    #[allow(unreachable_code)]
    match run_c(&compiled.code) {
        Ok((stdout, stderr, success)) => RunResult {
            stage: "run".into(),
            code: compiled.code,
            diagnostics: compiled.diagnostics,
            stdout,
            stderr,
            success,
            blocked: None,
            surface: compiled.surface,
        },
        Err(stderr) => RunResult {
            stage: "compile".into(),
            code: compiled.code,
            diagnostics: compiled.diagnostics,
            stdout: String::new(),
            stderr,
            success: false,
            blocked: None,
            surface: compiled.surface,
        },
    }
}

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

pub fn hover(source: &str, line: u32, col: u32) -> Option<String> {
    let offset = to_offset(source, line, col);
    vbr::compile(source)
        .hovers
        .into_iter()
        .filter(|(span, _)| span.start <= offset && offset < span.end)
        .min_by_key(|(span, _)| span.end - span.start)
        .map(|(_, text)| text)
}

#[cfg(has_tcc)]
mod tcc {
    use std::ffi::{CStr, CString};
    use std::os::raw::{c_char, c_int};

    #[repr(C)]
    struct VbrTccResult {
        stdout_text: *mut c_char,
        stderr_text: *mut c_char,
        exit_code: c_int,
        ok: c_int,
    }

    extern "C" {
        fn vbr_tcc_run(
            c_source: *const c_char,
            tccdir: *const c_char,
            use_prelude: c_int,
            out: *mut VbrTccResult,
        ) -> c_int;
        fn vbr_tcc_result_free(out: *mut VbrTccResult);
    }

    fn take_cstr(p: *mut c_char) -> String {
        if p.is_null() {
            return String::new();
        }
        unsafe { CStr::from_ptr(p).to_string_lossy().into_owned() }
    }

    pub fn run_c_with(code: &str, prelude: bool) -> Result<(String, String, bool), String> {
        let src = CString::new(code).map_err(|_| "generated C contains an interior NUL".to_string())?;
        let dir = CString::new(super::tcc_dir()).unwrap_or_else(|_| CString::new(".").unwrap());
        let prelude = if prelude { 1 } else { 0 };
        unsafe {
            let mut out = VbrTccResult {
                stdout_text: std::ptr::null_mut(),
                stderr_text: std::ptr::null_mut(),
                exit_code: 0,
                ok: 0,
            };
            let rc = vbr_tcc_run(src.as_ptr(), dir.as_ptr(), prelude, &mut out);
            let stdout = take_cstr(out.stdout_text);
            let stderr = take_cstr(out.stderr_text);
            let ok = out.ok != 0;
            let exit = out.exit_code;
            vbr_tcc_result_free(&mut out);
            if rc != 0 || !ok {
                return Err(if stderr.is_empty() {
                    "TinyCC could not compile the generated C.".into()
                } else {
                    stderr
                });
            }
            Ok((stdout, stderr, exit == 0))
        }
    }

    pub fn run_c(code: &str) -> Result<(String, String, bool), String> {
        run_c_with(code, cfg!(target_os = "android"))
    }
}

#[cfg(has_tcc)]
fn run_c(code: &str) -> Result<(String, String, bool), String> {
    tcc::run_c(code)
}

#[cfg(not(has_tcc))]
fn run_c(_code: &str) -> Result<(String, String, bool), String> {
    Err(
        "TinyCC is not linked into this build. On a dev machine run \
         vbr-android/scripts/fetch-tcc.sh, then rebuild. On a phone, the APK \
         must be built with the NDK so libtcc is inside libvbr_android.so."
            .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_main(body: &str) -> String {
        format!("Function Main()\n{body}\nEnd Function\n")
    }

    #[test]
    fn compiles_hello_to_c() {
        let src = in_main("    Debug.Print \"hello\"");
        let out = compile(&src);
        assert!(!out.has_errors, "{:?}", out.diagnostics);
        assert!(out.code.contains("printf"), "{}", out.code);
        assert!(out.code.contains("int main"), "{}", out.code);
        assert!(out.blocked.is_none(), "{:?}", out.blocked);
        assert!(
            out.line_map.iter().any(|&(_, v)| v == 2),
            "expected a checkpoint for VBR line 2 (Debug.Print), got {:?}",
            out.line_map
        );
        let c_lines: Vec<&str> = out.code.lines().collect();
        for &(c, v) in &out.line_map {
            if v == 2 {
                let line = c_lines.get(c.saturating_sub(1)).copied().unwrap_or("");
                assert!(
                    line.contains("printf"),
                    "VBR line 2 should map to a printf, C line {c} was {line:?}"
                );
            }
        }
    }

    #[test]
    fn missing_type_is_an_error() {
        let out = compile(&in_main("    Dim x = 5"));
        assert!(out.has_errors);
        assert!(out.diagnostics.iter().any(|d| d.level == "error"));
    }

    #[test]
    fn window_is_blocked_from_run_with_a_note() {
        let src = include_str!("../../../examples/counter.vbr");
        let out = compile(src);
        assert_eq!(out.surface.as_deref(), Some("window"));
        assert!(out.blocked.is_some(), "Window should stay blocked on the phone");
    }

    #[test]
    fn screen_runs_in_the_host_not_tinycc() {
        let src = include_str!("../../../examples/tui_counter.vbr");
        let out = compile(src);
        assert!(!out.has_errors, "{:?}", out.diagnostics);
        assert_eq!(out.surface.as_deref(), Some("screen"));
        assert!(out.blocked.is_none(), "{:?}", out.blocked);
        let ran = run(src);
        assert_eq!(ran.stage, "screen");
        assert!(ran.success);
    }

    #[test]
    fn run_hello_via_tcc() {
        if !cfg!(has_tcc) {
            return;
        }
        let src = in_main("    Debug.Print \"hello, android\"");
        let out = run(&src);
        assert_eq!(out.stage, "run", "stderr: {}", out.stderr);
        assert!(out.success, "stderr: {}\nC:\n{}", out.stderr, out.code);
        assert!(
            out.stdout.contains("hello, android"),
            "stdout was: {:?}",
            out.stdout
        );
    }

    #[test]
    fn run_maths_example() {
        if !cfg!(has_tcc) {
            return;
        }
        let src = include_str!("../../../examples/maths.vbr");
        let out = run(src);
        assert!(out.success, "stderr: {}\nC:\n{}", out.stderr, out.code);
        assert!(out.stdout.contains("sqrt(9)"), "stdout: {:?}", out.stdout);
    }

    #[test]
    fn run_hello_via_interpreter() {
        let src = in_main("    Debug.Print \"hello, android\"");
        let out = run_main(&src).expect("Main host");
        assert!(
            out.contains("hello, android"),
            "stdout was: {out:?}"
        );
    }

    #[cfg(has_tcc)]
    #[test]
    fn android_prelude_runs_hello() {
        let src = in_main("    Debug.Print \"prelude-ok\"");
        let c = compile(&src);
        assert!(!c.has_errors && c.blocked.is_none(), "{:?}", c.diagnostics);
        let (stdout, stderr, ok) = super::tcc::run_c_with(&c.code, true)
            .expect("prelude TinyCC compile");
        assert!(ok, "stderr: {stderr}");
        assert!(stdout.contains("prelude-ok"), "stdout: {stdout:?}");
    }

    #[cfg(has_tcc)]
    #[test]
    fn android_prelude_runs_maths() {
        let src = include_str!("../../../examples/maths.vbr");
        let c = compile(src);
        assert!(!c.has_errors && c.blocked.is_none(), "{:?}", c.diagnostics);
        let (stdout, stderr, ok) = super::tcc::run_c_with(&c.code, true)
            .expect("prelude TinyCC compile");
        assert!(ok, "stderr: {stderr}\nC:\n{}", c.code);
        assert!(stdout.contains("sqrt(9)"), "stdout: {stdout:?}");
    }
}
