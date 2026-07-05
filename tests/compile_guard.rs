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
const GUARDED: &[&str] =
    &["datetime_json", "dataframe_join", "showcase", "gui_event_stdlib", "tui_monitor"];

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
            "· web_counter skipped — install the target with \
             `rustup target add wasm32-unknown-unknown` to guard the web backend"
        );
        return;
    }
    let vbr = Command::new(env!("CARGO_BIN_EXE_vbr"))
        .arg("build")
        .arg(examples.join("web_counter.vbr"))
        .output()
        .expect("failed to run vbr");
    assert!(
        vbr.status.success(),
        "vbr build failed for web_counter:\n{}",
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
        "cargo rejected the generated web project:\n{stderr}"
    );
    assert!(
        !stderr.contains("warning:"),
        "cargo emitted warnings for web_counter:\n{stderr}"
    );
    eprintln!("✔ web_counter compiled clean (wasm32)");
}
