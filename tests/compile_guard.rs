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
