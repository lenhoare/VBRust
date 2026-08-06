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

/// Regression: a local mutated through a user `&mut self` method inside a
/// `Test`/`Assert` must compile. The test-emission path used to discard the
/// resolver's mutable-lend set (unlike a normal function), so `acc.Deposit(...)`
/// left `acc` as `let acc` and rustc refused it ("cannot borrow as mutable").
#[test]
fn a_mutating_method_on_a_local_compiles_in_a_test() {
    if !have_cargo() {
        eprintln!("skipping a_mutating_method_on_a_local_compiles_in_a_test: no cargo");
        return;
    }
    let dir: PathBuf = std::env::temp_dir().join("vbr_harness_mut_method");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    // A struct with a &mut self method, a thin entry, and a spec that calls the
    // mutating method on a local and checks the effect.
    fs::write(
        dir.join("account.vbr"),
        "Public Type Account\n    Public balance As Long\nEnd Type\n\n\
         Public Function Account.Deposit(ByVal amount As Long)\n    \
         Me.balance = Me.balance + amount\nEnd Function\n",
    )
    .unwrap();
    fs::write(
        dir.join("main.vbr"),
        "Function Main()\n    Debug.Print \"ok\"\nEnd Function\n",
    )
    .unwrap();
    fs::write(
        dir.join("account.test.vbr"),
        "Test \"a mutating method on a local compiles\"\n    \
         Dim acc As Account = Account { balance: 0 }\n    \
         acc.Deposit(50)\n    Assert acc.balance = 50\nEnd Test\n",
    )
    .unwrap();

    let (ok, out) = run_vbr_test(&dir);
    let _ = fs::remove_dir_all(&dir);
    assert!(ok, "the mutating-method test should build and pass:\n{out}");
    assert!(out.contains("1 passed"), "expected `1 passed`:\n{out}");
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
