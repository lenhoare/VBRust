//! Bust command-line driver.
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
        Some("debugbuild") => cmd_debugbuild(&args[1..]),
        Some("runproject") => cmd_project(&args[1..], true),
        Some("runweb") => cmd_runweb(&args[1..]),
        Some("rungodot") => cmd_rungodot(&args[1..]),
        Some("build") => cmd_project(&args[1..], false),
        Some("test") => cmd_test(&args[1..]),
        Some("transpile") => cmd_transpile(&args[1..]),
        Some("emit") => cmd_emit(&args[1..]),
        Some("embed") => cmd_embed(&args[1..]),
        Some("py") => cmd_py(&args[1..]),
        Some("c") => cmd_c(&args[1..]),
        Some("graduate") => cmd_graduate(&args[1..]),
        Some("help") => cmd_help(&args[1..]),
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
         \tvbr debugbuild <file>   compile a debuggable binary (symbols) to .vbrdebug/ for VS Code + CodeLLDB\n\
         \tvbr runproject [path]   generate a cargo project in build/ and run it\n\
         \tvbr runweb [path]       build a Page or Screen for WebAssembly and serve it (trunk)\n\
         \tvbr rungodot <file.vbr> build a Node2D program as a Godot GDExtension and open it in Godot 4\n\
         \tvbr build [path]        generate the cargo project without running (--web for the browser form)\n\
         \tvbr test [path]         run the program's `Test` blocks and report ✓ / ✗\n\
         \tvbr transpile <file>    write the generated Rust to <file>.rs (or -o <file>)\n\
         \tvbr emit <file.vbr>     print the generated Rust (use -o <file> to write it)\n\
         \tvbr embed [--check] <file.rs>  expand Bust in `/* vbr … */` block comments in place (--check: verify only, for CI)\n\
         \tvbr py <file.vbr>       transpile to Python (core language; -o <file> to write it)\n\
         \tvbr c <file.vbr>        transpile to C (core language; -o <file> to write it)\n\
         \tvbr graduate <file.vbr> replace a module with the Rust it became — permanently.\n\
         \t                        The project keeps building; you maintain that file in Rust\n\
         \t                        from now on. Graduate main.vbr last to finish the journey.\n\
         \tvbr help build [dir]    generate the offline help site + text skin from help/entries/\n\
         \t                        into help/build/ (dir overrides the entries folder)."
    );
}

/// `vbr help build [entries_dir]` — regenerate the offline help from
/// `help/entries/` into `help/build/`. Fails if any example doesn't transpile.
fn cmd_help(args: &[String]) {
    let rest: &[String] = match args.first().map(String::as_str) {
        Some("build") => &args[1..],
        _ => {
            eprintln!("Usage: vbr help build [entries_dir]");
            exit(2);
        }
    };
    let entries = rest
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("help/entries"));
    let out = PathBuf::from("help/build");
    match vbr::help::build(&entries, &out) {
        Ok(report) => {
            for (id, err) in &report.failures {
                eprintln!("✘ {}: {}", id, err);
            }
            println!(
                "Help: wrote {} pages → {} ({} of {} topics + {} member pages, {} stub{}).",
                report.written,
                out.display(),
                report.covered,
                report.total,
                report.members,
                report.stubs.len(),
                if report.stubs.len() == 1 { "" } else { "s" }
            );
            if !report.failures.is_empty() {
                eprintln!(
                    "\n{} example(s) failed to transpile — help not fully valid.",
                    report.failures.len()
                );
                exit(1);
            }
        }
        Err(e) => {
            eprintln!("✘ {}", e);
            exit(1);
        }
    }
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
/// Does the generated Rust define a Godot GDExtension (a `Node2D` program)?
fn is_godot_rust(rust: &str) -> bool {
    rust.contains("impl ExtensionLibrary for")
}
/// The first node class in a Godot program: `(base, name)` pulled from the
/// generated `#[class(base = <Base>)]` / `struct <Name>`. Slice 1 has one node.
fn godot_class_info(rust: &str) -> Option<(String, String)> {
    let base = rust
        .lines()
        .find_map(|l| l.trim().strip_prefix("#[class(base = ")?.strip_suffix(")]"))
        .map(str::to_string)?;
    let name = rust
        .lines()
        .find_map(|l| l.trim().strip_prefix("struct ")?.strip_suffix(" {"))
        .map(str::to_string)?;
    Some((base, name))
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

/// `vbr embed <file.rs>` — Bust embedded in Rust. Bust written inside a `/* vbr …
/// */` block comment is transpiled and the resulting Rust written into a managed
/// `// vbr:gen … // vbr:gen-end` region right after, indented to match the `/*
/// vbr` marker. Re-running overwrites that region, so it's idempotent; the `.rs`
/// always compiles (the Bust stays a comment). The Rust the fragment becomes is
/// spliced in as plain statements — call Rust functions, leave values in scope,
/// all in one flat function body.
///
/// Caveat: block comments end at the first `*/`, so embedded Bust can't contain a
/// literal `*/` (only realistic inside a string — split it, e.g. `"a*" & "/b"`).
fn cmd_embed(args: &[String]) {
    // `--check` verifies without writing — for a pre-commit hook or CI to catch a
    // stale generated region (Bust edited but `vbr embed` not re-run).
    let mut check = false;
    let mut file: Option<PathBuf> = None;
    for a in args {
        if a == "--check" {
            check = true;
        } else {
            file = Some(PathBuf::from(a));
        }
    }
    let input = match file {
        Some(p) => p,
        None => {
            eprintln!("Usage: vbr embed [--check] <file.rs>");
            exit(2);
        }
    };
    let text = match fs::read_to_string(&input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("✘ Could not read {}: {}", input.display(), e);
            exit(1);
        }
    };

    let (result, expanded, errors) = expand_embedded(&text);

    if check {
        if !errors.is_empty() {
            eprintln!("✘ {}: embedded Bust has errors:", input.display());
            for e in &errors {
                eprintln!("    {}", e);
            }
            exit(1);
        }
        if result != text {
            eprintln!(
                "✘ {}: the generated Rust is out of date — run `vbr embed {}`.",
                input.display(),
                input.display()
            );
            exit(1);
        }
        eprintln!("✔ {}: embedded Bust is up to date.", input.display());
        return;
    }

    if let Err(e) = fs::write(&input, &result) {
        eprintln!("✘ Could not write {}: {}", input.display(), e);
        exit(1);
    }
    eprintln!(
        "✔ Expanded {} Bust block{} in {}",
        expanded,
        if expanded == 1 { "" } else { "s" },
        input.display()
    );
    if !errors.is_empty() {
        eprintln!("⚠ Some blocks had errors — their diagnostics were written in place.");
        exit(1);
    }
}

