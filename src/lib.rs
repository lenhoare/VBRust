//! Bust — VBA syntax in, idiomatic Rust out.
//!
//! The whole pipeline is exposed here so it can be driven both by the CLI
//! (`src/main.rs`) and by the integration tests.

pub mod ast;
pub mod c;
pub mod complete;
pub mod diagnostics;
pub mod fmtpat;
pub mod godot;
pub mod gpu;
pub mod gui;
pub mod help;
pub mod iter;
pub mod lexer;
pub mod parser;
pub mod pattern;
pub mod python;
pub mod resolver;
pub mod span;
pub mod surface;
pub mod theme;
pub mod transpiler;
pub mod types;
pub mod tui;
pub mod web;

use diagnostics::Diagnostics;

/// The result of transpiling one Bust source string.
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
    /// (generated-Rust line, Bust source line) checkpoints, ascending — used to
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

/// One `Test` block's identity, bridging the Bust source and the generated
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

/// The Rust a Bust *fragment* becomes — a sequence of statements, not a whole
/// program. Used to embed Bust inside Rust (`vbr embed`, and later a `vbr!{}`
/// macro): the statements are spliced straight into a Rust function body.
pub struct Fragment {
    /// The generated Rust statements (dedented one level, no `fn` wrapper).
    pub rust: String,
    /// Every diagnostic, already rendered (`✘ / ⚠ / ℹ`).
    pub diagnostics: Vec<String>,
    /// True if compilation failed — `rust` is then empty.
    pub has_errors: bool,
}

/// Transpile a Bust fragment (statements) to a Rust statement block. The trick:
/// wrap it in `Function Main()`, run the normal pipeline, then lift out the body
/// of the generated `fn main`. So a fragment reuses the whole compiler. A
/// fragment that would need *top-level* items (imports/helpers) can't be inlined
/// into a block and is reported as an error.
///
/// Embedding contract: a fragment is embedded *inside* Rust, so names it doesn't
/// recognise (`square`, `limit`) are that surrounding Rust — the resolver's
/// "unknown passes through, rustc is the backstop" behaviour is what makes that
/// work. A future diagnostic that flags unknown names (task #24) must exempt
/// fragments; see the coherence note in `resolver.rs`'s `Call` arm.
pub fn compile_fragment(source: &str) -> Fragment {
    let wrapped = format!("Function Main()\n{}\nEnd Function\n", source);
    let compiled = compile(&wrapped);
    // The `Function Main()` header is one line above the fragment, so every
    // reported line is one too high — shift it back onto the fragment's own lines.
    let diagnostics = fragment_diagnostics(&compiled.diagnostic_items);

    if compiled.has_errors {
        return Fragment {
            rust: String::new(),
            diagnostics,
            has_errors: true,
        };
    }
    match extract_fn_main_body(&compiled.rust) {
        Some(body) => Fragment {
            rust: body,
            diagnostics,
            has_errors: false,
        },
        None => {
            let mut diagnostics = diagnostics;
            diagnostics.push(
                "✘ This fragment needs top-level items (imports or helper definitions) that \
                 can't be inlined into a Rust block — keep an embedded fragment to plain \
                 statements."
                    .to_string(),
            );
            Fragment {
                rust: String::new(),
                diagnostics,
                has_errors: true,
            }
        }
    }
}

/// Re-render the wrapped compile's diagnostics against the fragment's own lines
/// (undo the `Function Main()` header line the wrapper added).
fn fragment_diagnostics(items: &[diagnostics::Diagnostic]) -> Vec<String> {
    items
        .iter()
        .map(|d| {
            let shifted = diagnostics::Diagnostic {
                line: d.line.map(|l| l.saturating_sub(1).max(1)),
                ..d.clone()
            };
            shifted.render()
        })
        .collect()
}

