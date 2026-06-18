//! VBR — VBA syntax in, idiomatic Rust out.
//!
//! The whole pipeline is exposed here so it can be driven both by the CLI
//! (`src/main.rs`) and by the integration tests.

pub mod ast;
pub mod diagnostics;
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
}

/// Run the full lexer → parser → transpiler pipeline over `source`.
pub fn compile(source: &str) -> Compiled {
    let mut diags = Diagnostics::new();
    let tokens = lexer::lex(source);
    let program = parser::parse(tokens, &mut diags);
    let rust = transpiler::transpile(&program, &mut diags);

    Compiled {
        rust,
        diagnostics: diags.items().iter().map(|d| d.render()).collect(),
        has_errors: diags.has_errors(),
    }
}
