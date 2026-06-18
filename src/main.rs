//! VBR command-line driver.
//!
//!   vbr input.vbr            transpile to input.rs
//!   vbr input.vbr -o out.rs  transpile to a chosen path
//!   vbr input.vbr --run      transpile, compile with rustc, and run the binary
//!   vbr input.vbr --emit     transpile and print the Rust to stdout

use std::path::{Path, PathBuf};
use std::process::{exit, Command};

struct Args {
    input: PathBuf,
    output: Option<PathBuf>,
    run: bool,
    emit: bool,
}

fn main() {
    let args = match parse_args() {
        Some(a) => a,
        None => {
            eprintln!(
                "Usage: vbr <input.vbr> [-o <output.rs>] [--run] [--emit]\n\
                 \n\
                 \t--run   compile the generated Rust with rustc and execute it\n\
                 \t--emit  print the generated Rust to stdout"
            );
            exit(2);
        }
    };

    let source = match std::fs::read_to_string(&args.input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("✘ Could not read {}: {}", args.input.display(), e);
            exit(1);
        }
    };

    let result = vbr::compile(&source);

    for d in &result.diagnostics {
        eprintln!("{}", d);
    }
    if result.has_errors {
        eprintln!("\nTranspilation failed — no Rust was written.");
        exit(1);
    }

    if args.emit {
        print!("{}", result.rust);
    }

    let out_path = args
        .output
        .clone()
        .unwrap_or_else(|| args.input.with_extension("rs"));

    if let Err(e) = std::fs::write(&out_path, &result.rust) {
        eprintln!("✘ Could not write {}: {}", out_path.display(), e);
        exit(1);
    }
    eprintln!("✔ Wrote {}", out_path.display());

    if args.run {
        run_with_rustc(&out_path);
    }
}

fn parse_args() -> Option<Args> {
    let mut input = None;
    let mut output = None;
    let mut run = false;
    let mut emit = false;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--run" => run = true,
            "--emit" => emit = true,
            "-o" | "--output" => output = Some(PathBuf::from(it.next()?)),
            _ if arg.starts_with('-') => return None,
            _ => input = Some(PathBuf::from(arg)),
        }
    }

    Some(Args {
        input: input?,
        output,
        run,
        emit,
    })
}

/// Compile the generated Rust and run it, so a `.vbr` file feels like a program.
fn run_with_rustc(rs_path: &Path) {
    let bin_path = rs_path.with_extension(""); // strip .rs
    eprintln!("→ rustc {}", rs_path.display());
    let status = Command::new("rustc")
        .arg("--edition")
        .arg("2021")
        .arg(rs_path)
        .arg("-o")
        .arg(&bin_path)
        .status();

    match status {
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

    eprintln!("→ running {}\n", bin_path.display());
    let run = Command::new(&bin_path).status();
    match run {
        Ok(s) => exit(s.code().unwrap_or(0)),
        Err(e) => {
            eprintln!("✘ Could not run {}: {}", bin_path.display(), e);
            exit(1);
        }
    }
}