/// Lift the statements out of the generated `fn vbr_main` (falling back to
/// `fn main` for hosts that don't wrap). Returns `None` if any top-level item
/// precedes the entry function (those can't live inside a Rust block).
fn extract_fn_main_body(rust: &str) -> Option<String> {
    let lines: Vec<&str> = rust.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.trim().starts_with("fn vbr_main("))
        .or_else(|| lines.iter().position(|l| l.trim() == "fn main() {"))?;
    if lines[..start].iter().any(|l| !l.trim().is_empty()) {
        return None; // top-level items precede the entry — not inlineable
    }
    // Close of this function: the first column-0 `}`, not the later `fn main` wrapper.
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, l)| **l == "}")
        .map(|(i, _)| i)?;
    if end <= start {
        return None;
    }
    let body: Vec<String> = lines[start + 1..end]
        .iter()
        .map(|l| l.strip_prefix("    ").unwrap_or(l).to_string())
        .collect();
    Some(body.join("\n"))
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
    if !program.windows.is_empty() || !program.sketches.is_empty() {
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
    if rust.contains("tui_textarea::") {
        dependencies.push(("tui-textarea".to_string(), "0.7".to_string()));
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

/// The result of transpiling one Bust source string to **Python** (an alternative
/// target to Rust — the core language, not the GUI/TUI/Web surfaces).
pub struct PyCompiled {
    /// The generated Python source.
    pub code: String,
    /// Parse diagnostics, already rendered.
    pub diagnostics: Vec<String>,
    /// True if a parse error means no Python should be used.
    pub has_errors: bool,
    /// Constructs that couldn't cross to Python cleanly (rendered `⚠` notes).
    pub warnings: Vec<String>,
    /// Standard-library namespaces used — non-empty means the output is a
    /// *project* that imports the bundled `vbrpy` package.
    pub stdlib_used: Vec<String>,
    /// pip requirement lines from `Use <module> <version>` (plus our own deps) —
    /// non-empty means a `requirements.txt` is written beside `main.py`.
    pub requirements: Vec<String>,
}

/// Transpile `source` to Python. Parse errors are reported through
/// `diagnostics`/`has_errors`, exactly as the Rust path does; the resolver's
/// Rust-specific work (casts, borrows) is skipped — Python needs none of it.
pub fn compile_python(source: &str) -> PyCompiled {
    let mut diags = Diagnostics::new();
    let tokens = lexer::lex(source);
    let program = parser::parse(tokens, &mut diags);
    if diags.has_errors() {
        return PyCompiled {
            code: String::new(),
            diagnostics: diags.items().iter().map(|d| d.render()).collect(),
            has_errors: true,
            warnings: Vec::new(),
            stdlib_used: Vec::new(),
            requirements: Vec::new(),
        };
    }
    let out = python::emit_python(&program);
    PyCompiled {
        code: out.code,
        diagnostics: diags.items().iter().map(|d| d.render()).collect(),
        has_errors: false,
        warnings: out.warnings,
        stdlib_used: out.stdlib_used,
        requirements: out.requirements,
    }
}

/// The result of transpiling one Bust source string to **C** (a third target
/// beside Rust and Python — slice 1: the core language over scalars + strings).
pub struct CCompiled {
    /// The generated C source (a single self-contained `.c` with the runtime
    /// inlined at the top).
    pub code: String,
    /// Parse diagnostics, already rendered.
    pub diagnostics: Vec<String>,
    /// True if a parse error means no C should be used.
    pub has_errors: bool,
    /// Constructs that couldn't cross to C cleanly (rendered `⚠` notes).
    pub warnings: Vec<String>,
    /// Vendored C libraries this program needs bundled beside `main.c` (base
    /// names under `csupport/`, e.g. `"cJSON"` → `cJSON.c` + `cJSON.h`). When
    /// non-empty the output is a *project folder* (`main.c` + the sources + a
    /// `Makefile`), the parallel of Python's `vbrpy/` project mode.
    pub vendored: Vec<String>,
    /// Extra linker flags the `Makefile` must pass (`"m"` → `-lm`, `"curl"` →
    /// `-lcurl`) — a system library the program links rather than vendoring.
    pub link_flags: Vec<String>,
    /// `(c_line, vbr_line)` 1-based checkpoints for an IDE's generated-C pane.
    pub line_map: Vec<(usize, usize)>,
}

impl CCompiled {
    /// A program is a *project* (a folder + `Makefile`, not one file) when it
    /// needs anything beyond the plain `cc main.c -lm` build — a vendored library
    /// source, or a link flag other than `libm` (`-lsqlite3`, `-lcurl`).
    pub fn is_project(&self) -> bool {
        !self.vendored.is_empty() || self.link_flags.iter().any(|f| f != "m")
    }
}

/// Transpile `source` to C. Like the Python path, the resolver's Rust-specific
/// rewrites are skipped; the C backend gets its types from the neutral typing
/// pass (`types::type_program`) instead.
pub fn compile_c(source: &str) -> CCompiled {
    let mut diags = Diagnostics::new();
    let tokens = lexer::lex(source);
    let program = parser::parse(tokens, &mut diags);
    if diags.has_errors() {
        return CCompiled {
            code: String::new(),
            diagnostics: diags.items().iter().map(|d| d.render()).collect(),
            has_errors: true,
            warnings: Vec::new(),
            vendored: Vec::new(),
            link_flags: Vec::new(),
            line_map: Vec::new(),
        };
    }
    let out = c::emit_c(&program);
    CCompiled {
        code: out.code,
        diagnostics: diags.items().iter().map(|d| d.render()).collect(),
        has_errors: false,
        warnings: out.warnings,
        vendored: out.vendored,
        link_flags: out.link_flags,
        line_map: out.line_map,
    }
}

/// Where the vendored C support libraries live (cJSON, later the SQLite
/// amalgamation), bundled into a C project folder the way `vbrpy/` is for
/// Python: `$VBR_CSTDLIB_PATH`, else the compile-time default beside the crate.
pub fn cstdlib_path() -> std::path::PathBuf {
    std::env::var("VBR_CSTDLIB_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/csupport"))
        })
}

/// Where the `vbrpy` package lives (the Python stdlib, bundled into projects):
/// `$VBR_PYSTDLIB_PATH`, else the compile-time default beside the crate.
pub fn pystdlib_path() -> std::path::PathBuf {
    std::env::var("VBR_PYSTDLIB_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/vbrpy"))
        })
}
