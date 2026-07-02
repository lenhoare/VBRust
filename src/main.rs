//! VBR command-line driver.
//!
//!   vbr run <file.vbr>        transpile, compile with rustc, and run (single file,
//!                             no standard library or external crates)
//!   vbr runproject [path]     generate a cargo project in `build/` and run it
//!                             (handles the standard library and external crates)
//!   vbr build [path]          generate the cargo project without running it
//!   vbr transpile <file.vbr>  write the generated Rust to <file>.rs (or `-o file`)
//!   vbr emit <file.vbr>       print the generated Rust to stdout (or `-o file`)
//!
//! `path` for runproject/build is a `.vbr` entry file or a folder containing
//! `main.vbr`; it defaults to the current directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => cmd_run(&args[1..]),
        Some("runproject") => cmd_project(&args[1..], true),
        Some("build") => cmd_project(&args[1..], false),
        Some("transpile") => cmd_transpile(&args[1..]),
        Some("emit") => cmd_emit(&args[1..]),
        _ => {
            usage();
            exit(2);
        }
    }
}

fn usage() {
    eprintln!(
        "Usage:\n\
         \tvbr run <file.vbr>      compile with rustc and run (single file, no stdlib/crates)\n\
         \tvbr runproject [path]   generate a cargo project in build/ and run it\n\
         \tvbr build [path]        generate the cargo project without running\n\
         \tvbr transpile <file>    write the generated Rust to <file>.rs (or -o <file>)\n\
         \tvbr emit <file.vbr>     print the generated Rust (use -o <file> to write it)"
    );
}

/// Read a file, transpile it, print diagnostics, and bail on errors.
fn transpile(path: &Path) -> vbr::Compiled {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("✘ Could not read {}: {}", path.display(), e);
            exit(1);
        }
    };
    let result = vbr::compile(&source);
    for d in &result.diagnostics {
        eprintln!("{}", d);
    }
    if result.has_errors {
        eprintln!("\nTranspilation failed — no Rust was produced.");
        exit(1);
    }
    result
}

/// Does the generated Rust pull in the standard library (or, later, a crate)?
/// Such programs can't be linked by `rustc` alone — they need the project build.
fn needs_project(rust: &str) -> bool {
    rust.contains("vbr_stdlib")
}

fn cmd_transpile(args: &[String]) {
    let (input, output) = parse_emit_args(args);
    let result = transpile(&input);
    let out = output.unwrap_or_else(|| input.with_extension("rs"));
    if let Err(e) = fs::write(&out, &result.rust) {
        eprintln!("✘ Could not write {}: {}", out.display(), e);
        exit(1);
    }
    eprintln!("✔ Wrote {}", out.display());
}

fn cmd_emit(args: &[String]) {
    let (input, output) = parse_emit_args(args);
    let result = transpile(&input);
    match output {
        Some(out) => {
            if let Err(e) = fs::write(&out, &result.rust) {
                eprintln!("✘ Could not write {}: {}", out.display(), e);
                exit(1);
            }
            eprintln!("✔ Wrote {}", out.display());
        }
        None => print!("{}", result.rust),
    }
}

fn parse_emit_args(args: &[String]) -> (PathBuf, Option<PathBuf>) {
    let mut input = None;
    let mut output = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" | "--output" => output = it.next().map(PathBuf::from),
            _ => input = Some(PathBuf::from(a)),
        }
    }
    match input {
        Some(i) => (i, output),
        None => {
            eprintln!("Usage: vbr emit <file.vbr> [-o <file>]");
            exit(2);
        }
    }
}

