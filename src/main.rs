//! VBR command-line driver.
//!
//!   vbr run <file.vbr>        transpile, compile with rustc, and run (single file,
//!                             no standard library or external crates)
//!   vbr runproject [path]     generate a cargo project in `build/` and run it
//!                             (handles the standard library and external crates)
//!   vbr runweb [path]         build a `Page` (or a `Screen`, via Ratzilla) for
//!                             WebAssembly and serve it in the browser with trunk
//!   vbr build [path]          generate the cargo project without running it
//!                             (`--web` generates the browser form)
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
        Some("runweb") => cmd_runweb(&args[1..]),
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
         \tvbr runweb [path]       build a Page or Screen for WebAssembly and serve it (trunk)\n\
         \tvbr build [path]        generate the cargo project without running (--web for the browser form)\n\
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

/// Is this generated Rust a browser program (a `Page`, or a `Screen` compiled
/// for the web)? Those build for WebAssembly via `vbr runweb`.
fn is_web_rust(rust: &str) -> bool {
    rust.contains("yew::Renderer::<") || rust.contains("ratzilla::")
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

    if is_web_rust(&result.rust) {
        eprintln!(
            "\n✘ This program has a `Page`, so it compiles to a web app.\n  \
             Run it with `vbr runweb` instead."
        );
        exit(1);
    }
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
        .args(["--edition", "2021", "--error-format", "json"])
        .arg(&rs)
        .arg("-o")
        .arg(&bin)
        .output();
    match compiled {
        Ok(o) if o.status.success() => {}
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            let errors = parse_rustc_json(stderr.lines());
            report_errors(&errors, |_| Some((input.clone(), result.line_map.clone())));
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
    // `vbr build --web <file>` generates the browser form of a Screen program
    // (what `vbr runweb` builds) without serving it.
    let web = args.iter().any(|a| a == "--web");
    let path_arg =
        args.iter().find(|a| !a.starts_with("--")).map(String::as_str).unwrap_or(".");
    let entry = match resolve_entry(path_arg) {
        Some(e) => e,
        None => exit(1),
    };
    if web && run {
        eprintln!("✘ `--web` builds a browser app — serve it with `vbr runweb` instead.");
        exit(1);
    }
    let (build, file_maps) = generate_project(&entry, web);
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
    if cargo_toml.contains("yew") {
        eprintln!(
            "✘ This program has a `Page`, so it compiles to a web app.\n  \
             Run it with `vbr runweb` instead."
        );
        exit(1);
    }
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
    } else if cargo_toml.contains("dataframe") {
        eprintln!(
            "→ Building with dataframes — compiling polars takes a minute or so the \
             first time (instant once cached)."
        );
    }

    // Build first with JSON diagnostics, so a failure can be translated back
    // to .vbr lines; the run afterwards reuses the cached build instantly.
    let built = Command::new("cargo")
        .args(["build", "--message-format", "json", "--quiet"])
        .current_dir(&build)
        .output();
    match built {
        Ok(o) if o.status.success() => {}
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let errors = parse_cargo_json(&stdout);
            report_errors(&errors, |e| {
                // Match the error's file ("src/main.rs") to the .vbr it came from.
                let name = e.file.as_deref()?;
                file_maps
                    .iter()
                    .find(|m| name.ends_with(&m.rs_name))
                    .map(|m| (m.source.clone(), m.map.clone()))
            });
            exit(1);
        }
        Err(e) => {
            eprintln!("✘ Could not run cargo (is it installed?): {}", e);
            exit(1);
        }
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

/// `vbr runweb`: generate the project, build it for WebAssembly (translating
/// errors back to `.vbr` lines), and serve it in the browser with trunk.
fn cmd_runweb(args: &[String]) {
    let entry = match resolve_entry(args.first().map(String::as_str).unwrap_or(".")) {
        Some(e) => e,
        None => exit(1),
    };
    let (build, file_maps) = generate_project(&entry, true);
    eprintln!("→ project: {}", build.display());

    let cargo_toml = fs::read_to_string(build.join("Cargo.toml")).unwrap_or_default();
    if !cargo_toml.contains("yew") && !cargo_toml.contains("ratzilla") {
        eprintln!(
            "✘ Nothing here runs in a browser — `runweb` serves a `Page` (a web app) or a \
             `Screen` (a terminal app drawn in the browser).\n  \
             Run this with `vbr run` or `vbr runproject` instead."
        );
        exit(1);
    }

    // One-time toolchain setup, checked up front so the failure is friendly.
    // (No rustup — a distro toolchain — means we can't check; let cargo report.)
    if let Ok(o) = Command::new("rustup").args(["target", "list", "--installed"]).output() {
        let installed = String::from_utf8_lossy(&o.stdout);
        if !installed.lines().any(|l| l.trim() == "wasm32-unknown-unknown") {
            eprintln!(
                "✘ The web build needs Rust's WebAssembly target. Install it once with:\n\n    \
                 rustup target add wasm32-unknown-unknown\n\nthen re-run `vbr runweb`."
            );
            exit(1);
        }
    }
    if Command::new("trunk").arg("--version").output().is_err() {
        eprintln!(
            "✘ The web build needs trunk (the WebAssembly bundler and dev server). \
             Install it once with:\n\n    cargo install trunk --locked\n\n\
             then re-run `vbr runweb`."
        );
        exit(1);
    }

    if cargo_toml.contains("ratzilla") {
        eprintln!(
            "→ Building the web terminal — compiling Ratzilla for WebAssembly takes a \
             minute the first time (instant once cached)."
        );
    } else {
        eprintln!(
            "→ Building the web app — compiling Yew for WebAssembly takes a minute the \
             first time (instant once cached)."
        );
    }
    // Build first with JSON diagnostics, so a failure can be translated back to
    // .vbr lines; trunk then reuses the cached build.
    let built = Command::new("cargo")
        .args([
            "build", "--target", "wasm32-unknown-unknown", "--message-format", "json", "--quiet",
        ])
        .current_dir(&build)
        .output();
    match built {
        Ok(o) if o.status.success() => {}
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let errors = parse_cargo_json(&stdout);
            report_errors(&errors, |e| {
                let name = e.file.as_deref()?;
                file_maps
                    .iter()
                    .find(|m| name.ends_with(&m.rs_name))
                    .map(|m| (m.source.clone(), m.map.clone()))
            });
            exit(1);
        }
        Err(e) => {
            eprintln!("✘ Could not run cargo (is it installed?): {}", e);
            exit(1);
        }
    }

    eprintln!("→ trunk serve — opening the browser (Ctrl+C to stop)\n");
    match Command::new("trunk")
        .args(["serve", "--open"])
        .current_dir(&build)
        .status()
    {
        Ok(s) => exit(s.code().unwrap_or(0)),
        Err(e) => {
            eprintln!("✘ Could not run trunk: {}", e);
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

/// Translation info for one generated file: its path under the build dir, the
/// `.vbr` it came from, and the (rust line → vbr line) map.
struct FileMap {
    rs_name: String,
    source: PathBuf,
    map: Vec<(usize, usize)>,
}

/// Generate the cargo project under `<project>/build/` and return its path
/// plus the per-file line maps (for translating build errors).
fn generate_project(entry: &Path, web: bool) -> (PathBuf, Vec<FileMap>) {
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
    let mut file_maps: Vec<FileMap> = Vec::new();
    let entry_compiled = compile_path(entry, &module_names, true, web);
    if let Err(e) = fs::write(src.join("main.rs"), &entry_compiled.rust) {
        eprintln!("✘ Could not write main.rs: {}", e);
        exit(1);
    }
    file_maps.push(FileMap {
        rs_name: "src/main.rs".to_string(),
        source: entry.to_path_buf(),
        map: entry_compiled.line_map.clone(),
    });
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
        let compiled = compile_path(file, &module_names, false, web);
        let path = src.join(format!("{}.rs", name));
        if let Err(e) = fs::write(&path, &compiled.rust) {
            eprintln!("✘ Could not write {}: {}", path.display(), e);
            exit(1);
        }
        file_maps.push(FileMap {
            rs_name: format!("src/{}.rs", name),
            source: file.clone(),
            map: compiled.line_map.clone(),
        });
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
        } else if krate == "yew" {
            // A `Page` renders client-side in the browser (`csr`); the project is
            // built for wasm32 and served by trunk (`vbr runweb`).
            cargo.push_str(&format!(
                "yew = {{ version = \"{}\", features = [\"csr\"] }}\n",
                version
            ));
        } else if krate == "ratatui" && version == "0.30" {
            // ratatui 0.30 is the web (Ratzilla) pairing — its default features
            // pull the crossterm backend, which can't compile for wasm. The
            // widgets/layout the generated `view` uses need no feature.
            cargo.push_str("ratatui = { version = \"0.30\", default-features = false }\n");
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
    // A web input reads its DOM element (`web_sys::HtmlInputElement`) to get the
    // typed text / checked state, so web-sys must be a direct dependency.
    if entry_compiled.rust.contains("web_sys::HtmlInputElement") {
        cargo.push_str("web-sys = { version = \"0.3\", features = [\"HtmlInputElement\"] }\n");
    }
    // An awaited `Http.Get` in a Page runs on the browser's fetch via gloo-net
    // (the generated `http_get` wrapper) — only its `http` feature is needed.
    if entry_compiled.rust.contains("gloo_net::") {
        cargo.push_str(
            "gloo-net = { version = \"0.6\", default-features = false, features = [\"http\"] }\n",
        );
    }
    // An `Every` timer in a browser Screen runs on a gloo-timers Interval.
    if entry_compiled.rust.contains("gloo_timers::") {
        cargo.push_str("gloo-timers = \"0.3\"\n");
    }
    // A browser Screen's async continuation is spawned with wasm-bindgen-futures.
    if entry_compiled.rust.contains("wasm_bindgen_futures::") {
        cargo.push_str("wasm-bindgen-futures = \"0.4\"\n");
    }
    if let Err(e) = fs::write(build.join("Cargo.toml"), cargo) {
        eprintln!("✘ Could not write Cargo.toml: {}", e);
        exit(1);
    }

    // A web project also gets the `index.html` trunk serves — the page's (or
    // screen's) `Title` becomes the browser-tab title. A Screen's page styles
    // the terminal: Ratzilla's DOM backend renders it as <pre> text, so it
    // gets a monospace font, centered on a dark page.
    if is_web_rust(&entry_compiled.rust) {
        let title = entry_compiled
            .web_title
            .clone()
            .unwrap_or_else(|| "VBR app".to_string());
        let html = if entry_compiled.rust.contains("ratzilla::") {
            format!(
                "<!DOCTYPE html>\n<html>\n  <head>\n    <meta charset=\"utf-8\" />\n    \
                 <title>{}</title>\n    <style>\n      body {{\n        margin: 0;\n        \
                 width: 100%;\n        height: 100vh;\n        display: flex;\n        \
                 flex-direction: column;\n        justify-content: center;\n        \
                 align-items: center;\n        background-color: #121212;\n      }}\n      \
                 pre {{\n        font-family: monospace;\n        font-size: 16px;\n        \
                 margin: 0px;\n      }}\n    </style>\n  </head>\n  <body></body>\n</html>\n",
                title
            )
        } else {
            // The page's stylesheet: its Theme's palette + any Css blocks. The
            // asset links make trunk copy local Image files into the site.
            let style = match &entry_compiled.web_style {
                Some(css) => {
                    let indented: String =
                        css.lines().map(|l| format!("      {}\n", l)).collect();
                    format!("    <style>\n{}    </style>\n", indented)
                }
                None => String::new(),
            };
            let assets: String = entry_compiled
                .web_assets
                .iter()
                .map(|a| format!("    <link data-trunk rel=\"copy-file\" href=\"../{}\" />\n", a))
                .collect();
            format!(
                "<!DOCTYPE html>\n<html>\n  <head>\n    <meta charset=\"utf-8\" />\n    \
                 <title>{}</title>\n{}{}  </head>\n  <body></body>\n</html>\n",
                title, assets, style
            )
        };
        if let Err(e) = fs::write(build.join("index.html"), html) {
            eprintln!("✘ Could not write index.html: {}", e);
            exit(1);
        }
    }

    (build, file_maps)
}

/// Stdlib namespaces that map to a `vbr_stdlib` Cargo feature. `FileSystem` is
/// std-only and needs no feature, so it is intentionally absent.
const STDLIB_FEATURES: &[(&str, &str)] = &[
    ("Json", "json"),
    ("DateTime", "datetime"),
    ("Regex", "regex"),
    ("Http", "http"),
    ("DataFrame", "dataframe"),
];

/// The raw file stem (`http.rs` → `http`), before lowercasing.
fn stem_name(p: &Path) -> String {
    p.file_stem().and_then(|s| s.to_str()).unwrap_or("module").to_string()
}

/// The Rust module name for a project file (`MyHelpers.vbr` → `my_helpers`).
fn module_of(p: &Path) -> String {
    vbr::module_name(&stem_name(p))
}

/// Read + compile one project file (as entry or module), printing diagnostics
/// and exiting on error.
fn compile_path(path: &Path, modules: &[String], is_entry: bool, web: bool) -> vbr::Compiled {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("✘ Could not read {}: {}", path.display(), e);
            exit(1);
        }
    };
    let result = if web {
        vbr::compile_module_web(&source, modules, is_entry)
    } else {
        vbr::compile_module(&source, modules, is_entry)
    };
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

// ── Translating rustc errors back to .vbr lines ─────────────────────────────
//
// The transpiler records (generated-Rust line → VBR line) checkpoints as it
// emits. rustc runs with `--error-format=json`; each error's primary span is
// mapped through the checkpoints back to the .vbr source, quoted, and — for
// the classic Rust stumbling blocks — given a teaching hint. The raw rustc
// output (against the generated Rust) is available with VBR_RUSTC_RAW=1.

/// One rustc diagnostic, reduced to what the translation needs.
struct RustcError {
    message: String,
    code: Option<String>,
    /// Primary-span file (cargo mode; a bare `rustc` run has only one file).
    file: Option<String>,
    /// Primary-span 1-based line in the generated Rust.
    line: Option<usize>,
    label: Option<String>,
    /// rustc's own pretty rendering — the fallback when we can't map.
    rendered: String,
}

/// Parse `rustc --error-format=json` output (one JSON object per line).
fn parse_rustc_json<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<RustcError> {
    lines
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| error_from_json(&v))
        .collect()
}

/// Parse `cargo build --message-format=json` output: the rustc diagnostic is
/// nested inside each `compiler-message`.
fn parse_cargo_json(stdout: &str) -> Vec<RustcError> {
    stdout
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v["reason"] == "compiler-message")
        .filter_map(|v| error_from_json(&v["message"]))
        .collect()
}