/// Expand every `/* vbr … */` block in `text`, returning the rewritten source,
/// the number of blocks expanded cleanly, and any error messages. Pure (no I/O)
/// so `--check` can compare against the file without touching it. The original
/// line ending (LF vs CRLF) is preserved, so a check is byte-exact on Windows.
fn expand_embedded(text: &str) -> (String, usize, Vec<String>) {
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let is = |line: &str, marker: &str| line.trim() == marker;
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    let mut expanded = 0usize;
    let mut errors: Vec<String> = Vec::new();

    while i < lines.len() {
        let line = lines[i];
        // An opener is `/* vbr` (optionally with Bust trailing on the same line).
        let opener = line.trim_start().strip_prefix("/* vbr").filter(|rest| {
            rest.is_empty() || rest.starts_with(char::is_whitespace) || rest.starts_with("*/")
        });
        let Some(first_rest) = opener else {
            out.push(line.to_string());
            i += 1;
            continue;
        };

        // The indent of the `/* vbr` marker — the generated Rust aligns to it.
        let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
        out.push(line.to_string()); // keep the opener line verbatim
        i += 1;

        // Collect the Bust (verbatim — no per-line prefix to strip), up to `*/`.
        let mut vbr: Vec<String> = Vec::new();
        let mut unterminated = false;
        if let Some(idx) = first_rest.find("*/") {
            // Whole block on the opener line: `/* vbr … */`.
            let content = first_rest[..idx].trim();
            if !content.is_empty() {
                vbr.push(content.to_string());
            }
        } else {
            let fr = first_rest.trim();
            if !fr.is_empty() {
                vbr.push(fr.to_string());
            }
            loop {
                if i >= lines.len() {
                    unterminated = true;
                    break;
                }
                let l = lines[i];
                out.push(l.to_string()); // keep the Bust/closer line verbatim
                i += 1;
                if let Some(idx) = l.find("*/") {
                    let before = l[..idx].trim();
                    if !before.is_empty() {
                        vbr.push(before.to_string());
                    }
                    break;
                }
                vbr.push(l.to_string());
            }
        }

        if unterminated {
            let msg = "✘ `/* vbr` without a closing `*/`.".to_string();
            out.push(format!("{}// {}", indent, msg));
            errors.push(msg);
            continue;
        }

        // Transpile the fragment and emit the managed region.
        let frag = vbr::compile_fragment(&vbr.join("\n"));
        out.push(format!(
            "{}// vbr:gen (generated by `vbr embed` — do not edit)",
            indent
        ));
        if frag.has_errors {
            for d in &frag.diagnostics {
                errors.push(d.clone());
                for dl in d.lines() {
                    out.push(format!("{}// {}", indent, dl));
                }
            }
        } else {
            for l in frag.rust.lines() {
                if l.is_empty() {
                    out.push(String::new());
                } else {
                    out.push(format!("{}{}", indent, l));
                }
            }
            expanded += 1;
        }
        out.push(format!("{}// vbr:gen-end", indent));

        // Drop any previous generated region that followed, so re-runs replace it.
        if i < lines.len()
            && lines[i].trim().starts_with("// vbr:gen")
            && !is(lines[i], "// vbr:gen-end")
        {
            i += 1;
            while i < lines.len() && !is(lines[i], "// vbr:gen-end") {
                i += 1;
            }
            if i < lines.len() {
                i += 1; // the old `// vbr:gen-end`
            }
        }
    }

    let mut result = out.join(nl);
    if text.ends_with('\n') {
        result.push_str(nl);
    }
    (result, expanded, errors)
}

/// `vbr py <file.vbr>` — transpile to Python. A core-language program prints (or
/// writes with `-o`); a standard-library program becomes a *project* folder
/// (`main.py` + the bundled `vbrpy` package), the parallel of `vbr runproject`.
/// Warnings go to stderr, so a redirected `stdout` stays clean Python.
fn cmd_py(args: &[String]) {
    let (input, output) = parse_emit_args(args);
    let source = match fs::read_to_string(&input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("✘ Could not read {}: {}", input.display(), e);
            exit(1);
        }
    };
    let result = vbr::compile_python(&source);
    for d in &result.diagnostics {
        eprintln!("{}", d);
    }
    if result.has_errors {
        eprintln!("\nTranspilation failed — no Python was produced.");
        exit(1);
    }
    for w in &result.warnings {
        eprintln!("{}", w);
    }

    // A program is a *project* (a folder, not one file) when it uses the stdlib
    // (needs the `vbrpy` package) OR declares pip deps via `Use` (needs a
    // `requirements.txt`). The stdlib brings `vbrpy/`; `Use` brings requirements.
    if !result.stdlib_used.is_empty() || !result.requirements.is_empty() {
        let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("app");
        let dir = output.unwrap_or_else(|| {
            input.parent().unwrap_or_else(|| Path::new(".")).join(format!("{}_py", stem))
        });
        if let Err(e) = fs::create_dir_all(&dir) {
            eprintln!("✘ Could not create {}: {}", dir.display(), e);
            exit(1);
        }
        if let Err(e) = fs::write(dir.join("main.py"), &result.code) {
            eprintln!("✘ Could not write main.py: {}", e);
            exit(1);
        }
        // The stdlib package is copied only when actually used — a pure-`Use`
        // program (e.g. `import numpy`) needs no `vbrpy/`.
        if !result.stdlib_used.is_empty() {
            copy_dir_recursive(&vbr::pystdlib_path(), &dir.join("vbrpy"));
        }
        // pip deps → a `requirements.txt`, the parallel of Cargo `[dependencies]`.
        if !result.requirements.is_empty() {
            let reqs = format!("{}\n", result.requirements.join("\n"));
            if let Err(e) = fs::write(dir.join("requirements.txt"), reqs) {
                eprintln!("✘ Could not write requirements.txt: {}", e);
                exit(1);
            }
        }
        let uses = if result.stdlib_used.is_empty() {
            String::new()
        } else {
            format!(" (uses {})", result.stdlib_used.join(", "))
        };
        let pip = if result.requirements.is_empty() {
            String::new()
        } else {
            "pip install -r requirements.txt && ".to_string()
        };
        eprintln!(
            "✔ Wrote {}{} — run it with:\n    cd {} && {}python3 main.py",
            dir.join("main.py").display(),
            uses,
            dir.display(),
            pip,
        );
        return;
    }

    match output {
        Some(out) => {
            if let Err(e) = fs::write(&out, &result.code) {
                eprintln!("✘ Could not write {}: {}", out.display(), e);
                exit(1);
            }
            eprintln!("✔ Wrote {}", out.display());
        }
        None => print!("{}", result.code),
    }
}

/// `vbr c <file.vbr> [-o out.c]` — transpile to C. A single self-contained `.c`;
/// compile it with any C compiler (`cc out.c -lm && ./a.out`).
fn cmd_c(args: &[String]) {
    let (input, output) = parse_emit_args(args);
    let source = match fs::read_to_string(&input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("✘ Could not read {}: {}", input.display(), e);
            exit(1);
        }
    };
    let result = vbr::compile_c(&source);
    for d in &result.diagnostics {
        eprintln!("{}", d);
    }
    if result.has_errors {
        eprintln!("\nTranspilation failed — no C was produced.");
        exit(1);
    }
    for w in &result.warnings {
        eprintln!("{}", w);
    }

    // A program that vendors a C library becomes a *project* folder (`main.c` +
    // the bundled sources + a `Makefile`) — the parallel of `vbr py`'s `vbrpy/`
    // mode. A self-contained program stays a single `.c`.
    if result.is_project() {
        let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("app");
        let dir = output.unwrap_or_else(|| {
            input.parent().unwrap_or_else(|| Path::new(".")).join(format!("{}_c", stem))
        });
        if let Err(e) = fs::create_dir_all(&dir) {
            eprintln!("✘ Could not create {}: {}", dir.display(), e);
            exit(1);
        }
        if let Err(e) = fs::write(dir.join("main.c"), &result.code) {
            eprintln!("✘ Could not write main.c: {}", e);
            exit(1);
        }
        // Copy each vendored library's `.c`/`.h` pair from `csupport/`.
        let src = vbr::cstdlib_path();
        for base in &result.vendored {
            for ext in ["c", "h"] {
                let name = format!("{}.{}", base, ext);
                if let Err(e) = fs::copy(src.join(&name), dir.join(&name)) {
                    eprintln!("✘ Could not copy {}: {}", name, e);
                    exit(1);
                }
            }
        }
        if let Err(e) = fs::write(dir.join("Makefile"), c_makefile(&result)) {
            eprintln!("✘ Could not write Makefile: {}", e);
            exit(1);
        }
        // Describe the dependencies: vendored sources and/or linked libraries.
        let mut deps = Vec::new();
        if !result.vendored.is_empty() {
            deps.push(format!("vendors {}", result.vendored.join(", ")));
        }
        let links: Vec<String> =
            result.link_flags.iter().filter(|f| *f != "m").map(|f| format!("-l{}", f)).collect();
        if !links.is_empty() {
            deps.push(format!("links {}", links.join(" ")));
        }
        let deps = if deps.is_empty() { String::new() } else { format!(" ({})", deps.join(", ")) };
        eprintln!(
            "✔ Wrote {}{} — build it with:\n    cd {} && make && ./main",
            dir.join("main.c").display(),
            deps,
            dir.display(),
        );
        return;
    }

    match output {
        Some(out) => {
            if let Err(e) = fs::write(&out, &result.code) {
                eprintln!("✘ Could not write {}: {}", out.display(), e);
                exit(1);
            }
            eprintln!("✔ Wrote {} — build it with:\n    cc {} -lm && ./a.out", out.display(), out.display());
        }
        None => print!("{}", result.code),
    }
}

