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

    if needs_project(&result.rust) {
        eprintln!(
            "\n✘ This program uses the standard library (or an external crate), which \
             needs the project build.\n  Run it with `vbr runproject` instead."
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
    let result = transpile(entry);

    let project_dir = entry.parent().unwrap_or_else(|| Path::new("."));
    let build = project_dir.join("build");
    let src = build.join("src");
    if let Err(e) = fs::create_dir_all(&src) {
        eprintln!("✘ Could not create {}: {}", src.display(), e);
        exit(1);
    }

    if let Err(e) = fs::write(src.join("main.rs"), &result.rust) {
        eprintln!("✘ Could not write main.rs: {}", e);
        exit(1);
    }

    let mut cargo = format!(
        "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n",
        pkg_name(entry)
    );
    if needs_project(&result.rust) {
        cargo.push_str(&format!("vbr_stdlib = {{ path = \"{}\" }}\n", stdlib_path()));
    }
    if let Err(e) = fs::write(build.join("Cargo.toml"), cargo) {
        eprintln!("✘ Could not write Cargo.toml: {}", e);
        exit(1);
    }

    build
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
fn stdlib_path() -> String {
    std::env::var("VBR_STDLIB_PATH")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/vbr_stdlib").to_string())
}
