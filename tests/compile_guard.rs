//! Compile guard for the transpile-only examples.
//!
//! The snapshot suite proves single-file programs compile via rustc, but the
//! stdlib/GUI/TUI examples need a cargo project (crates to link), so their
//! snapshots alone can't prove the generated code builds — twice that blind
//! spot has hidden real regressions (the stdlib method-name break, the
//! `.cloned()` gap). This test builds one representative example per
//! heavyweight backend for real.
//!
//! It compiles Iced/polars/ratatui, so it is `#[ignore]`d in the default run.
//! Run it explicitly (first build is slow; cached rebuilds take seconds):
//!
//!     cargo test -- --ignored
//!
//! It builds into the shared `examples/build/` project sequentially, exactly
//! as `vbr build` does, so it reuses the same warm target cache.

use std::path::Path;
use std::process::Command;

/// One example per backend: stdlib wrappers (Json + DateTime multi-word
/// methods), dataframes (polars, formulas, joins, IsNull), GUI (the full Iced
/// control set + async `Await`), stdlib-inside-a-GUI-event (the event bodies
/// run the resolver, so `now.AddDays(30)` maps to `add_days`), and TUI
/// (ratatui charts, timers, async). Inline-Python examples are left out —
/// they link libpython, which not every machine has.
const GUARDED: &[&str] = &[
    "datetime_json",
    "dataframe_join",
    "showcase",
    "gui_event_stdlib",
    "tui_monitor",
    // `Http.Post` with headers: blocking in `Main` (http_post), and awaited off
    // the UI thread in a Screen event (tui_post) — the LLM-call shape.
    "http_post",
    "tui_post",
    // SQLite: the Database handle (bundled rusqlite), Json rows, `?` chaining,
    // the list literal as params, and a `&Database` function param.
    "database",
    // A Database held in a Screen's State: the fallible `init()` constructor
    // (built before the terminal starts, clean bail-out on Err), events using
    // the handle via `state.db`, and the file-top stdlib-type imports.
    "tui_ideas",
    // Loops inside an Event (the state rewrite recurses into For/For Each/Do
    // bodies) and a State field initialised by a call with ByVal arguments.
    "tui_life",
    // A focusable `List` nested inside a view `Match` arm — the focusable
    // collector recurses into Match/If, so the widget's `<field>_state` is
    // declared, inited, and key-wired even when it isn't top-level.
    "tui_list_tabs",
    // Shell: run-and-capture in Main plus a background Process behind a
    // Screen (fallible Shell.Start in State, IsRunning/Kill from events).
    "shell",
    "tui_shell",
];