/// The `Makefile` for a C project folder: compile `main.c` plus every vendored
/// `.c`, with each `link_flag` as a `-l`. A tab indents the recipe (make's rule).
fn c_makefile(result: &vbr::CCompiled) -> String {
    let mut srcs = String::from("main.c");
    for base in &result.vendored {
        srcs.push_str(&format!(" {}.c", base));
    }
    let libs: String =
        result.link_flags.iter().map(|f| format!(" -l{}", f)).collect();
    format!(
        "# Generated by `vbr c` — build with `make`, then run `./main`.\n\
         CC ?= cc\n\
         CFLAGS ?= -O2\n\n\
         main: {srcs}\n\
         \t$(CC) $(CFLAGS) {srcs}{libs} -o main\n\n\
         clean:\n\
         \trm -f main\n"
    )
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

/// `vbr debugbuild <file.vbr>` — compile a *debuggable* binary (with DWARF debug
/// info) so VS Code + CodeLLDB can step through the generated Rust. Unlike
/// `run`, it persists the artifacts next to the source in `.vbrdebug/`: the
/// generated `<stem>.rs` (so the debugger's line info points at readable Rust)
/// and the `<stem>` binary. It does not run the program — the debugger does.
/// The binary's path is printed to stdout (the one clean line) for tooling; the
/// human-readable summary goes to stderr.
fn cmd_debugbuild(args: &[String]) {
    let input = match args.first() {
        Some(a) => PathBuf::from(a),
        None => {
            eprintln!("Usage: vbr debugbuild <file.vbr>");
            exit(2);
        }
    };
    let result = transpile(&input);

    if is_web_rust(&result.rust) {
        eprintln!("\n✘ This program is a `Page`/web app — debugging the browser build isn't wired up.");
        exit(1);
    }
    if is_godot_rust(&result.rust) {
        eprintln!("\n✘ This is a Godot GDExtension — debug it from inside Godot, not here.");
        exit(1);
    }
    if needs_project(&result.rust) || !result.dependencies.is_empty() {
        eprintln!(
            "\n✘ This program uses the standard library (or a crate via `Use`), so it needs the \
             project build.\n  Debugging project builds isn't wired up yet — this slice covers \
             single-file programs."
        );
        exit(1);
    }

    // Persist the artifacts next to the source, in .vbrdebug/ (git-ignored).
    let dir = input.parent().unwrap_or_else(|| Path::new(".")).join(".vbrdebug");
    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("✘ Could not create {}: {}", dir.display(), e);
        exit(1);
    }
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("main");
    let rs = dir.join(format!("{}.rs", stem));
    let bin = dir.join(if cfg!(windows) {
        format!("{}.exe", stem)
    } else {
        stem.to_string()
    });
    if let Err(e) = fs::write(&rs, &result.rust) {
        eprintln!("✘ Could not write {}: {}", rs.display(), e);
        exit(1);
    }

    eprintln!("→ rustc -g {}", input.display());
    let compiled = Command::new("rustc")
        .args(["--edition", "2021", "-C", "debuginfo=2", "--error-format", "json"])
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

    // Binary path on stdout (clean line for tooling); summary to stderr.
    println!("{}", bin.display());
    eprintln!(
        "✔ Debuggable binary: {}\n  Generated Rust:   {}",
        bin.display(),
        rs.display()
    );
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
    let (build, file_maps) = generate_project(&entry, web, false);
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

    // If the program uses `Log`, point at the file it writes (the run's cwd is
    // `build/`), so a `Screen` you can't `Debug.Print` from is still diagnosable.
    if project_logs(&build) {
        eprintln!("→ logging to {}/vbr.log", build.display());
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

/// One `Test` block, flattened across the project for the runner: its generated
/// `#[test] fn` name, the human description, and the source file + line-map for
/// translating a failure location back to `.vbr`.
struct TestRec {
    fn_name: String,
    description: String,
    source: PathBuf,
    map: Vec<(usize, usize)>,
}

/// `vbr test`: generate the project (its `Test` blocks are already emitted as
/// `#[cfg(test)]` `#[test] fn`s), build the test binary, run it, and translate
/// `cargo test`'s output back to the Bust descriptions and `.vbr` lines.
fn cmd_test(args: &[String]) {
    let entry = match resolve_entry(args.first().map(String::as_str).unwrap_or(".")) {
        Some(e) => e,
        None => exit(1),
    };
    let (build, file_maps) = generate_project(&entry, false, true);

    // Flatten every file's tests into one lookup keyed by the generated fn name.
    let mut recs: Vec<TestRec> = Vec::new();
    for fm in &file_maps {
        for t in &fm.tests {
            recs.push(TestRec {
                fn_name: t.fn_name.clone(),
                description: t.description.clone(),
                source: fm.source.clone(),
                map: fm.map.clone(),
            });
        }
    }
    if recs.is_empty() {
        eprintln!(
            "· No `Test` blocks found. Add a `Test \"what it should do\" … End Test` block \
             (with `Assert …` inside) and run `vbr test` again."
        );
        return;
    }

    // Build the test binary first with JSON diagnostics, so a compile failure is
    // translated back to `.vbr` lines (same as `vbr run`). `--no-run` keeps the
    // run's output clean of cargo's build JSON.
    let built = Command::new("cargo")
        .args(["test", "--no-run", "--message-format", "json", "--quiet"])
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

    // Run the tests. The build is cached, so this only executes them; the plain
    // stdout is the libtest report (one `test NAME ... ok` line each) we
    // translate. No `--quiet` here — that switches libtest to terse dots.
    let run = Command::new("cargo")
        .args(["test"])
        .current_dir(&build)
        .output();
    let run = match run {
        Ok(o) => o,
        Err(e) => {
            eprintln!("✘ Could not run cargo (is it installed?): {}", e);
            exit(1);
        }
    };
    let out = String::from_utf8_lossy(&run.stdout);
    report_test_results(&out, &recs);
    exit(if run.status.success() { 0 } else { 1 });
}

/// Translate libtest's plain output into Bust terms: one `✓ / ✗` line per test,
/// keyed by the human description, with the failure's operand values and the
/// `.vbr` line beneath a `✗`. Tests are shown in **source order** (libtest runs
/// them in parallel, but the suite reads as a spec, so order matters).
fn report_test_results(out: &str, recs: &[TestRec]) {
    // fn name → passed?  (from the `test NAME ... ok/FAILED` lines)
    let mut passed_of: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
    for line in out.lines() {
        let Some(rest) = line.strip_prefix("test ") else { continue };
        let Some((path, result)) = rest.split_once(" ... ") else { continue };
        let fn_name = path.rsplit("::").next().unwrap_or(path).to_string();
        let result = result.trim();
        if result == "ok" {
            passed_of.insert(fn_name, true);
        } else if result.starts_with("FAILED") {
            passed_of.insert(fn_name, false);
        }
    }
    let failures = parse_failure_blocks(out, recs);

    let (mut passed, mut failed) = (0usize, 0usize);
    eprintln!();
    for rec in recs {
        match passed_of.get(&rec.fn_name) {
            Some(true) => {
                passed += 1;
                eprintln!("  ✓ {}", rec.description);
            }
            Some(false) => {
                failed += 1;
                eprintln!("  ✗ {}", rec.description);
                if let Some(d) = failures.get(&rec.fn_name) {
                    for m in &d.message {
                        eprintln!("      {}", m);
                    }
                    if let Some(loc) = &d.location {
                        eprintln!("      {}", loc);
                    }
                }
            }
            None => {} // not run (filtered/ignored) — skip quietly
        }
    }
    eprintln!();
    if failed == 0 {
        eprintln!("  {} passed", passed);
    } else {
        eprintln!("  {} passed, {} failed", passed, failed);
    }
}

/// A failed test's human detail: the assertion + operand values, and the mapped
/// `.vbr` location (shown last).
struct FailureDetail {
    message: Vec<String>,
    location: Option<String>,
}

/// Pull each failed test's operand values (`left`/`right`) and mapped `.vbr`
/// location out of libtest's `failures:` detail blocks.
fn parse_failure_blocks(
    out: &str,
    recs: &[TestRec],
) -> std::collections::HashMap<String, FailureDetail> {
    let mut map: std::collections::HashMap<String, FailureDetail> = std::collections::HashMap::new();
    let lines: Vec<&str> = out.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        // `---- vbr_tests::a stdout ----`
        if let Some(rest) = lines[i].strip_prefix("---- ") {
            if let Some(path) = rest.strip_suffix(" stdout ----") {
                let fn_name = path.rsplit("::").next().unwrap_or(path).to_string();
                let rec = recs.iter().find(|r| r.fn_name == fn_name);
                let mut message: Vec<String> = Vec::new();
                let mut location: Option<String> = None;
                i += 1;
                while i < lines.len()
                    && !lines[i].starts_with("---- ")
                    && lines[i].trim() != "failures:"
                {
                    let l = lines[i].trim();
                    if l.contains("panicked at ") {
                        // `…panicked at src/main.rs:14:9:` → map back to `.vbr`.
                        if let Some(loc) = l.split("panicked at ").nth(1) {
                            location = rec.and_then(|r| map_panic_location(loc, r));
                        }
                    } else if l.starts_with("assertion") {
                        message.push(
                            l.trim_end_matches(" failed")
                                .replace("assertion `", "expected ")
                                .replace('`', ""),
                        );
                    } else if let Some(v) = l.strip_prefix("left: ") {
                        message.push(format!("left:  {}", v));
                    } else if let Some(v) = l.strip_prefix("right: ") {
                        message.push(format!("right: {}", v));
                    }
                    i += 1;
                }
                map.insert(fn_name, FailureDetail { message, location });
                continue;
            }
        }
        i += 1;
    }
    map
}

/// `src/main.rs:14:9:` → `at <source>:<vbr line>` if the line maps.
fn map_panic_location(loc: &str, rec: &TestRec) -> Option<String> {
    // loc looks like `src/main.rs:14:9:` — take the file and the first number.
    let mut parts = loc.split(':');
    let _file = parts.next()?;
    let rs_line: usize = parts.next()?.trim().parse().ok()?;
    let vbr_line = vbr_line_for(&rec.map, rs_line)?;
    Some(format!("at {}:{}", rec.source.display(), vbr_line))
}

/// `vbr runweb`: generate the project, build it for WebAssembly (translating
/// errors back to `.vbr` lines), and serve it in the browser with trunk.
fn cmd_runweb(args: &[String]) {
    let entry = match resolve_entry(args.first().map(String::as_str).unwrap_or(".")) {
        Some(e) => e,
        None => exit(1),
    };
    let (build, file_maps) = generate_project(&entry, true, false);
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

/// `vbr rungodot <file.vbr>` — build a `Node2D` program as a Godot 4
/// **GDExtension** and open it in the editor.
///
/// A Godot program is a cdylib, not a binary: Godot is the host. So we assemble a
/// self-contained Godot *project folder* beside the source —
/// `<stem>_godot/` holding `project.godot`, a `.gdextension` pointing at the
/// built library, a starter `main.tscn`, and a `rust/` crate — build the crate,
/// then hand off to Godot (opening the editor). The folder is stable: you keep
/// editing the scene in Godot while `rungodot` regenerates the Rust.
fn cmd_rungodot(args: &[String]) {
    let arg = match args.first() {
        Some(a) => a.as_str(),
        None => {
            eprintln!("Usage: vbr rungodot <file.vbr | project-dir>");
            exit(2);
        }
    };
    let entry = match resolve_entry(arg) {
        Some(e) => e,
        None => exit(1),
    };
    // A folder (entry `main.vbr`) is a multi-file project; a lone file is a
    // project of one. Named/located beside the source unit either way.
    let is_project = entry.file_name().and_then(|s| s.to_str()) == Some("main.vbr");
    let src_dir = entry.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
    let name = if is_project {
        src_dir.file_name().and_then(|s| s.to_str()).unwrap_or("game").to_string()
    } else {
        entry.file_stem().and_then(|s| s.to_str()).unwrap_or("game").to_string()
    };
    let crate_name = sanitise_crate(&name);
    // Placed beside the *source unit*: for a project folder, next to the folder
    // (`src_dir.parent()`); for a lone file, next to the file (`src_dir`).
    let proj_parent =
        if is_project { src_dir.parent().unwrap_or_else(|| Path::new(".")) } else { &src_dir };
    let proj = proj_parent.join(format!("{}_godot", name));
    let rust_dir = proj.join("rust");

    // --- compile the entry (+ any sibling modules) into rust/src/ -------
    let (entry_rust, file_maps, deps) = generate_godot_sources(&entry, is_project, &rust_dir);
    if !is_godot_rust(&entry_rust) {
        eprintln!(
            "\n✘ This isn't a Godot program — `rungodot` builds a `Node2D` (…) block into \
             a Godot GDExtension.\n  (In a project, the entry `main.vbr` must contain the \
             main scene's root node.)\n  Run an ordinary program with `vbr run`/`runproject`."
        );
        exit(1);
    }
    let (base, class) = godot_class_info(&entry_rust).unwrap_or_else(|| {
        eprintln!("✘ Could not find the node class in the generated code.");
        exit(1);
    });

    // --- the rest of the Godot project folder ---------------------------
    let write = |path: PathBuf, contents: &str| {
        if let Err(e) = fs::write(&path, contents) {
            eprintln!("✘ Could not write {}: {}", path.display(), e);
            exit(1);
        }
    };
    write(rust_dir.join("Cargo.toml"), &godot_cargo_toml(&crate_name, &deps));
    write(proj.join("project.godot"), &godot_project_file(&name));
    write(proj.join(format!("{}.gdextension", crate_name)), &gdextension_file(&crate_name));
    // A project's assets (scenes, textures, audio, an `assets/` folder) → the
    // Godot project root, so `res://…` paths resolve.
    if is_project {
        copy_data_files(&src_dir, &proj);
    }
    // A starter scene, only if the project doesn't supply its own `main.tscn`.
    if !proj.join("main.tscn").exists() {
        write(proj.join("main.tscn"), &godot_main_scene(&base, &class));
    }
    eprintln!("→ project: {}", proj.display());

    // --- build the cdylib (JSON diagnostics → .vbr lines) ---------------
    eprintln!("→ Building the GDExtension — compiling the `godot` crate takes a minute the first time.");
    let built = Command::new("cargo")
        .args(["build", "--message-format", "json", "--quiet"])
        .current_dir(&rust_dir)
        .output();
    match built {
        Ok(o) if o.status.success() => {}
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let errors = parse_cargo_json(&stdout);
            report_errors(&errors, |e| {
                let fname = e.file.as_deref()?;
                file_maps
                    .iter()
                    .find(|m| fname.ends_with(&m.rs_name))
                    .map(|m| (m.source.clone(), m.map.clone()))
            });
            exit(1);
        }
        Err(e) => {
            eprintln!("✘ Could not run cargo (is it installed?): {}", e);
            exit(1);
        }
    }

    // --- hand off to Godot ----------------------------------------------
    match find_godot() {
        Some(bin) => {
            // A fresh project registers the GDExtension only after Godot's
            // `.godot/` cache is built, so warm it once (headless) before playing
            // — otherwise the first run can't find the node class. The cache
            // persists, so this is skipped on later runs. (Headless 4.7 SIGABRTs
            // on teardown *after* writing the cache; `.output()` swallows it.)
            if !proj.join(".godot/extension_list.cfg").exists() {
                eprintln!("→ importing (first run only)…");
                let _ = Command::new(&bin)
                    .args(["--headless", "--editor", "--quit", "--path"])
                    .arg(&proj)
                    .output();
            }
            eprintln!(
                "→ playing (arrow keys move the square; close the window to stop).\n  \
                 To edit the scene: {} -e --path {}\n",
                bin,
                proj.display()
            );
            match Command::new(&bin).arg("--path").arg(&proj).status() {
                Ok(s) => exit(s.code().unwrap_or(0)),
                Err(e) => {
                    eprintln!("✘ Could not launch Godot: {}", e);
                    exit(1);
                }
            }
        }
        None => {
            eprintln!(
                "\n✔ Built. Godot 4 wasn't found on your PATH, so open the project yourself:\n\n    \
                 godot4 --path {}\n\n  \
                 (install Godot 4 from https://godotengine.org, or `snap install godot4`; \
                 set GODOT4_BIN to point at it). Then press Play ▶.",
                proj.display()
            );
        }
    }
}

/// Compile a Godot program's entry (+ any sibling `.vbr` modules, for a project
/// folder) into the cdylib crate's `src/`: the entry → `lib.rs` (crate root:
/// `mod` decls + the `ExtensionLibrary` stub), each sibling → `<name>.rs`. gdext
/// registers every `#[derive(GodotClass)]` in the cdylib regardless of module, so
/// nodes can live in any file. Returns the entry's Rust (for the Godot-program +
/// class checks), the per-file line maps (build errors → `.vbr` lines), and the
/// accumulated `Use` dependencies.
fn generate_godot_sources(
    entry: &Path,
    is_project: bool,
    rust_dir: &Path,
) -> (String, Vec<FileMap>, Vec<(String, String)>) {
    let project_dir = entry.parent().unwrap_or_else(|| Path::new("."));
    let entry_canon = entry.canonicalize().ok();
    let mut vbr_files: Vec<PathBuf> = Vec::new();
    if is_project {
        if let Ok(entries) = fs::read_dir(project_dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.canonicalize().ok() == entry_canon {
                    continue;
                }
                let n = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if p.extension().and_then(|s| s.to_str()) == Some("vbr") && !n.ends_with(".test.vbr")
                {
                    vbr_files.push(p);
                }
            }
        }
    }
    vbr_files.sort();
    let vbr_names: Vec<String> = vbr_files.iter().map(|p| module_of(p)).collect();
    let module_names = vbr_names.clone();

    // Harvest each module's public interface, so a qualified cross-module call
    // gets the same argument treatment as a local one.
    let mut interfaces = vbr::resolver::ProjectInterfaces::new();
    for (f, n) in vbr_files.iter().zip(&vbr_names) {
        if let Ok(source) = fs::read_to_string(f) {
            interfaces.insert(n.clone(), vbr::module_interface(&source));
        }
    }
    if let Ok(source) = fs::read_to_string(entry) {
        interfaces.insert(
            vbr::resolver::CRATE_ROOT.to_string(),
            vbr::module_interface(&source),
        );
    }

    let src = rust_dir.join("src");
    if let Err(e) = fs::create_dir_all(&src) {
        eprintln!("✘ Could not create {}: {}", src.display(), e);
        exit(1);
    }

    let mut file_maps: Vec<FileMap> = Vec::new();
    let mut deps: Vec<(String, String)> = Vec::new();

    let entry_compiled = compile_path(entry, &module_names, &interfaces, true, false);
    if let Err(e) = fs::write(src.join("lib.rs"), &entry_compiled.rust) {
        eprintln!("✘ Could not write lib.rs: {}", e);
        exit(1);
    }
    file_maps.push(FileMap {
        rs_name: "src/lib.rs".to_string(),
        source: entry.to_path_buf(),
        map: entry_compiled.line_map.clone(),
        tests: entry_compiled.tests.clone(),
    });
    deps.extend(entry_compiled.dependencies.clone());
    let entry_rust = entry_compiled.rust;

    for (f, n) in vbr_files.iter().zip(&vbr_names) {
        let compiled = compile_path(f, &module_names, &interfaces, false, false);
        if let Err(e) = fs::write(src.join(format!("{}.rs", n)), &compiled.rust) {
            eprintln!("✘ Could not write {}.rs: {}", n, e);
            exit(1);
        }
        file_maps.push(FileMap {
            rs_name: format!("src/{}.rs", n),
            source: f.clone(),
            map: compiled.line_map.clone(),
            tests: compiled.tests.clone(),
        });
        deps.extend(compiled.dependencies);
    }
    deps.sort();
    deps.dedup();
    (entry_rust, file_maps, deps)
}

