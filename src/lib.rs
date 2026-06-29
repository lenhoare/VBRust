//! VBR — VBA syntax in, idiomatic Rust out.
//!
//! The whole pipeline is exposed here so it can be driven both by the CLI
//! (`src/main.rs`) and by the integration tests.

pub mod ast;
pub mod diagnostics;
pub mod gui;
pub mod lexer;
pub mod parser;
pub mod resolver;
pub mod transpiler;

use diagnostics::Diagnostics;

/// The result of transpiling one VBR source string.
pub struct Compiled {
    /// The generated Rust source.
    pub rust: String,
    /// Every diagnostic, already rendered (`✘ / ⚠ / ℹ`).
    pub diagnostics: Vec<String>,
    /// True if any diagnostic was a hard error (no Rust should be used).
    pub has_errors: bool,
    /// Crate dependencies declared with `Use <crate> <version>` → Cargo lines.
    pub dependencies: Vec<(String, String)>,
    /// Stdlib namespaces used (e.g. `Json`, `Http`) → which `vbr_stdlib`
    /// features to enable.
    pub stdlib_used: Vec<String>,
}

/// Run the full pipeline over `source` as a single standalone file (the entry,
/// with no sibling modules).
pub fn compile(source: &str) -> Compiled {
    compile_module(source, &[], true)
}

/// Compile one file of a multifile project. `modules` are the other project
/// module names (snake-cased file stems), used to qualify cross-module calls;
/// `is_entry` marks the crate root (gets `mod <name>;` declarations and `fn main`).
pub fn compile_module(source: &str, modules: &[String], is_entry: bool) -> Compiled {
    let mut diags = Diagnostics::new();
    let tokens = lexer::lex(source);
    let program = parser::parse(tokens, &mut diags);
    let mut dependencies: Vec<(String, String)> = program
        .uses
        .iter()
        .map(|u| (u.crate_name.clone(), u.version.clone()))
        .collect();
    // A GUI program needs Iced (a project build, like the stdlib/crate cases).
    if !program.windows.is_empty() {
        dependencies.push(("iced".to_string(), "0.13".to_string()));
    }
    let rust = transpiler::transpile_module(&program, modules, is_entry, &mut diags);
    let stdlib_used = transpiler::stdlib_used(&diags);

    Compiled {
        rust,
        diagnostics: diags.items().iter().map(|d| d.render()).collect(),
        has_errors: diags.has_errors(),
        dependencies,
        stdlib_used,
    }
}

/// The Rust module name for a project file stem (`MyHelpers` → `my_helpers`),
/// matching how identifiers are snake-cased everywhere else.
pub fn module_name(stem: &str) -> String {
    transpiler::to_snake(stem)
}