#[test]
#[ignore = "builds Iced/polars/ratatui — run with `cargo test -- --ignored`"]
fn transpile_only_examples_compile() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let build = examples.join("build");
    for name in GUARDED {
        let vbr = Command::new(env!("CARGO_BIN_EXE_vbr"))
            .arg("build")
            .arg(examples.join(format!("{name}.vbr")))
            .output()
            .expect("failed to run vbr");
        assert!(
            vbr.status.success(),
            "vbr build failed for {name}:\n{}",
            String::from_utf8_lossy(&vbr.stderr)
        );

        let out = Command::new("cargo")
            .arg("build")
            .current_dir(&build)
            .output()
            .expect("failed to run cargo");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            out.status.success(),
            "cargo rejected the generated project for {name}:\n{stderr}"
        );
        // Cargo suppresses warnings from registry dependencies, so any
        // `warning:` here comes from us — the generated code or vbr_stdlib
        // (a path dependency). The bar is the same as the rustc snapshots:
        // warning-free.
        assert!(
            !stderr.contains("warning:"),
            "cargo emitted warnings for {name}:\n{stderr}"
        );
        eprintln!("✔ {name} compiled clean");
    }

    // The web backend builds for wasm32 — guard it too, when the target is
    // installed (skip with a notice otherwise, like the Python examples).
    let wasm_ready = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .any(|l| l.trim() == "wasm32-unknown-unknown")
        })
        .unwrap_or(false);
    if !wasm_ready {
        eprintln!(
            "· web examples skipped — install the target with \
             `rustup target add wasm32-unknown-unknown` to guard the web backend"
        );
        return;
    }
    // The web examples cover the whole Page surface between them:
    // web_greeting the input round-trip (TextInput/Checkbox, payload messages,
    // the web-sys dep); web_settings the view logic (Match/If, Slider,
    // ProgressBar); web_fetch async (`Await Http.Get` → send_future + the
    // gloo-net fetch wrapper).
    // (web_dracula adds nothing at the Rust level beyond classes — its Theme/Css
    // land in index.html — so the wasm builds stop at web_fetch.)
    for name in ["web_greeting", "web_settings", "web_fetch"] {
        let vbr = Command::new(env!("CARGO_BIN_EXE_vbr"))
            .arg("build")
            .arg(examples.join(format!("{name}.vbr")))
            .output()
            .expect("failed to run vbr");
        assert!(
            vbr.status.success(),
            "vbr build failed for {name}:\n{}",
            String::from_utf8_lossy(&vbr.stderr)
        );
        let out = Command::new("cargo")
            .args(["build", "--target", "wasm32-unknown-unknown"])
            .current_dir(&build)
            .output()
            .expect("failed to run cargo");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            out.status.success(),
            "cargo rejected the generated web project for {name}:\n{stderr}"
        );
        assert!(
            !stderr.contains("warning:"),
            "cargo emitted warnings for {name}:\n{stderr}"
        );
        eprintln!("✔ {name} compiled clean (wasm32)");
    }

    // A `Screen` also runs in the browser (`vbr runweb` → Ratzilla, ratatui
    // 0.30). tui_counter covers the web shell (keymap + events); tui_dashboard
    // proves the chart/gauge widget lowering — written against native ratatui
    // 0.29 — also compiles against 0.30 on wasm; tui_input covers the focus
    // machinery (Input + List, Tab cycling, Enter dispatch) in the browser
    // key handler; tui_pulse covers `Every` timers (gloo-timers Intervals and
    // the RefCell-guard reborrow that lets one statement touch two fields);
    // tui_monitor covers async — `Await Http.Get` split into a spawn_local
    // future over the browser's fetch, from a timer.
    for name in ["tui_counter", "tui_dashboard", "tui_input", "tui_pulse", "tui_monitor"] {
        let vbr = Command::new(env!("CARGO_BIN_EXE_vbr"))
            .args(["build", "--web"])
            .arg(examples.join(format!("{name}.vbr")))
            .output()
            .expect("failed to run vbr");
        assert!(
            vbr.status.success(),
            "vbr build --web failed for {name}:\n{}",
            String::from_utf8_lossy(&vbr.stderr)
        );
        let out = Command::new("cargo")
            .args(["build", "--target", "wasm32-unknown-unknown"])
            .current_dir(&build)
            .output()
            .expect("failed to run cargo");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            out.status.success(),
            "cargo rejected the generated web-screen project for {name}:\n{stderr}"
        );
        assert!(
            !stderr.contains("warning:"),
            "cargo emitted warnings for {name}:\n{stderr}"
        );
        eprintln!("✔ {name} compiled clean (wasm32, ratzilla)");
    }

    // Two real project builds (folders, not single examples):
    // - idea-engine: a Screen with a Database + Json config in fallible State,
    //   an awaited Http.Post to an LLM, the Json builder, list-literal params.
    // - life_screen: a Screen in main.vbr driving logic in life.vbr — the
    //   surfaces-join-projects shape (`mod life;`, cross-module calls from
    //   State inits and events with full argument treatment).
    for name in ["projects/idea-engine", "examples/life_screen"] {
        let proj = Path::new(env!("CARGO_MANIFEST_DIR")).join(name);
        let vbr = Command::new(env!("CARGO_BIN_EXE_vbr"))
            .arg("build")
            .arg(&proj)
            .output()
            .expect("failed to run vbr");
        assert!(
            vbr.status.success(),
            "vbr build failed for {name}:\n{}",
            String::from_utf8_lossy(&vbr.stderr)
        );
        let out = Command::new("cargo")
            .arg("build")
            .current_dir(proj.join("build"))
            .output()
            .expect("failed to run cargo");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success(), "cargo rejected {name}:\n{stderr}");
        assert!(!stderr.contains("warning:"), "cargo emitted warnings for {name}:\n{stderr}");
        eprintln!("✔ {name} compiled clean");
    }
    // A folder project's data files ride along into build/ (the program's
    // working directory) — the idea engine's config.json must be there, and
    // its README.md (docs, not data) must not.
    let engine_build = Path::new(env!("CARGO_MANIFEST_DIR")).join("projects/idea-engine/build");
    assert!(
        engine_build.join("config.json").exists(),
        "config.json was not copied into the idea-engine build/"
    );
    assert!(
        !engine_build.join("README.md").exists(),
        "README.md (docs) should not be copied into build/"
    );

    // The playground — the transpiler itself compiled to WebAssembly behind a
    // Yew UI (playground/). Guarding it keeps `vbr::compile` wasm-clean: a new
    // dependency that doesn't build for wasm32 would break it silently.
    let out = Command::new("cargo")
        .args(["build", "--target", "wasm32-unknown-unknown"])
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("playground"))
        .output()
        .expect("failed to run cargo");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "cargo rejected the playground:\n{stderr}"
    );
    assert!(
        !stderr.contains("warning:"),
        "cargo emitted warnings for the playground:\n{stderr}"
    );
    eprintln!("✔ playground compiled clean (wasm32)");
}

