//! End-to-end coverage of the `vbr test` harness itself.
//!
//! The rest of the suite snapshots the *transpiled Rust* of a `Test` block; this
//! is the one place the whole flow is exercised as a user runs it — the
//! `<module>.test.vbr` sibling discovery, the `cargo test` build, and the `✓ / ✗`
//! runner with its exit code. Gated on `cargo` (the harness shells out to it).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The freshly-built `vbr` binary (Cargo hands integration tests its path).
fn vbr() -> Command {
    Command::new(env!("CARGO_BIN_EXE_vbr"))
}

fn have_cargo() -> bool {
    Command::new("cargo").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Run `vbr test <dir>` and return (success, merged stdout+stderr). The runner
/// prints its report to stderr, so both streams are combined for assertions.
fn run_vbr_test(dir: &Path) -> (bool, String) {
    let out = vbr().arg("test").arg(dir).output().expect("run vbr test");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), combined)
}

/// The bundled `examples/receipt/` project passes, by description, and exits 0.
#[test]
fn receipt_example_tests_pass() {
    if !have_cargo() {
        eprintln!("skipping receipt_example_tests_pass: no cargo");
        return;
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/receipt");
    let (ok, out) = run_vbr_test(&dir);
    assert!(ok, "vbr test on examples/receipt should succeed:\n{out}");
    assert!(out.contains("4 passed"), "expected `4 passed`:\n{out}");
    assert!(
        out.contains("a line total multiplies unit price by quantity"),
        "the report should echo the test descriptions:\n{out}"
    );
    assert!(!out.contains('✗'), "no test should fail:\n{out}");
}

/// A deliberately-wrong `Assert` is reported as a failure and makes `vbr test`
/// exit non-zero (so it fails a CI gate). Built in a temp project so nothing in
/// the repo is left broken.
#[test]
fn a_failing_assert_exits_non_zero() {
    if !have_cargo() {
        eprintln!("skipping a_failing_assert_exits_non_zero: no cargo");
        return;
    }
    let dir: PathBuf = std::env::temp_dir().join("vbr_harness_fail");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    // A logic module, a thin entry, and a spec that asserts something false.
    fs::write(
        dir.join("calc.vbr"),
        "Public Function Add(ByVal a As Long, ByVal b As Long) As Long\n    Return a + b\nEnd Function\n",
    )
    .unwrap();
    fs::write(
        dir.join("main.vbr"),
        "Function Main()\n    Debug.Print Calc.Add(1, 2)\nEnd Function\n",
    )
    .unwrap();
    fs::write(
        dir.join("calc.test.vbr"),
        "Test \"two plus two is wrong on purpose\"\n    Assert Calc.Add(2, 2) = 5\nEnd Test\n",
    )
    .unwrap();

    let (ok, out) = run_vbr_test(&dir);
    let _ = fs::remove_dir_all(&dir);
    assert!(!ok, "a failing test must make `vbr test` exit non-zero:\n{out}");
    assert!(
        out.contains('✗') || out.to_lowercase().contains("fail"),
        "the failure should be reported:\n{out}"
    );
}
