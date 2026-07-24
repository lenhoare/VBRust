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

/// The single-file inlined prelude (`src/python.rs`) and the project prelude
/// (`vbrpy/prelude.py`) must stay identical — otherwise `Some`/`Ok`/`Err` would
/// be different classes across the two modes and `isinstance` would break. Emit
/// a program that triggers every helper, and assert its distinctive lines also
/// appear in `vbrpy/prelude.py`.
#[test]
fn inlined_prelude_matches_vbrpy() {
    let src = "\
Function Maker() As Result<Long>
    Return Ok(1)
End Function
Function Half(ByVal n As Long) As Option<Long>
    Return None
End Function
Function Main()
    Dim r As Double = 2.5
    Debug.Print \"\" & Round(r)
    Dim k As Long = Maker().Unwrap()
    Match Half(4)
        Some(v) => Debug.Print \"\" & v
        None => Debug.Print \"none\"
    End Match
End Function
";
    let inlined = vbr::compile_python(src);
    assert!(!inlined.has_errors, "prelude probe errors: {:?}", inlined.diagnostics);
    let vbrpy = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("vbrpy/prelude.py"),
    )
    .expect("read vbrpy/prelude.py");

    for line in [
        "return \"true\" if x else \"false\"",
        "raise Exception(f'unwrapped an Err: {x.error}')",
        "_math.floor(x + 0.5) if x >= 0 else _math.ceil(x - 0.5)",
        "    value: object",
        "    error: object",
    ] {
        assert!(
            inlined.code.contains(line),
            "single-file prelude missing distinctive line: {line:?}"
        );
        assert!(
            vbrpy.contains(line),
            "vbrpy/prelude.py drifted from the inlined prelude — missing: {line:?}"
        );
    }
}

/// Standard-library examples: emitted as a *project* (main.py + vbrpy/). Their
/// generated `main.py` is snapshotted, and their runtime stdout is checked
/// against a stored `.out` — verified byte-for-byte against `vbr runproject`
/// (the Rust ground truth) when the snapshots were taken. Kept as a stored
/// output rather than a live Rust run so the suite needn't compile `vbr_stdlib`.
const PY_STDLIB: &[&str] = &["stdlib", "json_basics", "database", "datetime_basics"];

/// Stdlib examples that are transpiled + snapshotted but NOT run: their output
/// isn't reproducible (network, wall-clock, …), so there's no stdout to diff —
/// same situation as `hashmap`. The shim itself is exercised separately (e.g.
/// `vbrpy_http_roundtrip`).
const PY_STDLIB_NORUN: &[&str] = &[
    "http_post",
    // DataFrame: `.Print()` renders a polars table (formatting differs from
    // Rust), and polars is a pip install — so snapshot the code, and check
    // behaviour separately (`python_dataframe_runs`, gated on polars).
    "dataframe_basics",
    "dataframe_groupby",
    "dataframe_join",
];

#[test]
fn python_stdlib_snapshot_only() {
    for name in PY_STDLIB_NORUN {
        let result = vbr::compile_python(&read_example(name));
        assert!(!result.has_errors, "{name} errors: {:?}", result.diagnostics);
        assert!(result.warnings.is_empty(), "{name} warned: {:?}", result.warnings);
        assert!(
            !result.stdlib_used.is_empty(),
            "{name} should use the stdlib (project mode)"
        );
        check_snapshot(name, "py", &result.code);
    }
}

/// Exercise the `vbrpy.Http` shim end-to-end against a loopback server — GET,
/// POST (body + headers echoed back), and the error path — since a real Http
/// program can't be diffed against Rust. Pure-Python: proves the shim works.
#[test]
fn vbrpy_http_roundtrip() {
    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping vbrpy_http_roundtrip: no python3");
        return;
    }
    let vbrpy = Path::new(env!("CARGO_MANIFEST_DIR")).join("vbrpy");
    let dir = std::env::temp_dir().join("vbr_http_rt");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    copy_dir(&vbrpy, &dir.join("vbrpy"));

    let probe = r#"
import threading, http.server
from vbrpy import Http, Ok, Err

