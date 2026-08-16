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
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;

/// Slice 1: pure computation + strings + `= Nothing`.
/// Slice 2: `Type`/struct, methods (`Me`→`self->`), module `Const`.
/// Slice 3: `Match`/`Enum` — C `enum` + tagged unions + if-chain lowering.
/// Slice 4: collections — monomorphised `Vec`/`HashMap`, iterator loops.
/// Slice 5: `Option`/`Result`/`?` — struct wrappers, propagation, `.Unwrap()`.
const C: &[&str] = &[
    // slice 1
    "hello", "functions", "logic", "maths", "rnd", "doloop", "memory",
    // slice 2
    "types", "structs", "methods", "constants",
    // slice 3
    "match", "match_guards", "enums", "sum_types",
    // slice 4 (deterministic)
    "vec", "list_literal", "iterators",
    // slice 5 — Option / Result / `?`
    "option", "result", "result_e", "result_unit", "if_let",
];

/// Collection examples whose runtime *order* isn't reproducible against Rust
/// (a `HashMap` iterates in Rust's randomised order, ours in insertion order),
/// so the generated C is snapshotted and merely compiled + run, not diffed.
const C_SNAPSHOT_ONLY: &[&str] = &["hashmap"];

/// Standard-library examples. `rustc` alone can't link the stdlib, so the Rust
/// ground truth is a stored `.out` (captured from `vbr runproject`); the C is
/// compiled, run, and diffed against it — the same discipline as the deterministic
/// examples, just with the reference precomputed.
const C_STDLIB: &[&str] = &["filesystem", "datetime_basics", "stdlib", "shell"];

/// Standard-library examples that vendor a C library, so the output is a *project
/// folder* (`main.c` + the bundled sources + a `Makefile`) rather than a single
/// `.c`. Built by compiling `main.c` alongside the vendored sources, then diffed
/// against the stored `.out` — the same `vbr runproject` ground truth.
const C_STDLIB_PROJECT: &[&str] = &["json_basics", "database"];

