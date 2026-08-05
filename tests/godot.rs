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

/// Transpile a bundled `examples/<name>.vbr` and return its Rust.
fn example_rust(name: &str) -> String {
    let src = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("examples/{name}.vbr")),
    )
    .unwrap_or_else(|_| panic!("read {name}.vbr"));
    let out = vbr::compile(&src);
    assert!(!out.has_errors, "{name}.vbr should transpile cleanly:\n{:?}", out.diagnostics);
    out.rust
}

/// Build `rust` as a gdext cdylib in a fresh temp crate; return whether it
/// compiled. Opt-in behind `VBR_GODOT_BUILD=1` (pulls the large `godot` crate).
fn compiles_as_cdylib(name: &str, rust: &str) -> bool {
    let dir = std::env::temp_dir().join(format!("vbr_godot_{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(
        dir.join("Cargo.toml"),
        format!(
            "[package]\nname = \"vbr_godot_{name}\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n\
             [lib]\ncrate-type = [\"cdylib\"]\n\n[dependencies]\ngodot = \"0.5\"\n"
        ),
    )
    .unwrap();
    fs::write(dir.join("src/lib.rs"), rust).unwrap();
    let ok = Command::new("cargo")
        .arg("build")
        .current_dir(&dir)
        .status()
        .expect("run cargo build")
        .success();
    let _ = fs::remove_dir_all(&dir);
    ok
}

/// The moving-square example lowers to the gdext shape the probe proved: a
/// `GodotClass` extending `Node2D`, the `ExtensionLibrary` entry point, an
/// `#[export]` field, the two lifecycle callbacks, and the two lowering rules
/// (a class `use`, and a hoisted `set_position`).
#[test]
fn godot_player_has_gdext_shape() {
    let rust = example_rust("godot_player");
    for needle in [
        "use godot::prelude::*;",
        "use godot::classes::{INode2D, Input, Node2D};", // base + I-trait + singleton
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

/// Slice 2: a different base class and the general passthrough — any `Me.<Prop>`
/// is a property, `Me.<Method>()` a base-class method, `Input.GetAxis` snake-cased
/// generally, and unary minus folded so it survives without the resolver.
#[test]
fn godot_runner_has_slice2_shape() {
    let rust = example_rust("godot_runner");
    for needle in [
        "use godot::classes::{CharacterBody2D, ICharacterBody2D, Input};",
        "#[class(base = CharacterBody2D)]",
        "impl ICharacterBody2D for Runner",
        "fn physics_process(&mut self, delta: f64)",
        "self.base().get_velocity()",             // general property read
        "self.base_mut().set_velocity(",          // general property write (hoisted)
        "self.base_mut().move_and_slide()",       // base-class method
        "self.base_mut().is_on_floor()",          // base-class method (as condition)
        "Input::singleton().get_axis(",           // general snake_case (not `getaxis`)
        "Input::singleton().is_action_just_pressed(",
        "0.0 - self.jumpforce",                   // `-JumpForce`: resolver widens the 0 (like core VBR)
    ] {
        assert!(rust.contains(needle), "generated Rust missing `{needle}`:\n{rust}");
    }
}

/// Slice 3: signals. `Signal` declares them in a second, inherent `#[godot_api]
/// impl` (`#[signal] fn …`); `Emit` fires them through gdext's typed API, hoisting
/// args past the `self.signals()` borrow.
#[test]
fn godot_signal_has_slice3_shape() {
    let rust = example_rust("godot_signal");
    for needle in [
        "#[godot_api]\nimpl Pinger {",   // the second, inherent impl
        "#[signal]",
        "fn pinged(count: i64);",        // typed payload
        "self.signals().pinged().emit(", // typed emit
        "let __vbr_a0 = self.count;",    // args hoisted past the borrow
    ] {
        assert!(rust.contains(needle), "generated Rust missing `{needle}`:\n{rust}");
    }
}

/// The real proof: the examples compile as gdext cdylibs. Opt-in —
/// `VBR_GODOT_BUILD=1 cargo test --test godot` — because building pulls the
/// (large) `godot` crate.
#[test]
fn godot_examples_compile_as_cdylib() {
    if std::env::var("VBR_GODOT_BUILD").is_err() {
        eprintln!("skipping godot_examples_compile_as_cdylib (set VBR_GODOT_BUILD=1 to run)");
        return;
    }
    for name in ["godot_player", "godot_runner", "godot_signal"] {
        assert!(compiles_as_cdylib(name, &example_rust(name)), "{name} cdylib");
    }
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