/// A Cargo crate name from a file stem: lowercase, non-alphanumerics → `_`.
fn sanitise_crate(stem: &str) -> String {
    let s: String = stem
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect();
    if s.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("g_{}", s)
    } else {
        s
    }
}

/// The cdylib crate's `Cargo.toml` — a GDExtension against gdext, plus any crates
/// the project's modules pulled in with `Use`.
fn godot_cargo_toml(crate_name: &str, deps: &[(String, String)]) -> String {
    let mut s = format!(
        "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
         [lib]\ncrate-type = [\"cdylib\"]\n\n\
         [dependencies]\ngodot = \"0.5\"\n",
        crate_name
    );
    for (c, v) in deps {
        s.push_str(&format!("{} = \"{}\"\n", c, v));
    }
    s
}

/// The `.gdextension` manifest — points Godot at the built library per platform.
/// `entry_symbol` is gdext's fixed init hook. Paths are `res://`-relative to the
/// project folder (so they point straight at cargo's `target/debug` output).
fn gdextension_file(crate_name: &str) -> String {
    let dbg = "res://rust/target/debug";
    let rel = "res://rust/target/release";
    format!(
        "[configuration]\n\
         entry_symbol = \"gdext_rust_init\"\n\
         compatibility_minimum = 4.2\n\
         reloadable = true\n\n\
         [libraries]\n\
         linux.debug.x86_64 =     \"{dbg}/lib{c}.so\"\n\
         linux.release.x86_64 =   \"{rel}/lib{c}.so\"\n\
         windows.debug.x86_64 =   \"{dbg}/{c}.dll\"\n\
         windows.release.x86_64 = \"{rel}/{c}.dll\"\n\
         macos.debug =            \"{dbg}/lib{c}.dylib\"\n\
         macos.release =          \"{rel}/lib{c}.dylib\"\n",
        c = crate_name
    )
}

/// A minimal Godot 4 `project.godot`. GL-compatibility renderer keeps it working
/// on machines without a Vulkan driver (e.g. WSL).
fn godot_project_file(name: &str) -> String {
    format!(
        "config_version=5\n\n\
         [application]\n\
         config/name=\"{name}\"\n\
         run/main_scene=\"res://main.tscn\"\n\
         config/features=PackedStringArray(\"4.2\", \"GL Compatibility\")\n\n\
         [rendering]\n\
         renderer/rendering_method=\"gl_compatibility\"\n"
    )
}

/// A starter scene: the Bust node class as the root, with a coloured box child so
/// there's something visible to move. `type="<Class>"` resolves once Godot loads
/// the GDExtension.
fn godot_main_scene(base: &str, class: &str) -> String {
    if base.ends_with("3D") {
        // 3D needs a camera and a light to see anything, plus a mesh so the node
        // has a visible body. A real game supplies its own scene (drop a
        // `main.tscn` beside the source); this just gets *something* on screen.
        format!(
            "[gd_scene load_steps=2 format=3]\n\n\
             [sub_resource type=\"BoxMesh\" id=\"BoxMesh_1\"]\n\n\
             [node name=\"{class}\" type=\"{class}\"]\n\n\
             [node name=\"Mesh\" type=\"MeshInstance3D\" parent=\".\"]\n\
             mesh = SubResource(\"BoxMesh_1\")\n\n\
             [node name=\"Camera\" type=\"Camera3D\" parent=\".\"]\n\
             transform = Transform3D(1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 1, 4)\n\n\
             [node name=\"Light\" type=\"DirectionalLight3D\" parent=\".\"]\n\
             transform = Transform3D(1, 0, 0, 0, 0.7, 0.7, 0, -0.7, 0.7, 0, 5, 0)\n"
        )
    } else {
        format!(
            "[gd_scene format=3]\n\n\
             [node name=\"{class}\" type=\"{class}\"]\n\n\
             [node name=\"Box\" type=\"ColorRect\" parent=\".\"]\n\
             offset_right = 40.0\n\
             offset_bottom = 40.0\n\
             color = Color(0.3, 0.7, 1, 1)\n"
        )
    }
}

/// Find a Godot 4 executable: `$GODOT4_BIN`/`$GODOT_BIN`, then `godot4`/`godot`
/// on PATH.
fn find_godot() -> Option<String> {
    for var in ["GODOT4_BIN", "GODOT_BIN"] {
        if let Ok(p) = std::env::var(var) {
            if !p.is_empty() {
                return Some(p);
            }
        }
    }
    for bin in ["godot4", "godot"] {
        if Command::new(bin).arg("--version").output().is_ok_and(|o| o.status.success()) {
            return Some(bin.to_string());
        }
    }
    None
}

/// Does the generated project use `Log` (so the run writes `vbr.log`)? Scans the
/// emitted `src/*.rs` for the sink helper.
fn project_logs(build: &Path) -> bool {
    fs::read_dir(build.join("src"))
        .map(|entries| {
            entries.flatten().any(|e| {
                fs::read_to_string(e.path())
                    .map(|s| s.contains("fn vbr_log("))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// `vbr graduate <file.vbr>` — replace a module with the Rust it generates.
///
/// Bust's end goal is that you stop needing it: the generated Rust *is* the
/// curriculum, and graduation is the day one file of it becomes yours. The
/// module's generated `.rs` — exactly what `build/` has been compiling all
/// along, no rewriting, no drift — is placed next to the sources, the `.vbr`
/// is retired to `.vbr.graduated`, and the project is rebuilt (tests and all)
/// to prove nothing changed. From then on you maintain that file in Rust; the
/// remaining Bust modules keep calling it. Graduate `main.vbr` last: that
/// finishes the journey, and `build/` is a plain cargo project you own.
fn cmd_graduate(args: &[String]) {
    let Some(path_arg) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("✘ Which file? `vbr graduate <file.vbr>`");
        exit(2);
    };
    let path = PathBuf::from(path_arg);
    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
    if path.extension().and_then(|e| e.to_str()) != Some("vbr") || !path.is_file() {
        eprintln!("✘ {} is not a .vbr file.", path.display());
        exit(1);
    }
    if file_name.ends_with(".test.vbr") {
        eprintln!(
            "✘ A `.test.vbr` module stays Bust — its Test blocks are the readable \
             spec. Graduate the modules it tests instead."
        );
        exit(1);
    }
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
    let dir = if dir.as_os_str().is_empty() { PathBuf::from(".") } else { dir };
    let stem = stem_name(&path);
    let module = module_of(&path);
    let main_vbr = dir.join("main.vbr");
    // The entry graduates last: `main.vbr`, or a standalone file that is its
    // own project of one.
    let is_entry = file_name.eq_ignore_ascii_case("main.vbr") || !main_vbr.is_file();

    // The other modules still written in Bust (tests don't count — they stay).
    let mut vbr_siblings: Vec<String> = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
            if p.extension().and_then(|x| x.to_str()) == Some("vbr")
                && name != file_name
                && !name.ends_with(".test.vbr")
            {
                vbr_siblings.push(name);
            }
        }
    }
    if is_entry && !vbr_siblings.is_empty() {
        vbr_siblings.sort();
        eprintln!(
            "✘ The entry graduates last — these modules are still Bust:\n      {}\n  \
             Graduate them first, then come back for {}.",
            vbr_siblings.join(", "),
            file_name
        );
        exit(1);
    }
    let target = dir.join(format!("{}.rs", stem));
    if target.exists() {
        eprintln!(
            "✘ {} already exists — is this module already graduated?",
            target.display()
        );
        exit(1);
    }
    let has_tests = fs::read_dir(&dir).ok().is_some_and(|entries| {
        entries.flatten().any(|e| {
            e.file_name().to_str().is_some_and(|n| n.ends_with(".test.vbr"))
        })
    });

    // The graduated content is the build artifact itself: generate the project
    // (which also proves everything still transpiles) and lift the file out.
    let entry = if is_entry { path.clone() } else { main_vbr.clone() };
    let (build, _) = generate_project(&entry, false, true);
    let built_file = if is_entry {
        build.join("src/main.rs")
    } else {
        build.join(format!("src/{}.rs", module))
    };
    let rust = match fs::read_to_string(&built_file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("✘ Could not read {}: {}", built_file.display(), e);
            exit(1);
        }
    };
    let graduated = format!(
        "// Graduated from {} — this is the Rust your Bust became, now yours to keep.\n\
         // (The retired original beside it tells the build how Bust callers pass\n\
         // arguments here; it stops mattering once the whole project is Rust.)\n\n{}",
        file_name, rust
    );

    if is_entry {
        // The final step: verify the build as-is (nothing changes underneath
        // it), then hand the keys over.
        if !cargo_passes(&build, has_tests) {
            exit(1);
        }
        write_or_die(&target, &graduated);
        retire(&path);
        eprintln!("🎓 {} → {} — the last module. The journey is complete.", file_name, target.display());
        eprintln!(
            "   Every module is Rust now; nothing here needs Bust any more. The cargo\n   \
             project in build/ compiles exactly these files — it's yours:\n       \
             cd {} && cargo run",
            build.display()
        );
        return;
    }

    // A sibling module: promote, then prove the project still builds with the
    // graduated file compiled verbatim (and its tests still passing). The one
    // honest risk is a caller that leaned on Bust's argument sugar toward this
    // module — cargo is the ground truth, and failure rolls everything back.
    write_or_die(&target, &graduated);
    retire(&path);
    let (build, _) = generate_project(&main_vbr, false, true);
    if !cargo_passes(&build, has_tests) {
        // Roll back: restore the .vbr, remove the .rs.
        let _ = fs::rename(dir.join(format!("{}.graduated", file_name)), &path);
        let _ = fs::remove_file(&target);
        eprintln!(
            "✘ Graduation rolled back — nothing changed. (Most likely another module \
             relies on Bust's argument treatment when calling this one; the errors \
             above show where.)"
        );
        exit(1);
    }
    eprintln!("🎓 {} → {}", file_name, target.display());
    eprintln!(
        "   The Rust it generated is now the source you keep — the other modules\n   \
         still call it; nothing changed but ownership. The original stays beside\n   \
         it as {}.graduated: it teaches the build how Bust callers pass\n   \
         arguments to this module, so keep it until the whole project graduates.",
        file_name
    );
    eprintln!("   ✓ project still builds");
    if has_tests {
        eprintln!("   ✓ tests still pass");
    }
}

/// Run `cargo build` (or `cargo test`, which builds and runs the specs) in the
/// generated project; on failure print the tail of the errors.
fn cargo_passes(build: &Path, run_tests: bool) -> bool {
    let out = Command::new("cargo")
        .args(if run_tests { ["test", "--quiet"] } else { ["build", "--quiet"] })
        .current_dir(build)
        .output();
    match out {
        Ok(o) if o.status.success() => true,
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr);
            for line in err.lines().rev().take(20).collect::<Vec<_>>().into_iter().rev() {
                eprintln!("  {}", line);
            }
            false
        }
        Err(e) => {
            eprintln!("✘ Could not run cargo: {}", e);
            false
        }
    }
}