fn error_from_json(v: &serde_json::Value) -> Option<RustcError> {
    if v["level"].as_str() != Some("error") {
        return None;
    }
    let message = v["message"].as_str()?.to_string();
    // The trailing summary ("aborting due to N errors") carries no span.
    if message.starts_with("aborting due to") {
        return None;
    }
    let primary = v["spans"]
        .as_array()
        .and_then(|s| s.iter().find(|sp| sp["is_primary"].as_bool() == Some(true)));
    Some(RustcError {
        code: v["code"]["code"].as_str().map(String::from),
        file: primary.and_then(|sp| sp["file_name"].as_str()).map(String::from),
        line: primary
            .and_then(|sp| sp["line_start"].as_u64())
            .map(|n| n as usize),
        label: primary
            .and_then(|sp| sp["label"].as_str())
            .map(String::from),
        rendered: v["rendered"].as_str().unwrap_or("").to_string(),
        message,
    })
}

/// The VBR line a generated-Rust line came from: the last checkpoint at or
/// before it (checkpoints are recorded in ascending emission order).
fn vbr_line_for(map: &[(usize, usize)], rust_line: usize) -> Option<usize> {
    map.iter()
        .take_while(|(r, _)| *r <= rust_line)
        .last()
        .map(|(_, v)| *v)
}