fn cmd_run(args: &[String]) {
    let input = match args.first() {
        Some(a) => PathBuf::from(a),
        None => {
            eprintln!("Usage: vbr run <file.vbr>");
            exit(2);
        }
    };
    let result = transpile(&input);

    if needs_project(&result.rust) || !result.dependencies.is_empty() {
        eprintln!(
            "\n✘ This program uses the standard library (or an external crate via `Use`), \
             which needs the project build.\n  Run it with `vbr runproject` instead."
        );
        exit(1);
    }

    // Compile the single file with rustc in a temp dir (no littering).
    let tmp = std::env::temp_dir().join("vbr_run");
    let _ = fs::create_dir_all(&tmp);
    let rs = tmp.join("main.rs");
    let bin = tmp.join("main");
    if let Err(e) = fs::write(&rs, &result.rust) {
        eprintln!("✘ Could not write temp file: {}", e);
        exit(1);
    }

    eprintln!("→ rustc {}", input.display());
    let compiled = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(&rs)
        .arg("-o")
        .arg(&bin)
        .status();
    match compiled {
        Ok(s) if s.success() => {}
        Ok(_) => {
            eprintln!("✘ rustc rejected the generated Rust.");
            exit(1);
        }
        Err(e) => {
            eprintln!("✘ Could not run rustc: {}", e);
            exit(1);
        }
    }

    eprintln!("→ running {}\n", input.display());
    match Command::new(&bin).status() {
        Ok(s) => exit(s.code().unwrap_or(0)),
        Err(e) => {
            eprintln!("✘ Could not run the program: {}", e);
            exit(1);
        }
    }
}

fn cmd_project(args: &[String], run: bool) {
    let entry = match resolve_entry(args.first().map(String::as_str).unwrap_or(".")) {
        Some(e) => e,
        None => exit(1),
    };
    let build = generate_project(&entry);
    eprintln!("→ project: {}", build.display());

    if !run {
        return;
    }

    // Compiling Iced from scratch takes ~30s — and `build/` is shared across
    // examples, so a different example's deps can force a recompile even when
    // `target/` already exists. So whenever Iced is a dependency, give the
    // heads-up; otherwise a long compile looks like a hang. (On a cached rebuild
    // it's instant, and the note is harmless.)
    let cargo_toml = fs::read_to_string(build.join("Cargo.toml")).unwrap_or_default();
    if cargo_toml.contains("iced") {
        eprintln!(
            "→ Building the GUI — compiling Iced can take ~30s the first time \
             (instant once cached). The window opens when it finishes."
        );
    } else if cargo_toml.contains("ratatui") {
        eprintln!(
            "→ Building the TUI — compiling ratatui takes a few seconds the first time \
             (instant once cached). The app takes over the terminal when it starts."
        );
    }

    eprintln!("→ cargo run\n");
    match Command::new("cargo")
        .args(["run", "--quiet"])
        .current_dir(&build)
        .status()
    {
        Ok(s) => exit(s.code().unwrap_or(0)),
        Err(e) => {
            eprintln!("✘ Could not run cargo (is it installed?): {}", e);
            exit(1);
        }
    }
}

/// Resolve a path argument to the entry `.vbr` file.
fn resolve_entry(arg: &str) -> Option<PathBuf> {
    let p = PathBuf::from(arg);
    if p.is_dir() {
        let main = p.join("main.vbr");
        if main.is_file() {
            Some(main)
        } else {
            eprintln!("✘ No `main.vbr` found in {}", p.display());
            None
        }
    } else if p.is_file() {
        Some(p)
    } else {
        eprintln!("✘ {} is not a file or directory.", p.display());
        None
    }
}