class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200); self.end_headers()
        self.wfile.write(b"hello-get")
    def do_POST(self):
        n = int(self.headers.get("Content-Length", 0))
        data = self.rfile.read(n).decode()
        auth = self.headers.get("Authorization", "")
        self.send_response(200); self.end_headers()
        self.wfile.write(("posted:" + data + "|auth:" + auth).encode())
    def log_message(self, *a): pass

srv = http.server.HTTPServer(("127.0.0.1", 0), H)
base = "http://127.0.0.1:%d" % srv.server_address[1]
threading.Thread(target=srv.serve_forever, daemon=True).start()

g = Http.get(base + "/")
assert isinstance(g, Ok) and g.value == "hello-get", g
p = Http.post(base + "/", "body123", {"Authorization": "Bearer xyz"})
assert isinstance(p, Ok) and p.value == "posted:body123|auth:Bearer xyz", p
e = Http.get("not-a-valid-url")
assert isinstance(e, Err), e
print("OK")
"#;
    fs::write(dir.join("probe.py"), probe).unwrap();
    let run = Command::new("python3")
        .arg("probe.py")
        .current_dir(&dir)
        .output()
        .expect("run python3");
    let out = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success() && out.contains("OK"),
        "http roundtrip failed:\nstdout: {out}\nstderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
}

/// Run the generated `dataframe_basics` against the real `people.csv` (gated on
/// polars being installed) and check the deterministic, non-table lines. The
/// `.Print()` table is skipped — its formatting differs from Rust-polars — but
/// the computed values (row count, first kept name) prove the lowered formulas,
/// filter and column extraction actually work.
#[test]
fn python_dataframe_runs() {
    let has_polars = Command::new("python3")
        .args(["-c", "import polars"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !has_polars {
        eprintln!("skipping python_dataframe_runs: polars not installed");
        return;
    }
    let result = vbr::compile_python(&read_example("dataframe_basics"));
    assert!(!result.has_errors, "errors: {:?}", result.diagnostics);
    assert!(result.warnings.is_empty(), "warned: {:?}", result.warnings);

    let dir = std::env::temp_dir().join("vbr_df_run");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("main.py"), &result.code).unwrap();
    copy_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("vbrpy"), &dir.join("vbrpy"));
    fs::copy(examples_dir().join("people.csv"), dir.join("people.csv")).unwrap();

    let run = Command::new("python3").arg("main.py").current_dir(&dir).output().unwrap();
    let out = String::from_utf8_lossy(&run.stdout);
    assert!(run.status.success(), "python failed:\n{}", String::from_utf8_lossy(&run.stderr));
    for line in ["loaded 5 rows, 5 columns", "first kept: Alice", "wrote out.csv"] {
        assert!(out.contains(line), "missing {line:?} in output:\n{out}");
    }
}

#[test]
fn python_stdlib_projects() {
    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipping python_stdlib_projects: no python3");
        return;
    }
    let vbrpy = Path::new(env!("CARGO_MANIFEST_DIR")).join("vbrpy");
    for name in PY_STDLIB {
        let result = vbr::compile_python(&read_example(name));
        assert!(!result.has_errors, "{name} errors: {:?}", result.diagnostics);
        assert!(result.warnings.is_empty(), "{name} warned: {:?}", result.warnings);
        assert!(
            !result.stdlib_used.is_empty(),
            "{name} should use the stdlib (project mode), but stdlib_used is empty"
        );
        check_snapshot(name, "py", &result.code);

        // Lay out the project in a temp dir and run it.
        let dir = std::env::temp_dir().join(format!("vbr_pystd_{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("main.py"), &result.code).unwrap();
        copy_dir(&vbrpy, &dir.join("vbrpy"));

        let run = Command::new("python3")
            .arg("main.py")
            .current_dir(&dir)
            .output()
            .expect("run python3");
        assert!(
            run.status.success(),
            "{name}: python3 failed:\n{}",
            String::from_utf8_lossy(&run.stderr)
        );
        let out = String::from_utf8_lossy(&run.stdout).into_owned();
        check_snapshot(name, "out", &out);
    }
}

/// Recursively copy a directory (the `vbrpy` package into the test project).
fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().flatten() {
        let p = entry.path();
        let dest = to.join(entry.file_name());
        if p.is_dir() {
            copy_dir(&p, &dest);
        } else {
            fs::copy(&p, &dest).unwrap();
        }
    }
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
