//! VBR — VBA syntax in, idiomatic Rust out.
//!
//! The whole pipeline is exposed here so it can be driven both by the CLI
//! (`src/main.rs`) and by the integration tests.

pub mod ast;
pub mod complete;
pub mod diagnostics;
pub mod gui;
pub mod lexer;
pub mod parser;
pub mod resolver;
pub mod span;
pub mod surface;
pub mod transpiler;
pub mod tui;
pub mod web;

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
    /// The structured diagnostics (level, message, line) — for tools like the
    /// language server that need more than the pre-rendered strings.
    pub diagnostic_items: Vec<diagnostics::Diagnostic>,
    /// (generated-Rust line, VBR source line) checkpoints, ascending — used to
    /// translate rustc errors back to the `.vbr` source. Empty for GUI/TUI
    /// programs (their emitters don't keep line order yet).
    pub line_map: Vec<(usize, usize)>,
    /// A web program's browser-tab title (the launched `Page`'s `Title`, or its
    /// name) — written into the generated `index.html`. `None` for non-web.
    pub web_title: Option<String>,
    /// A web program's stylesheet — the launched page's `Theme` as CSS plus any
    /// `Css … End Css` blocks — for the generated `index.html`'s `<style>`.
    pub web_style: Option<String>,
    /// Local files the pages reference (`Image "logo.png"`) — each becomes a
    /// trunk copy-file directive in the generated `index.html`.
    pub web_assets: Vec<String>,
    /// The `Test` blocks in this file, paired with their generated `#[test] fn`
    /// name — so `vbr test` can translate a `cargo test` result line back to the
    /// human description and `.vbr` line.
    pub tests: Vec<TestInfo>,
    /// What the resolver knows about each identifier use: (byte span, display
    /// text like ``total As Long · Rust: `i64` ``). The language server answers
    /// hover by finding the entry whose span covers the cursor.
    pub hovers: Vec<(span::Span, String)>,
    /// (use span, declaration span) pairs for identifiers — go-to-definition.
    pub defs: Vec<(span::Span, span::Span)>,
    /// Every identifier occurrence the resolver understood, with its declared
    /// type — what completion uses to answer `x.` (hovers derive from this).
    pub symbols: Vec<diagnostics::SymbolInfo>,
}

/// One `Test` block's identity, bridging the VBR source and the generated
/// `#[test] fn`.
#[derive(Debug, Clone)]
pub struct TestInfo {
    /// The generated Rust function name (a slug of the description).
    pub fn_name: String,
    /// The human description — the spec sentence shown in `vbr test` output.
    pub description: String,
    /// The `.vbr` source line of the `Test` block.
    pub line: usize,
}

/// Run the full pipeline over `source` as a single standalone file (the entry,
/// with no sibling modules).
pub fn compile(source: &str) -> Compiled {
    compile_with(source, &[], &resolver::ProjectInterfaces::new(), true, false)
}

/// Compile for the browser (`vbr runweb`): a `Screen` renders through Ratzilla
/// (the terminal drawn into the DOM) instead of crossterm. A `Page` is always
/// a web app, so for it this is the same as `compile`.
pub fn compile_web(source: &str) -> Compiled {
    compile_with(source, &[], &resolver::ProjectInterfaces::new(), true, true)
}

/// Harvest one module's public surface — pass 1 of a project compile. Each
/// file is parsed once for its interface (function signatures, constants);
/// pass 2 (`compile_module`) then resolves qualified calls against the
/// siblings' interfaces exactly as it does local ones. Parse problems are
/// ignored here — they resurface, with diagnostics, when the module itself
/// is compiled.
pub fn module_interface(source: &str) -> resolver::ModuleInterface {
    let mut diags = Diagnostics::new();
    let tokens = lexer::lex(source);
    let program = parser::parse(tokens, &mut diags);
    resolver::module_interface(&program)
}