/// Generate the cargo project under `<project>/build/` and return its path.
fn generate_project(entry: &Path) -> PathBuf {
    let project_dir = entry.parent().unwrap_or_else(|| Path::new("."));

    // A multi-module project is a folder whose entry is `main.vbr`; its siblings
    // are modules. A standalone file (e.g. `settings.vbr`) is a project of one —
    // we must NOT pull in unrelated neighbours (that would, say, try to compile
    // every other `.vbr` in `examples/`).
    let is_project = entry.file_name().and_then(|s| s.to_str()) == Some("main.vbr");

    // Discover sibling modules: every other `.vbr` file (transpiled), plus any
    // `.rs` file (included verbatim — a hand-written Rust module).
    let entry_canon = entry.canonicalize().ok();
    let mut vbr_files: Vec<PathBuf> = Vec::new();
    let mut rs_files: Vec<PathBuf> = Vec::new();
    if is_project {
        if let Ok(entries) = fs::read_dir(project_dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.canonicalize().ok() == entry_canon {
                    continue;
                }
                match p.extension().and_then(|s| s.to_str()) {
                    Some("vbr") => vbr_files.push(p),
                    // A stray `main.rs` would clobber the generated entry — skip it.
                    Some("rs") if stem_name(&p) != "main" => rs_files.push(p),
                    _ => {}
                }
            }
        }
    }
    vbr_files.sort();
    rs_files.sort();
    let vbr_names: Vec<String> = vbr_files.iter().map(|p| module_of(p)).collect();
    let rs_names: Vec<String> = rs_files.iter().map(|p| module_of(p)).collect();
    // Every sibling module is a possible qualified-call target and a `mod` decl.
    let module_names: Vec<String> = vbr_names.iter().chain(&rs_names).cloned().collect();

    let build = project_dir.join("build");
    let src = build.join("src");
    if let Err(e) = fs::create_dir_all(&src) {
        eprintln!("✘ Could not create {}: {}", src.display(), e);
        exit(1);
    }

    // Entry → main.rs (crate root: `mod` declarations + `fn main`).
    let entry_compiled = compile_path(entry, &module_names, true);
    if let Err(e) = fs::write(src.join("main.rs"), &entry_compiled.rust) {
        eprintln!("✘ Could not write main.rs: {}", e);
        exit(1);
    }
    let mut any_stdlib = needs_project(&entry_compiled.rust);
    // An async GUI (an event with `Await`) runs blocking work via tokio, so Iced
    // needs its `tokio` feature; an `Image` needs Iced's `image` feature.
    let async_gui = entry_compiled.rust.contains("spawn_blocking");
    let uses_image = entry_compiled.rust.contains("iced::widget::image(");
    let uses_canvas = entry_compiled.rust.contains("iced::widget::Canvas::new(");
    let mut deps: Vec<(String, String)> = entry_compiled.dependencies.clone();
    let mut stdlib_ns: Vec<String> = entry_compiled.stdlib_used.clone();

    // Each `.vbr` sibling → transpiled `<name>.rs`.
    for (file, name) in vbr_files.iter().zip(&vbr_names) {
        let compiled = compile_path(file, &module_names, false);
        let path = src.join(format!("{}.rs", name));
        if let Err(e) = fs::write(&path, &compiled.rust) {
            eprintln!("✘ Could not write {}: {}", path.display(), e);
            exit(1);
        }
        any_stdlib |= needs_project(&compiled.rust);
        deps.extend(compiled.dependencies);
        stdlib_ns.extend(compiled.stdlib_used);
    }

    // Each `.rs` sibling → copied verbatim as `<name>.rs`.
    for (file, name) in rs_files.iter().zip(&rs_names) {
        let content = match fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("✘ Could not read {}: {}", file.display(), e);
                exit(1);
            }
        };
        let path = src.join(format!("{}.rs", name));
        if let Err(e) = fs::write(&path, &content) {
            eprintln!("✘ Could not write {}: {}", path.display(), e);
            exit(1);
        }
        any_stdlib |= needs_project(&content);
        // A hand-written `.rs` module may use a stdlib namespace too; over-enabling
        // a feature is harmless, under-enabling breaks the build, so scan loosely.
        for (ns, _) in STDLIB_FEATURES {
            if content.contains(ns) {
                stdlib_ns.push(ns.to_string());
            }
        }
    }

    let mut cargo = format!(
        "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n",
        pkg_name(entry)
    );
    if any_stdlib {
        // Enable only the features the program uses (FileSystem needs none).
        let mut features: Vec<&str> = STDLIB_FEATURES
            .iter()
            .filter(|(ns, _)| stdlib_ns.iter().any(|u| u == ns))
            .map(|(_, feat)| *feat)
            .collect();
        features.sort();
        features.dedup();
        if features.is_empty() {
            cargo.push_str(&format!(
                "vbr_stdlib = {{ path = \"{}\", default-features = false }}\n",
                stdlib_path()
            ));
        } else {
            let list = features
                .iter()
                .map(|f| format!("\"{}\"", f))
                .collect::<Vec<_>>()
                .join(", ");
            cargo.push_str(&format!(
                "vbr_stdlib = {{ path = \"{}\", default-features = false, features = [{}] }}\n",
                stdlib_path(),
                list
            ));
        }
    }
    // `Use`'d crates, sorted and deduped by name, for stable output.
    deps.sort();
    deps.dedup_by(|a, b| a.0 == b.0);
    for (krate, version) in &deps {
        if krate == "iced" {
            // VBR GUIs render in software (tiny-skia) rather than wgpu: it builds
            // far faster and runs everywhere (WSL2, modest/no GPU) — the right
            // trade for a teaching tool, since forms don't need GPU acceleration.
            // An async GUI also needs `tokio` (blocking work via spawn_blocking);
            // an `Image` needs the `image` feature.
            let mut feats = vec!["\"tiny-skia\""];
            if async_gui {
                feats.push("\"tokio\"");
            }
            if uses_image {
                feats.push("\"image\"");
            }
            if uses_canvas {
                feats.push("\"canvas\"");
            }
            cargo.push_str(&format!(
                "iced = {{ version = \"{}\", default-features = false, features = [{}] }}\n",
                version,
                feats.join(", ")
            ));
        } else if krate == "pyo3" {
            // `auto-initialize` lets a standalone binary boot CPython on first use,
            // so the generated `Python::with_gil` "just works" without a manual
            // interpreter setup. It links libpython — a real Python must be present.
            cargo.push_str(&format!(
                "pyo3 = {{ version = \"{}\", features = [\"auto-initialize\"] }}\n",
                version
            ));
        } else {
            cargo.push_str(&format!("{} = \"{}\"\n", krate, version));
        }
    }
    // An async GUI calls `tokio::task::spawn_blocking` directly, so tokio must be a
    // direct dependency (Iced's `tokio` feature only links it transitively).
    if async_gui {
        cargo.push_str("tokio = { version = \"1\", features = [\"rt\"] }\n");
    }
    if let Err(e) = fs::write(build.join("Cargo.toml"), cargo) {
        eprintln!("✘ Could not write Cargo.toml: {}", e);
        exit(1);
    }

    build
}

