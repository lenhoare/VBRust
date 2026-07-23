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

/// One diagnostic, flattened for the frontend: a level string the UI can style
/// on, the message, the 1-based VBR line, and the byte span the compiler pinned
/// (when it had one — line-only diagnostics leave the span `None`).
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub level: String,
    pub message: String,
    pub line: Option<usize>,
    pub start: Option<usize>,
    pub end: Option<usize>,
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
pub fn transpile(source: &str) -> TranspileResult {
    let compiled = vbr::compile(source);
    let diagnostics = compiled
        .diagnostic_items
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
            start: d.span.map(|s| s.start),
            end: d.span.map(|s| s.end),
        })
        .collect();
    TranspileResult {
        rust: compiled.rust,
        diagnostics,
    }
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
}
