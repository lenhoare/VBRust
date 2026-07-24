//! Golden tests for the VBR → Python backend (`vbr::compile_python`).
//!
//! Two guarantees, per the "compile-against-truth" discipline:
//!   1. the generated Python is locked against a stored snapshot
//!      (`tests/snapshots/<name>.py`), and
//!   2. it is *behaviourally* identical to the Rust output — each example is
//!      compiled+run through rustc AND run through `python3`, and their stdout
//!      must match byte-for-byte. `vbr run` (Rust) is the ground truth.
//!
//! Regenerate snapshots after an intended change with:
//!     UPDATE_SNAPSHOTS=1 cargo test
//!
//! Slice 1 is the pure-computation core; the list grows as the backend does.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Examples the Python backend fully supports:
///   slice 1 — pure computation; slice 2 — `Type`/methods/`Const`;
///   slice 3 — `Match` + `Enum`.
const PY: &[&str] = &[
    // slice 1
    "hello", "functions", "logic", "maths", "doloop",
    // slice 2
    "types", "structs", "methods", "constants",
    // slice 3
    "match", "match_guards", "enums", "sum_types",
    // slice 4 — collections + the method table
    "vec", "list_literal", "iterators", "enum_payloads", "field_inference",
    // slice 5 — Option / Result
    "option", "result", "result_e", "result_unit", "iterator_strings", "iterator_more",
];

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
fn python_output_matches_snapshots() {
    for name in PY {
        let result = vbr::compile_python(&read_example(name));
        assert!(
            !result.has_errors,
            "{name} produced errors: {:?}",
            result.diagnostics
        );
        assert!(
            result.warnings.is_empty(),
            "{name} is a slice-1 example but warned: {:?}",
            result.warnings
        );
        check_snapshot(name, "py", &result.code);
    }
}

/// The heart of the discipline: Python stdout must equal Rust stdout. Skips
/// (rather than fails) if a toolchain is absent, so the suite still runs on a
/// machine without `python3`.
#[test]
fn python_behaviour_matches_rust() {
    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping python_behaviour_matches_rust: no python3");
        return;
    }
    if Command::new("rustc").arg("--version").output().is_err() {
        eprintln!("skipping python_behaviour_matches_rust: no rustc");
        return;
    }

    for name in PY {
        let src = read_example(name);
        let rust_out = run_via_rust(name, &src);
        let py_out = run_via_python(name, &src);
        assert_eq!(
            rust_out, py_out,
            "{name}: Python stdout differs from Rust stdout (ground truth)"
        );
    }
}

/// Transpile to Rust, compile with rustc, run, return stdout.
fn run_via_rust(name: &str, src: &str) -> String {
    let compiled = vbr::compile(src);
    assert!(!compiled.has_errors, "{name} (rust) errors: {:?}", compiled.diagnostics);
    let dir = std::env::temp_dir().join(format!("vbr_py_rust_{name}"));
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

/// Transpile to Python, run with python3, return stdout.
fn run_via_python(name: &str, src: &str) -> String {
    let compiled = vbr::compile_python(src);
    assert!(!compiled.has_errors, "{name} (python) errors: {:?}", compiled.diagnostics);
    let dir = std::env::temp_dir().join(format!("vbr_py_run_{name}"));
    fs::create_dir_all(&dir).unwrap();
    let py = dir.join("main.py");
    fs::write(&py, &compiled.code).unwrap();
    let run = Command::new("python3").arg(&py).output().expect("run python3");
    assert!(
        run.status.success(),
        "{name}: python3 failed:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    String::from_utf8_lossy(&run.stdout).into_owned()
}