/// Stdlib namespaces that map to a `vbr_stdlib` Cargo feature. `FileSystem` is
/// std-only and needs no feature, so it is intentionally absent.
const STDLIB_FEATURES: &[(&str, &str)] = &[
    ("Json", "json"),
    ("DateTime", "datetime"),
    ("Regex", "regex"),
    ("Http", "http"),
];

/// The raw file stem (`http.rs` → `http`), before snake-casing.
fn stem_name(p: &Path) -> String {
    p.file_stem().and_then(|s| s.to_str()).unwrap_or("module").to_string()
}

/// The Rust module name for a project file (`MyHelpers.vbr` → `my_helpers`).
fn module_of(p: &Path) -> String {
    vbr::module_name(&stem_name(p))
}

/// Read + compile one project file (as entry or module), printing diagnostics
/// and exiting on error.
fn compile_path(path: &Path, modules: &[String], is_entry: bool) -> vbr::Compiled {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("✘ Could not read {}: {}", path.display(), e);
            exit(1);
        }
    };
    let result = vbr::compile_module(&source, modules, is_entry);
    for d in &result.diagnostics {
        eprintln!("{}", d);
    }
    if result.has_errors {
        eprintln!(
            "\nTranspilation failed in {} — no Rust was produced.",
            path.display()
        );
        exit(1);
    }
    result
}

/// A valid cargo package name derived from the entry file stem.
fn pkg_name(entry: &Path) -> String {
    let stem = entry
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("vbr_app");
    let mut name: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    if name.is_empty() || name.chars().next().unwrap().is_ascii_digit() {
        name = format!("app_{}", name);
    }
    name
}

/// Where `vbr_stdlib` lives: `$VBR_STDLIB_PATH`, else the compile-time default.
///
/// Backslashes are normalised to forward slashes so the path is a valid TOML
/// basic string (TOML treats `\` as an escape) — Cargo accepts forward-slash
/// paths on Windows too, so this is portable.
fn stdlib_path() -> String {
    std::env::var("VBR_STDLIB_PATH")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/vbr_stdlib").to_string())
        .replace('\\', "/")
}
