//! Snapshot tests for the VBR transpiler.
//!
//! Each example in `examples/` is locked against a stored snapshot:
//!   * happy-path programs   → their generated Rust (`tests/snapshots/<name>.rs`)
//!   * intentional-error files → their diagnostics  (`tests/snapshots/<name>.diag`)
//!
//! Regenerate snapshots after an intended change with:
//!     UPDATE_SNAPSHOTS=1 cargo test
//!
//! A third test feeds every happy-path output back through `rustc` to prove it
//! is valid, warning-free Rust — the strongest guarantee we can make.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Programs that must transpile cleanly and compile.
const HAPPY: &[&str] = &[
    "hello",
    "strings",
    "functions",
    "types",
    "select",
    "string_funcs",
    "maths",
    "byref",
    "coercion",
    "result",
    "option",
    "vec",
    "hashmap",
    "doloop",
    "structs",
    "methods",
    "constants",
    "iterators",
    "tuples",
    "struct_params",
    "arrays",
    "coercion_more",
    "string_args",
];

/// Programs whose Rust output and notes we snapshot, but which we don't compile
/// because they rely on features not built yet (e.g. Option/Result handling).
const TRANSPILE_ONLY: &[&str] = &["string_options"];

/// Files that are meant to fail, exercising the teaching diagnostics.
const ERRORS: &[&str] = &[
    "ownership_error",
    "sub_error",
    "currency_error",
    "variant_error",
    "select_no_else",
    "rnd_error",
    "on_error",
    "struct_no_init",
    "array_access_error",
    "redim_error",
    "byref_literal_error",
    "ignored_result_error",
    "format_error",
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

/// Compare `actual` against the stored snapshot, or write it when updating.
fn check_snapshot(name: &str, ext: &str, actual: &str) {
    let path = snapshots_dir().join(format!("{name}.{ext}"));
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        fs::create_dir_all(snapshots_dir()).unwrap();
        fs::write(&path, actual).unwrap();
        return;
    }
    let expected = fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing snapshot {} — run `UPDATE_SNAPSHOTS=1 cargo test` to create it",
            path.display()
        )
    });
    assert_eq!(
        actual, expected,
        "snapshot mismatch for {name}.{ext} — rerun with UPDATE_SNAPSHOTS=1 if this change is intended"
    );
}

#[test]
fn happy_paths_match_snapshots() {
    for name in HAPPY.iter().chain(TRANSPILE_ONLY) {
        let result = vbr::compile(&read_example(name));
        assert!(
            !result.has_errors,
            "{name} unexpectedly produced errors: {:?}",
            result.diagnostics
        );
        check_snapshot(name, "rs", &result.rust);
    }
}

#[test]
fn transpile_only_notes_match_snapshots() {
    for name in TRANSPILE_ONLY {
        let result = vbr::compile(&read_example(name));
        check_snapshot(name, "diag", &result.diagnostics.join("\n"));
    }
}

#[test]
fn error_examples_match_snapshots() {
    for name in ERRORS {
        let result = vbr::compile(&read_example(name));
        assert!(
            result.has_errors,
            "{name} was expected to fail but produced no errors"
        );
        check_snapshot(name, "diag", &result.diagnostics.join("\n"));
    }
}

#[test]
fn happy_outputs_compile_without_warnings() {
    for name in HAPPY {
        let result = vbr::compile(&read_example(name));
        let dir = std::env::temp_dir().join(format!("vbr_snap_{name}"));
        fs::create_dir_all(&dir).unwrap();
        let rs = dir.join("out.rs");
        fs::write(&rs, &result.rust).unwrap();

        let output = Command::new("rustc")
            .arg("--edition")
            .arg("2021")
            .arg("-o")
            .arg(dir.join("out_bin"))
            .arg(&rs)
            .output()
            .expect("failed to run rustc");

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "rustc rejected generated Rust for {name}:\n{stderr}"
        );
        assert!(
            stderr.trim().is_empty(),
            "rustc emitted warnings for {name}:\n{stderr}"
        );
    }
}
