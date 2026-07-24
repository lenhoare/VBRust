//! Golden tests for the **C** backend (slice 1: scalars, strings, control flow).
//!
//! Two guarantees, mirroring the Python suite:
//!   1. the generated C is locked against a stored snapshot (`tests/snapshots/
//!      <name>.c`), and
//!   2. its runtime stdout equals the Rust ground truth (`vbr run`), byte for
//!      byte — the whole discipline of a second/third target.
//!
//! Regenerate snapshots after an intended change with:
//!     UPDATE_SNAPSHOTS=1 cargo test --test c

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Slice 1: pure computation + strings + `= Nothing`.
/// Slice 2: `Type`/struct, methods (`Me`→`self->`), module `Const`.
/// Slice 3: `Match`/`Enum` — C `enum` + tagged unions + if-chain lowering.
/// Slice 4: collections — monomorphised `Vec`/`HashMap`, iterator loops.
/// Slice 5: `Option`/`Result`/`?` — struct wrappers, propagation, `.Unwrap()`.
const C: &[&str] = &[
    // slice 1
    "hello", "functions", "logic", "maths", "doloop", "memory",
    // slice 2
    "types", "structs", "methods", "constants",
    // slice 3
    "match", "match_guards", "enums", "sum_types",
    // slice 4 (deterministic)
    "vec", "list_literal", "iterators",
    // slice 5 — Option / Result / `?`
    "option", "result", "result_e", "result_unit",
];

/// Collection examples whose runtime *order* isn't reproducible against Rust
/// (a `HashMap` iterates in Rust's randomised order, ours in insertion order),
/// so the generated C is snapshotted and merely compiled + run, not diffed.
const C_SNAPSHOT_ONLY: &[&str] = &["hashmap"];

/// Standard-library examples. `rustc` alone can't link the stdlib, so the Rust
/// ground truth is a stored `.out` (captured from `vbr runproject`); the C is
/// compiled, run, and diffed against it — the same discipline as the deterministic
/// examples, just with the reference precomputed.
const C_STDLIB: &[&str] = &["filesystem"];

fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples")
}

fn snapshots_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

fn read_example(name: &str) -> String {
    let path = examples_dir().join(format!("{name}.vbr"));
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

fn check_snapshot(name: &str, ext: &str, actual: &str) {
    let path = snapshots_dir().join(format!("{name}.{ext}"));
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        fs::create_dir_all(snapshots_dir()).unwrap();
        fs::write(&path, actual).unwrap();
        return;
    }
    let expected = fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!("missing snapshot {} — run `UPDATE_SNAPSHOTS=1 cargo test` to create it", path.display())
    });
    assert_eq!(
        actual, expected,
        "snapshot mismatch for {name}.{ext} — rerun with UPDATE_SNAPSHOTS=1 if intended"
    );
}

#[test]
fn c_output_matches_snapshots() {
    for name in C.iter().chain(C_SNAPSHOT_ONLY).chain(C_STDLIB) {
        let result = vbr::compile_c(&read_example(name));
        assert!(!result.has_errors, "{name} produced errors: {:?}", result.diagnostics);
        assert!(result.warnings.is_empty(), "{name} warned: {:?}", result.warnings);
        check_snapshot(name, "c", &result.code);
    }
}

/// Snapshot-only examples must still *build and run* — the code being valid C is
/// the guarantee here (its output order just can't be diffed against Rust).
#[test]
fn c_snapshot_only_compiles() {
    if Command::new("cc").arg("--version").output().is_err() {
        eprintln!("skipping c_snapshot_only_compiles: no cc");
        return;
    }
    for name in C_SNAPSHOT_ONLY {
        let _ = run_via_c(name, &read_example(name));
    }
}

/// The heart of the discipline: C stdout must equal Rust stdout. Skips (rather
/// than fails) if a toolchain is absent, so the suite still runs without `cc`.
#[test]
fn c_behaviour_matches_rust() {
    if Command::new("cc").arg("--version").output().is_err() {
        eprintln!("skipping c_behaviour_matches_rust: no cc");
        return;
    }
    if Command::new("rustc").arg("--version").output().is_err() {
        eprintln!("skipping c_behaviour_matches_rust: no rustc");
        return;
    }
    for name in C {
        let src = read_example(name);
        let rust_out = run_via_rust(name, &src);
        let c_out = run_via_c(name, &src);
        assert_eq!(
            rust_out, c_out,
            "{name}: C stdout differs from Rust stdout (ground truth)"
        );
    }
}

/// Standard-library examples: compile the C, run it, and diff stdout against the
/// stored `.out` (the `vbr runproject` ground truth). Skips without `cc`.
#[test]
fn c_stdlib_matches_out() {
    if Command::new("cc").arg("--version").output().is_err() {
        eprintln!("skipping c_stdlib_matches_out: no cc");
        return;
    }
    for name in C_STDLIB {
        let out = run_via_c(name, &read_example(name));
        check_snapshot(name, "out", &out);
    }
}

/// Transpile to Rust, compile with rustc, run, return stdout.
fn run_via_rust(name: &str, src: &str) -> String {
    let compiled = vbr::compile(src);
    assert!(!compiled.has_errors, "{name} (rust) errors: {:?}", compiled.diagnostics);
    let dir = std::env::temp_dir().join(format!("vbr_c_rust_{name}"));
    fs::create_dir_all(&dir).unwrap();
    let rs = dir.join("main.rs");
    let bin = dir.join("main_bin");
    fs::write(&rs, &compiled.rust).unwrap();
    let built = Command::new("rustc")
        .args(["--edition", "2021", "-o"])
        .arg(&bin)
        .arg(&rs)
        .output()
        .expect("rustc");
    assert!(
        built.status.success(),
        "{name}: rustc failed:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let run = Command::new(&bin).output().expect("run rust binary");
    String::from_utf8_lossy(&run.stdout).into_owned()
}

/// Transpile to C, compile with cc (`-lm` for the maths builtins), run, return
/// stdout.
fn run_via_c(name: &str, src: &str) -> String {
    let result = vbr::compile_c(src);
    assert!(!result.has_errors, "{name} (c) errors: {:?}", result.diagnostics);
    let dir = std::env::temp_dir().join(format!("vbr_c_c_{name}"));
    fs::create_dir_all(&dir).unwrap();
    let cfile = dir.join("main.c");
    let bin = dir.join("main_bin");
    fs::write(&cfile, &result.code).unwrap();
    let built = Command::new("cc")
        .arg(&cfile)
        .arg("-lm")
        .arg("-o")
        .arg(&bin)
        .output()
        .expect("cc");
    assert!(
        built.status.success(),
        "{name}: cc failed:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let run = Command::new(&bin).output().expect("run c binary");
    String::from_utf8_lossy(&run.stdout).into_owned()
}