/// Compile one file of a multifile project. `modules` are the other project
/// module names (lowercased file stems), used to qualify cross-module calls;
/// `interfaces` their harvested surfaces (`module_interface`), giving those
/// calls the full local argument treatment; `is_entry` marks the crate root
/// (gets `mod <name>;` declarations and `fn main`).
pub fn compile_module(
    source: &str,
    modules: &[String],
    interfaces: &resolver::ProjectInterfaces,
    is_entry: bool,
) -> Compiled {
    compile_with(source, modules, interfaces, is_entry, false)
}

/// The browser-targeted form of `compile_module` (`vbr runweb` on a project).
pub fn compile_module_web(
    source: &str,
    modules: &[String],
    interfaces: &resolver::ProjectInterfaces,
    is_entry: bool,
) -> Compiled {
    compile_with(source, modules, interfaces, is_entry, true)
}

fn compile_with(
    source: &str,
    modules: &[String],
    interfaces: &resolver::ProjectInterfaces,
    is_entry: bool,
    web: bool,
) -> Compiled {
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
    // A TUI program (a `Screen`) needs ratatui (crossterm comes with it) — or,
    // in the browser, Ratzilla, which draws the same ratatui widgets into the
    // DOM (it builds on ratatui 0.30, so the web project pins that).
    if !program.screens.is_empty() {
        if web {
            dependencies.push(("ratzilla".to_string(), "0.3".to_string()));
            dependencies.push(("ratatui".to_string(), "0.30".to_string()));
        } else {
            dependencies.push(("ratatui".to_string(), "0.29".to_string()));
        }
    }
    // A web program (a `Page`) needs Yew, built for WebAssembly (`vbr runweb`).
    if !program.pages.is_empty() {
        dependencies.push(("yew".to_string(), "0.21".to_string()));
    }
    // The launched page's (or, on the web, screen's) title, for the generated
    // index.html's <title>.
    let web_title = if !program.pages.is_empty() {
        surface::launched(&program, |name| {
            program.pages.iter().find(|p| p.name.eq_ignore_ascii_case(name))
        })
        .or_else(|| program.pages.first())
        .map(|p| p.title.clone().unwrap_or_else(|| p.name.clone()))
    } else if web && !program.screens.is_empty() {
        surface::launched(&program, |name| {
            program.screens.iter().find(|s| s.name.eq_ignore_ascii_case(name))
        })
        .or_else(|| program.screens.first())
        .map(|s| s.title.clone().unwrap_or_else(|| s.name.clone()))
    } else {
        None
    };
    let web_style = web::page_style(&program);
    let web_assets = web::page_assets(&program);
    let rust = transpiler::transpile_module(&program, modules, interfaces, is_entry, web, &mut diags);
    // An inline `Python` block runs via pyo3 (real CPython) — pull it in only when
    // one is actually used, so nothing else pays for it. Detected from the emitted
    // marker, like the other conditional deps (image/canvas/spawn_blocking).
    if rust.contains("pyo3::Python::with_gil") {
        dependencies.push(("pyo3".to_string(), "0.23".to_string()));
    }
    let stdlib_used = transpiler::stdlib_used(&diags);
    let line_map = diags.take_line_map();
    let symbols = diags.take_symbols();
    let hovers = symbols.iter().map(|s| (s.span, s.display.clone())).collect();
    let defs = diags.take_defs();
    // Pair each `Test` block with its generated `#[test] fn` name (the same slug
    // the emitter used), so the runner can map a `cargo test` line to it.
    let tests: Vec<TestInfo> = program
        .tests
        .iter()
        .zip(transpiler::test_fn_names(&program.tests))
        .map(|(t, fn_name)| TestInfo {
            fn_name,
            description: t.description.clone(),
            line: t.line,
        })
        .collect();

    Compiled {
        rust,
        diagnostics: diags.items().iter().map(|d| d.render()).collect(),
        has_errors: diags.has_errors(),
        dependencies,
        stdlib_used,
        diagnostic_items: diags.items().to_vec(),
        line_map,
        web_title,
        web_style,
        web_assets,
        tests,
        hovers,
        defs,
        symbols,
    }
}

/// The Rust module name for a project file stem (`MyHelpers` → `myhelpers`),
/// matching how identifiers are lowercased everywhere else.
pub fn module_name(stem: &str) -> String {
    transpiler::rust_name(stem)
}