/// A hint for the Rust errors a VB programmer meets first. Deliberately short —
/// the goal is orientation, not a lecture.
fn teaching_hint(code: &str) -> Option<&'static str> {
    Some(match code {
        "E0308" => {
            "Rust never converts between types silently — check the declared `As` type \
             against what the right-hand side actually produces."
        }
        "E0382" => {
            "The value was *moved*: a String/struct/Vec has one owner, and ownership \
             changed hands earlier. Use `Set` to borrow it instead, or `.clone()` for a \
             real (costed) copy."
        }
        "E0502" | "E0499" => {
            "Two borrows clash: a value may have many readers or one writer, never both \
             at once. Finish using the borrow (`Set`) before changing the original."
        }
        "E0425" => {
            "Rust can't find that name. Inside `Rust … End Rust` blocks and `Match` \
             patterns you're writing real Rust, so use the lowercase spelling — VBR's \
             `myTotal` is `mytotal` there."
        }
        "E0599" => {
            "No method with that name on this type. Method calls pass straight through \
             to Rust — check the name against Rust's String/Vec docs (VBR lowercases it)."
        }
        _ => return None,
    })
}

/// Print translated errors. `locate` finds the (.vbr path, line map) for an
/// error; anything it can't place falls back to rustc's own rendering.
fn report_errors(errors: &[RustcError], locate: impl Fn(&RustcError) -> Option<(PathBuf, Vec<(usize, usize)>)>) {
    if errors.is_empty() {
        eprintln!("✘ rustc rejected the generated Rust (and produced no diagnostics VBR could read).");
        return;
    }
    if std::env::var_os("VBR_RUSTC_RAW").is_some() {
        for e in errors {
            eprint!("{}", e.rendered);
        }
        return;
    }
    // Source files, read once each for quoting.
    let mut sources: std::collections::HashMap<PathBuf, String> = std::collections::HashMap::new();
    let mut any_mapped = false;
    for e in errors {
        let located = locate(e).and_then(|(path, map)| {
            let vl = e.line.and_then(|l| vbr_line_for(&map, l))?;
            Some((path, vl))
        });
        match located {
            Some((path, vl)) => {
                any_mapped = true;
                eprintln!("✘ [line {}] {}", vl, e.message);
                let src = sources
                    .entry(path.clone())
                    .or_insert_with(|| fs::read_to_string(&path).unwrap_or_default());
                if let Some(text) = src.lines().nth(vl.saturating_sub(1)) {
                    eprintln!("  {:>4} | {}", vl, text.trim_end());
                }
                if let Some(label) = &e.label {
                    eprintln!("       ({})", label);
                }
                if let Some(hint) = e.code.as_deref().and_then(teaching_hint) {
                    eprintln!("  ℹ {}", hint);
                }
                eprintln!();
            }
            None => eprint!("{}", e.rendered),
        }
    }
    if any_mapped {
        eprintln!("✘ The generated Rust didn't compile — the errors above point at your .vbr lines.");
        eprintln!("  (Set VBR_RUSTC_RAW=1 to see rustc's original output against the generated Rust.)");
    }
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