/// Project stdlib examples whose output isn't reproducible (network), so they're
/// snapshotted + built/linked but NOT run — the same situation as Python's
/// `PY_STDLIB_NORUN`. The transport itself is exercised against a loopback server
/// in `c_http_roundtrip`.
const C_STDLIB_PROJECT_NORUN: &[&str] = &["http_post"];

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
    for name in C
        .iter()
        .chain(C_SNAPSHOT_ONLY)
        .chain(C_STDLIB)
        .chain(C_STDLIB_PROJECT)
        .chain(C_STDLIB_PROJECT_NORUN)
    {
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

/// Project stdlib examples: emit `main.c`, bundle the vendored sources, build
/// them together with the `Makefile`'s link flags, run, and diff stdout against
/// the stored `.out`. Skips without `cc`.
#[test]
fn c_stdlib_project_matches_out() {
    if Command::new("cc").arg("--version").output().is_err() {
        eprintln!("skipping c_stdlib_project_matches_out: no cc");
        return;
    }
    for name in C_STDLIB_PROJECT {
        let result = vbr::compile_c(&read_example(name));
        assert!(!result.has_errors, "{name} (c) errors: {:?}", result.diagnostics);
        assert!(result.is_project(), "{name}: expected a vendored project");
        let dir = std::env::temp_dir().join(format!("vbr_c_proj_{name}"));
        let bin = build_c_project(&result, &dir);
        let run = Command::new(&bin).current_dir(&dir).output().expect("run c binary");
        let out = String::from_utf8_lossy(&run.stdout).into_owned();
        check_snapshot(name, "out", &out);
    }
}

/// Network project examples: build + link them (proving the code is valid C and
/// the link flag resolves), but don't run — a real URL isn't reproducible. Skips
/// without `cc`.
#[test]
fn c_stdlib_project_norun_compiles() {
    if Command::new("cc").arg("--version").output().is_err() {
        eprintln!("skipping c_stdlib_project_norun_compiles: no cc");
        return;
    }
    for name in C_STDLIB_PROJECT_NORUN {
        let result = vbr::compile_c(&read_example(name));
        assert!(!result.has_errors, "{name} (c) errors: {:?}", result.diagnostics);
        assert!(result.is_project(), "{name}: expected a project (link flags)");
        let dir = std::env::temp_dir().join(format!("vbr_c_norun_{name}"));
        let _ = build_c_project(&result, &dir);
    }
}

/// The `Http` transport end-to-end against a one-shot loopback server — GET and
/// POST (body + `Authorization` header echoed back) — since a real Http program
/// can't be diffed against Rust. Proves the libcurl runtime actually works. Skips
/// without `cc`.
#[test]
fn c_http_roundtrip() {
    if Command::new("cc").arg("--version").output().is_err() {
        eprintln!("skipping c_http_roundtrip: no cc");
        return;
    }
    // GET: the server replies a fixed body.
    let get_url = serve_once(|_req| "hello-from-c-get".to_string());
    let get_src = format!(
        "Function Main()\n    Debug.Print Http.Get(\"{get_url}\")\nEnd Function\n"
    );
    let out = compile_build_run(&get_src, "http_get");
    assert_eq!(out, "hello-from-c-get\n", "GET roundtrip");

    // POST: the server echoes the body and the Authorization header back.
    let post_url = serve_once(|req| {
        let body = req.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
        let auth = req
            .lines()
            .find_map(|l| l.strip_prefix("Authorization: "))
            .unwrap_or("")
            .to_string();
        format!("posted:{body}|auth:{auth}")
    });
    let post_src = format!(
        "Function Main()\n    \
         Dim headers As HashMap<String, String>\n    \
         headers.insert(\"Authorization\", \"Bearer xyz\")\n    \
         Debug.Print Http.Post(\"{post_url}\", \"body123\", headers)\nEnd Function\n"
    );
    let out = compile_build_run(&post_src, "http_post_rt");
    assert_eq!(out, "posted:body123|auth:Bearer xyz\n", "POST roundtrip");
}

/// A one-shot loopback HTTP server: accept one connection, read the full request
/// (headers + any body per `Content-Length`), and reply `reply(request)` as the
/// 200 body. Returns its `http://127.0.0.1:PORT/` URL. Hermetic — no external
/// network — mirroring the Rust stdlib's own `serve_once`.
fn serve_once<F>(reply: F) -> String
where
    F: Fn(&str) -> String + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut req = Vec::new();
            let mut chunk = [0u8; 1024];
            loop {
                let n = stream.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    break;
                }
                req.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&req);
                // Once the headers are in, read the declared body then stop.
                if let Some(hdr_end) = text.find("\r\n\r\n") {
                    let want = text
                        .lines()
                        .find_map(|l| l.strip_prefix("Content-Length: "))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if req.len() >= hdr_end + 4 + want {
                        break;
                    }
                }
            }
            let body = reply(&String::from_utf8_lossy(&req));
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    format!("http://127.0.0.1:{}/", port)
}

/// Compile `src` to a C project, build it (with its link flags), run it, and
/// return stdout — for the `Http` roundtrip's generated programs.
fn compile_build_run(src: &str, tag: &str) -> String {
    let result = vbr::compile_c(src);
    assert!(!result.has_errors, "{tag} (c) errors: {:?}", result.diagnostics);
    assert!(result.warnings.is_empty(), "{tag} warned: {:?}", result.warnings);
    let dir = std::env::temp_dir().join(format!("vbr_c_{tag}"));
    let bin = build_c_project(&result, &dir);
    let run = Command::new(&bin).current_dir(&dir).output().expect("run c binary");
    String::from_utf8_lossy(&run.stdout).into_owned()
}

/// Write a C project (`main.c` + any vendored sources), build it with `cc` and
/// its link flags, and return the built binary's path.
fn build_c_project(result: &vbr::CCompiled, dir: &Path) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("main.c"), &result.code).unwrap();
    let csupport = Path::new(env!("CARGO_MANIFEST_DIR")).join("csupport");
    let mut sources = vec![dir.join("main.c")];
    for base in &result.vendored {
        for ext in ["c", "h"] {
            let f = format!("{base}.{ext}");
            fs::copy(csupport.join(&f), dir.join(&f)).unwrap();
        }
        sources.push(dir.join(format!("{base}.c")));
    }
    let bin = dir.join("main_bin");
    let mut cc = Command::new("cc");
    cc.args(&sources).arg("-o").arg(&bin);
    for flag in &result.link_flags {
        cc.arg(format!("-l{flag}"));
    }
    let built = cc.output().expect("cc");
    assert!(
        built.status.success(),
        "cc failed:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );
    bin
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