/// `vbr test` end to end: the runner parses `Test`/`Assert`, emits `#[test]`
/// functions, runs `cargo test`, and translates the result back to VBR terms.
/// Covers the single-file form (the `tests.vbr` example) and the `.test.vbr`
/// sibling placement — including a deliberate failure, to prove the operand
/// values and `.vbr` line come through.
#[test]
#[ignore = "runs cargo test on generated projects — run with `cargo test -- --ignored`"]
fn vbr_test_runs_specs() {
    use std::fs;
    let vbr = env!("CARGO_BIN_EXE_vbr");

    // 1. The single-file example: all four specs pass.
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/tests.vbr");
    let single = std::env::temp_dir().join(format!("vbr_test_single_{}", std::process::id()));
    let _ = fs::remove_dir_all(&single);
    fs::create_dir_all(&single).unwrap();
    fs::copy(&example, single.join("main.vbr")).unwrap();
    let out = Command::new(vbr).arg("test").arg(&single).output().expect("run vbr test");
    let report = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "vbr test (single) exited non-zero:\n{report}");
    assert!(report.contains("4 passed"), "expected 4 passed:\n{report}");
    assert!(
        report.contains("multiples of three are fizz"),
        "expected the description in the report:\n{report}"
    );

    // 2. A project with a `.test.vbr` sibling: one spec passes, one fails. The
    //    failure must name the `.test.vbr` line and show the operand values, and
    //    the process must exit non-zero (usable in CI).
    let proj = std::env::temp_dir().join(format!("vbr_test_proj_{}", std::process::id()));
    let _ = fs::remove_dir_all(&proj);
    fs::create_dir_all(&proj).unwrap();
    fs::write(
        proj.join("life.vbr"),
        "Public Function StepCell(ByVal alive As Boolean, ByVal neighbours As Long) As Boolean\n\
         \x20   If alive Then\n\
         \x20       Return neighbours = 2 Or neighbours = 3\n\
         \x20   End If\n\
         \x20   Return neighbours = 3\n\
         End Function\n",
    )
    .unwrap();
    fs::write(
        proj.join("main.vbr"),
        "Function Main()\n    Debug.Print CStr(Life.StepCell(True, 2))\nEnd Function\n",
    )
    .unwrap();
    fs::write(
        proj.join("life.test.vbr"),
        "Test \"a live cell with two neighbours survives\"\n\
         \x20   Assert Life.StepCell(True, 2) = True\n\
         End Test\n\n\
         Test \"a lone cell wrongly expected to survive\"\n\
         \x20   Assert Life.StepCell(True, 0) = True\n\
         End Test\n",
    )
    .unwrap();
    let out = Command::new(vbr).arg("test").arg(&proj).output().expect("run vbr test");
    let report = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "a failing suite must exit non-zero:\n{report}");
    assert!(report.contains("1 passed, 1 failed"), "expected 1 passed, 1 failed:\n{report}");
    assert!(report.contains("left:  false"), "expected operand values:\n{report}");
    assert!(report.contains("life.test.vbr:6"), "expected the .test.vbr line:\n{report}");

    // 3. A plain `vbr build` of that project excludes the `.test.vbr` file and is
    //    warning-free (tested-only logic must not leak into the app build).
    let out = Command::new(vbr).arg("build").arg(&proj).output().expect("run vbr build");
    assert!(out.status.success(), "vbr build failed:\n{}", String::from_utf8_lossy(&out.stderr));
    let built = Command::new("cargo").arg("build").current_dir(proj.join("build")).output().unwrap();
    let stderr = String::from_utf8_lossy(&built.stderr);
    assert!(built.status.success(), "cargo build failed:\n{stderr}");
    assert!(!stderr.contains("warning:"), "the app build should be warning-free:\n{stderr}");

    let _ = fs::remove_dir_all(&single);
    let _ = fs::remove_dir_all(&proj);
    eprintln!("✔ vbr test ran specs (single-file + .test.vbr, pass and fail)");
}