fn write_or_die(path: &Path, contents: &str) {
    if let Err(e) = fs::write(path, contents) {
        eprintln!("✘ Could not write {}: {}", path.display(), e);
        exit(1);
    }
}

/// Retire a graduated `.vbr` — rename it to `.vbr.graduated` so the project
/// scanner no longer sees it, but nothing is lost until you delete it.
fn retire(path: &Path) {
    let retired = PathBuf::from(format!("{}.graduated", path.display()));
    if let Err(e) = fs::rename(path, &retired) {
        eprintln!("✘ Could not rename {}: {}", path.display(), e);
        exit(1);
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
    tests: Vec<vbr::TestInfo>,
}

/// Generate the cargo project under `<project>/build/` and return its path
/// plus the per-file line maps (for translating build errors).
fn generate_project(entry: &Path, web: bool, include_tests: bool) -> (PathBuf, Vec<FileMap>) {
    let project_dir = entry.parent().unwrap_or_else(|| Path::new("."));

    // A multi-module project is a folder whose entry is `main.vbr`; its siblings
    // are modules. A standalone file (e.g. `settings.vbr`) is a project of one —
    // we must NOT pull in unrelated neighbours (that would, say, try to compile
    // every other `.vbr` in `examples/`).
    let is_project = entry.file_name().and_then(|s| s.to_str()) == Some("main.vbr");

    // Discover sibling modules: every other `.vbr` file (transpiled), plus any
    // `.rs` file (included verbatim — a hand-written Rust module). A `*.test.vbr`
    // file is a **test module** — the dedicated home for `Test` blocks; it is
    // compiled (as `#[cfg(test)]`) only for `vbr test`, and skipped entirely by
    // `vbr run`/`build` so tested-only logic never counts as unused in the app.
    let entry_canon = entry.canonicalize().ok();
    let mut vbr_files: Vec<PathBuf> = Vec::new();
    let mut rs_files: Vec<PathBuf> = Vec::new();
    let mut test_files: Vec<PathBuf> = Vec::new();
    let mut graduated_files: Vec<PathBuf> = Vec::new();
    if is_project {
        if let Ok(entries) = fs::read_dir(project_dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.canonicalize().ok() == entry_canon {
                    continue;
                }
                let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
                let is_test = name.ends_with(".test.vbr");
                if name.ends_with(".vbr.graduated") {
                    graduated_files.push(p);
                    continue;
                }
                match p.extension().and_then(|s| s.to_str()) {
                    Some("vbr") if is_test => test_files.push(p),
                    Some("vbr") => vbr_files.push(p),
                    // A stray `main.rs` would clobber the generated entry — skip it.
                    Some("rs") if stem_name(&p) != "main" => rs_files.push(p),
                    _ => {}
                }
            }
        }
    }
    // A single test file passed directly (`vbr test foo.test.vbr`) is its own
    // project — treat the entry itself as the test module below is not needed;
    // the entry compiles normally and any inline `Test` blocks are emitted.
    vbr_files.sort();
    rs_files.sort();
    test_files.sort();
    let test_names: Vec<String> = test_files.iter().map(|p| test_module_of(p)).collect();
    let vbr_names: Vec<String> = vbr_files.iter().map(|p| module_of(p)).collect();
    let rs_names: Vec<String> = rs_files.iter().map(|p| module_of(p)).collect();
    // Every sibling module is a possible qualified-call target and a `mod` decl.
    let module_names: Vec<String> = vbr_names.iter().chain(&rs_names).cloned().collect();

    // Pass 1: harvest each `.vbr` module's interface (public functions and
    // constants), so pass 2 can give a qualified call the same argument
    // treatment as a local one. Verbatim `.rs` modules have no Bust interface —
    // their calls stay name-qualified only.
    let mut interfaces = vbr::resolver::ProjectInterfaces::new();
    for (file, name) in vbr_files.iter().zip(&vbr_names) {
        if let Ok(source) = fs::read_to_string(file) {
            interfaces.insert(name.clone(), vbr::module_interface(&source));
        }
    }
    // Public Types/Enums on the entry (`main.vbr`) are crate-root items, not a
    // `mod main`. Harvest them under a sentinel so a sibling can `use crate::Name;`
    // — they must not join `module_names` (that would emit `mod main;`).
    if let Ok(source) = fs::read_to_string(entry) {
        interfaces.insert(
            vbr::resolver::CRATE_ROOT.to_string(),
            vbr::module_interface(&source),
        );
    }
    // A graduated module (`life.rs` beside `life.vbr.graduated`) keeps its Bust
    // interface: the retired file records how Bust callers treat its arguments
    // (`ByRef` → `&mut`, collections borrow), so the calls other modules
    // generate don't change on graduation day. It stops mattering — and can be
    // deleted — once nothing in Bust calls the module (or you've adjusted the
    // callers by hand).
    for p in &graduated_files {
        let stem = p
            .file_name()
            .and_then(|s| s.to_str())
            .and_then(|n| n.strip_suffix(".vbr.graduated"))
            .unwrap_or("")
            .to_string();
        let name = vbr::module_name(&stem);
        if rs_names.contains(&name) && !interfaces.contains_key(&name) {
            if let Ok(source) = fs::read_to_string(p) {
                interfaces.insert(name, vbr::module_interface(&source));
            }
        }
    }

    let build = project_dir.join("build");
    let src = build.join("src");
    if let Err(e) = fs::create_dir_all(&src) {
        eprintln!("✘ Could not create {}: {}", src.display(), e);
        exit(1);
    }

    // Entry → main.rs (crate root: `mod` declarations + `fn main`).
    let mut file_maps: Vec<FileMap> = Vec::new();
    let entry_compiled = compile_path(entry, &module_names, &interfaces, true, web);
    // For `vbr test`, declare each `*.test.vbr` file as a `#[cfg(test)]` module —
    // so `cargo test` compiles it, but a plain build never sees it. Appended at
    // the end (item order is free in Rust) so main.rs's line map — which
    // translates its errors back to `.vbr` — keeps its offsets.
    let mut entry_rust = entry_compiled.rust.clone();
    if include_tests && !test_names.is_empty() {
        entry_rust.push('\n');
        for n in &test_names {
            entry_rust.push_str(&format!("#[cfg(test)]\nmod {};\n", n));
        }
    }
    if let Err(e) = fs::write(src.join("main.rs"), &entry_rust) {
        eprintln!("✘ Could not write main.rs: {}", e);
        exit(1);
    }
    file_maps.push(FileMap {
        rs_name: "src/main.rs".to_string(),
        source: entry.to_path_buf(),
        map: entry_compiled.line_map.clone(),
        tests: entry_compiled.tests.clone(),
    });
    let mut any_stdlib = needs_project(&entry_compiled.rust);
    // An async GUI (an event with `Await`) runs blocking work via tokio, so Iced
    // needs its `tokio` feature; an `Image` needs Iced's `image` feature.
    let async_gui = entry_compiled.rust.contains("spawn_blocking");
    let uses_image = entry_compiled.rust.contains("iced::widget::image");
    let uses_canvas = entry_compiled.rust.contains("iced::widget::Canvas::new(");
    let uses_time = entry_compiled.rust.contains("iced::time::every");
    let uses_advanced = entry_compiled.rust.contains("iced::advanced::");
    let mut deps: Vec<(String, String)> = entry_compiled.dependencies.clone();
    let mut stdlib_ns: Vec<String> = entry_compiled.stdlib_used.clone();

    // Each `.vbr` sibling → transpiled `<name>.rs`.
    for (file, name) in vbr_files.iter().zip(&vbr_names) {
        let compiled = compile_path(file, &module_names, &interfaces, false, web);
        let path = src.join(format!("{}.rs", name));
        if let Err(e) = fs::write(&path, &compiled.rust) {
            eprintln!("✘ Could not write {}: {}", path.display(), e);
            exit(1);
        }
        file_maps.push(FileMap {
            rs_name: format!("src/{}.rs", name),
            source: file.clone(),
            map: compiled.line_map.clone(),
            tests: compiled.tests.clone(),
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

    // Each `*.test.vbr` → `<name>_test.rs` (only for `vbr test`). It's compiled
    // with the real modules in scope, so its `Test` blocks call them by the
    // qualified name (`Life.StepCell`); its output is all `#[cfg(test)]`.
    if include_tests {
        for (file, name) in test_files.iter().zip(&test_names) {
            let compiled = compile_path(file, &module_names, &interfaces, false, web);
            let path = src.join(format!("{}.rs", name));
            if let Err(e) = fs::write(&path, &compiled.rust) {
                eprintln!("✘ Could not write {}: {}", path.display(), e);
                exit(1);
            }
            file_maps.push(FileMap {
                rs_name: format!("src/{}.rs", name),
                source: file.clone(),
                map: compiled.line_map.clone(),
                tests: compiled.tests.clone(),
            });
            any_stdlib |= needs_project(&compiled.rust);
            deps.extend(compiled.dependencies);
            stdlib_ns.extend(compiled.stdlib_used);
        }
    }

    // The program runs with `build/` as its working directory, so the project's
    // *data files* — `config.json`, a `data/` folder — must be there to be
    // found. Copy them across on every build (the project folder is the source
    // of truth): top-level files that aren't sources (`.vbr`/`.rs`) or docs
    // (`.md`), and whole subdirectories, skipping dotfiles and `build/` itself.
    if is_project {
        copy_data_files(project_dir, &build);
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
            // iced's default is wgpu with tiny-skia as fallback. Software-only
            // (tiny-skia → softbuffer) dies on WSLg for some animated 640×480
            // sketches (`Io error: Connection reset by peer`). wgpu uses the
            // GPU present path and survives; tiny-skia remains if wgpu cannot
            // init (no adapter).
            let mut feats = vec!["\"wgpu\"", "\"tiny-skia\""];
            if async_gui || uses_time {
                feats.push("\"tokio\"");
            }
            if uses_image {
                feats.push("\"image\"");
            }
            if uses_canvas {
                feats.push("\"canvas\"");
            }
            if uses_advanced {
                feats.push("\"advanced\"");
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
            .unwrap_or_else(|| "Bust app".to_string());
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
    ("Database", "database"),
];

/// The raw file stem (`http.rs` → `http`), before lowercasing.
fn stem_name(p: &Path) -> String {
    p.file_stem().and_then(|s| s.to_str()).unwrap_or("module").to_string()
}

/// The Rust module name for a project file (`MyHelpers.vbr` → `my_helpers`).
fn module_of(p: &Path) -> String {
    vbr::module_name(&stem_name(p))
}

/// The Rust module name for a `foo.test.vbr` test file — `foo_test`, kept
/// distinct from the real `foo` module it exercises.
fn test_module_of(p: &Path) -> String {
    let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("mod.test.vbr");
    let base = name.strip_suffix(".test.vbr").unwrap_or(name);
    vbr::module_name(&format!("{}_test", base))
}

/// Copy a folder project's data files into `build/` (see the call site).
/// A failed copy warns rather than kills the build — the program may not even
/// read the file.
fn copy_data_files(project_dir: &Path, build: &Path) {
    let Ok(entries) = fs::read_dir(project_dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        let name = match p.file_name().and_then(|s| s.to_str()) {
            Some(n) if !n.starts_with('.') => n.to_string(),
            _ => continue,
        };
        if p.is_dir() {
            if name != "build" {
                copy_dir_recursive(&p, &build.join(&name));
            }
        } else if !matches!(p.extension().and_then(|s| s.to_str()), Some("vbr" | "rs" | "md")) {
            if let Err(err) = fs::copy(&p, build.join(&name)) {
                eprintln!("⚠ Could not copy {} into build/: {}", p.display(), err);
            }
        }
    }
}

/// Recursively copy a data directory (e.g. `data/`) into the build folder.
/// Everything inside is data — only dotfiles are skipped.
fn copy_dir_recursive(from: &Path, to: &Path) {
    if let Err(err) = fs::create_dir_all(to) {
        eprintln!("⚠ Could not create {}: {}", to.display(), err);
        return;
    }
    let Ok(entries) = fs::read_dir(from) else { return };
    for e in entries.flatten() {
        let p = e.path();
        let name = match p.file_name().and_then(|s| s.to_str()) {
            Some(n) if !n.starts_with('.') => n.to_string(),
            _ => continue,
        };
        if p.is_dir() {
            copy_dir_recursive(&p, &to.join(&name));
        } else if let Err(err) = fs::copy(&p, to.join(&name)) {
            eprintln!("⚠ Could not copy {} into build/: {}", p.display(), err);
        }
    }
}

/// Read + compile one project file (as entry or module), printing diagnostics
/// and exiting on error.
fn compile_path(
    path: &Path,
    modules: &[String],
    interfaces: &vbr::resolver::ProjectInterfaces,
    is_entry: bool,
    web: bool,
) -> vbr::Compiled {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("✘ Could not read {}: {}", path.display(), e);
            exit(1);
        }
    };
    let result = if web {
        vbr::compile_module_web(&source, modules, interfaces, is_entry)
    } else {
        vbr::compile_module(&source, modules, interfaces, is_entry)
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
// The transpiler records (generated-Rust line → Bust line) checkpoints as it
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

/// The Bust line a generated-Rust line came from: the last checkpoint at or
/// before it (checkpoints are recorded in ascending emission order).
fn vbr_line_for(map: &[(usize, usize)], rust_line: usize) -> Option<usize> {
    map.iter()
        .take_while(|(r, _)| *r <= rust_line)
        .last()
        .map(|(_, v)| *v)
}

/// A hint for the Rust errors a VB programmer meets first. Deliberately short —
/// the goal is orientation, not a lecture.
/// Hints keyed on the message text, for cases rustc's error *code* alone can't
/// tell apart — e.g. a .NET/Java habit reaching for `.Length` on a `Vec`.
fn message_hint(message: &str) -> Option<&'static str> {
    if message.contains("no field `length`") || message.contains("no method named `length`") {
        return Some("A collection's length in Bust is `.Len()` (or `.Count()`), not `.Length`.");
    }
    None
}

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
             patterns you're writing real Rust, so use the lowercase spelling — Bust's \
             `myTotal` is `mytotal` there."
        }
        "E0599" => {
            "No method with that name on this type. Method calls pass straight through \
             to Rust — check the name against Rust's String/Vec docs (Bust lowercases it)."
        }
        _ => return None,
    })
}

/// Print translated errors. `locate` finds the (.vbr path, line map) for an
/// error; anything it can't place falls back to rustc's own rendering.
fn report_errors(errors: &[RustcError], locate: impl Fn(&RustcError) -> Option<(PathBuf, Vec<(usize, usize)>)>) {
    if errors.is_empty() {
        eprintln!("✘ rustc rejected the generated Rust (and produced no diagnostics Bust could read).");
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
                if let Some(hint) = message_hint(&e.message) {
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
