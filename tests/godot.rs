//! Tests for the **Godot** backend (slice 1: a `Node2D` → a gdext GDExtension).
//!
//! A Godot program compiles to a **cdylib** the Godot editor loads, so there is
//! no stdout to diff (the C/Python discipline doesn't apply). Two guarantees:
//!   1. the generated Rust has the right gdext shape (always-on, cheap), and
//!   2. it actually compiles against the real `godot` crate as a cdylib —
//!      opt-in behind `VBR_GODOT_BUILD=1`, since that pulls the (large) crate.

use std::fs;
use std::path::Path;
use std::process::Command;

/// Transpile the bundled `examples/godot_player.vbr` and return its Rust.
fn player_rust() -> String {
    let src = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/godot_player.vbr"),
    )
    .expect("read godot_player.vbr");
    let out = vbr::compile(&src);
    assert!(!out.has_errors, "godot_player.vbr should transpile cleanly:\n{:?}", out.diagnostics);
    out.rust
}

/// The moving-square example lowers to the gdext shape the probe proved: a
/// `GodotClass` extending `Node2D`, the `ExtensionLibrary` entry point, an
/// `#[export]` field, the two lifecycle callbacks, and the two lowering rules
/// (a class `use`, and a hoisted `set_position`).
#[test]
fn godot_player_has_gdext_shape() {
    let rust = player_rust();
    for needle in [
        "use godot::prelude::*;",
        "use godot::classes::Input;",       // rule 1: a class needs its `use`
        "unsafe impl ExtensionLibrary for", // the entry stub
        "#[derive(GodotClass)]",
        "#[class(base = Node2D)]",
        "impl INode2D for Player",
        "#[export]",
        "speed: f32",
        "fn ready(&mut self)",
        "fn process(&mut self, delta: f64)",
        "let delta = delta as f32;",        // Godot's f64 delta → the Single `delta`
        "Input::singleton().is_action_pressed(\"ui_right\")",
        "self.base().get_position()",       // property read
        "self.base_mut().set_position(",    // rule 2: hoisted property write
    ] {
        assert!(rust.contains(needle), "generated Rust missing `{needle}`:\n{rust}");
    }
}

/// The real proof: the generated Rust compiles as a gdext cdylib. Opt-in —
/// `VBR_GODOT_BUILD=1 cargo test --test godot` — because building it downloads
/// and compiles the `godot` crate (large, slow the first time).
#[test]
fn godot_player_compiles_as_cdylib() {
    if std::env::var("VBR_GODOT_BUILD").is_err() {
        eprintln!("skipping godot_player_compiles_as_cdylib (set VBR_GODOT_BUILD=1 to run)");
        return;
    }
    let rust = player_rust();
    let dir = std::env::temp_dir().join("vbr_godot_build");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"vbr_godot_build\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n\
         [lib]\ncrate-type = [\"cdylib\"]\n\n[dependencies]\ngodot = \"0.5\"\n",
    )
    .unwrap();
    fs::write(dir.join("src/lib.rs"), &rust).unwrap();

    let ok = Command::new("cargo")
        .arg("build")
        .current_dir(&dir)
        .status()
        .expect("run cargo build")
        .success();
    let _ = fs::remove_dir_all(&dir);
    assert!(ok, "the generated gdext cdylib should compile");
}

/// `vbr rungodot` assembles a loadable Godot 4 project: `project.godot`, a
/// `.gdextension` pointing at the built library, a starter scene using the node
/// class, and a `rust/` crate that builds to the `.so` the manifest names.
/// Opt-in (`VBR_GODOT_BUILD=1`) — it builds the cdylib. Godot itself isn't
/// needed: with none on PATH the command still exits 0 after building.
#[test]
fn rungodot_assembles_a_loadable_project() {
    if std::env::var("VBR_GODOT_BUILD").is_err() {
        eprintln!("skipping rungodot_assembles_a_loadable_project (set VBR_GODOT_BUILD=1 to run)");
        return;
    }
    let work = std::env::temp_dir().join("vbr_rungodot_test");
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&work).unwrap();
    let src = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/godot_player.vbr"),
    )
    .unwrap();
    let vbr_file = work.join("game.vbr");
    fs::write(&vbr_file, src).unwrap();

    // Stub the handoff with `/bin/true` so the test is deterministic whether or
    // not a real Godot is installed (it just exits 0 in place of the editor).
    let ok = Command::new(env!("CARGO_BIN_EXE_vbr"))
        .arg("rungodot")
        .arg(&vbr_file)
        .env("GODOT4_BIN", "/bin/true")
        .status()
        .expect("run vbr rungodot")
        .success();
    assert!(ok, "rungodot should succeed and hand off cleanly");

    let proj = work.join("game_godot");
    let gdext = fs::read_to_string(proj.join("game.gdextension")).expect("read .gdextension");
    assert!(gdext.contains("entry_symbol = \"gdext_rust_init\""), "gdext init symbol:\n{gdext}");
    assert!(
        gdext.contains("res://rust/target/debug/libgame.so"),
        "gdextension should point at the built .so:\n{gdext}"
    );
    let scene = fs::read_to_string(proj.join("main.tscn")).expect("read main.tscn");
    assert!(scene.contains("type=\"Player\""), "scene should use the node class:\n{scene}");
    assert!(proj.join("project.godot").exists(), "project.godot should exist");
    assert!(
        proj.join("rust/target/debug/libgame.so").exists(),
        "the cdylib the manifest names should be built"
    );
    let _ = fs::remove_dir_all(&work);
}
