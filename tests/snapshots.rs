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
    "sub",
    "types",
    "match",
    "match_guards",
    "single_line_if",
    "string_funcs",
    "maths",
    "byref",
    "compound_assign",
    "coercion",
    "result",
    "result_e",
    "result_unit",
    "option",
    "firstclass_types",
    "vec",
    "list_literal",
    "hashmap",
    "doloop",
    "structs",
    "field_inference",
    "enums",
    "sum_types",
    "enum_payloads",
    "methods",
    "constants",
    "iterators",
    "iterator_strings",
    "iterator_more",
    "tuples",
    "struct_params",
    "arrays",
    "coercion_more",
    "coercions",
    "string_args",
    "string_param",
    "string_coercions",
    "rust_string_methods",
    "rust_number_methods",
    "rust_vec_methods",
    "mid_and_date",
    "terminal_io",
    "inline_rust",
    "opaque_handle",
    "logic",
    "grid_logic",
];

/// Programs whose Rust output and notes we snapshot, but which we don't compile
/// here — they need a feature not yet built, or an external crate (vbr_stdlib)
/// that our rustc-only compile check can't link.
const TRANSPILE_ONLY: &[&str] =
    &["string_options", "stdlib", "datetime_json", "http_post", "tui_post", "database", "tui_ideas", "counter", "greeting", "settings", "fetch", "view_if", "toggle_progress", "radio_choice", "notes", "spacing", "dracula", "converter", "await_fn", "logo", "canvas", "plot", "gui_layout", "showcase", "gui_event_stdlib", "tui_counter", "tui_layout", "tui_list", "tui_panels", "tui_table", "tui_input", "tui_tabs", "tui_dashboard", "tui_chart", "tui_multichart", "tui_fetch", "tui_monitor", "tui_pulse", "tui_life", "python_scalar", "python_handle", "python_tuple", "dataframe_basics", "dataframe_groupby", "dataframe_join", "web_counter", "web_greeting", "web_settings", "web_fetch", "web_dracula"];

/// `Screen` programs also compiled for the browser (`vbr runweb` → Ratzilla):
/// the same example file, second snapshot. The State struct and `view` are
/// byte-identical to the native output; only `fn main` (the shell) differs.
const WEB_SCREENS: &[&str] = &["tui_counter", "tui_input", "tui_pulse", "tui_monitor"];

/// Files that are meant to fail, exercising the teaching diagnostics.
const ERRORS: &[&str] = &[
    "ownership_error",
    "currency_error",
    "variant_error",
    "date_error",
    "rnd_error",
    "on_error",
    "try_no_result_error",
    "struct_no_init",
    "array_access_error",
    "redim_error",
    "byref_literal_error",
    "ignored_result_error",
    "string_write_error",
    "format_error",
    "with_error",
    "option_base_error",
    "global_error",
    "handle_value_error",
    "generic_error",
    "closure_value_error",
    "closure_capture_error",
    "use_no_version_error",
    "blocking_no_await_error",
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
fn web_screen_variants_match_snapshots() {
    for name in WEB_SCREENS {
        let result = vbr::compile_web(&read_example(name));
        assert!(
            !result.has_errors,
            "{name} (web) unexpectedly produced errors: {:?}",
            result.diagnostics
        );
        check_snapshot(name, "web.rs", &result.rust);
        check_snapshot(name, "web.diag", &result.diagnostics.join("\n"));
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

/// A multifile project: cross-module qualified calls, `mod` declarations, and
/// `pub` items. We snapshot each generated file, then compile them together as
/// one crate to prove the qualified paths and visibility actually link.
#[test]
fn multifile_project_compiles() {
    let proj = examples_dir().join("geometry_project");
    let main_src = fs::read_to_string(proj.join("main.vbr")).unwrap();
    let shapes_src = fs::read_to_string(proj.join("shapes.vbr")).unwrap();
    let modules = vec![vbr::module_name("shapes")];
    // Pass 1: the sibling's interface, so qualified calls get the full local
    // argument treatment (`&` on ByVal collections/strings, `&mut` on ByRef).
    let mut interfaces = vbr::resolver::ProjectInterfaces::new();
    interfaces.insert(vbr::module_name("shapes"), vbr::module_interface(&shapes_src));

    let main_rs = vbr::compile_module(&main_src, &modules, &interfaces, true);
    let shapes_rs = vbr::compile_module(&shapes_src, &modules, &interfaces, false);
    assert!(!main_rs.has_errors, "main.vbr errors: {:?}", main_rs.diagnostics);
    assert!(!shapes_rs.has_errors, "shapes.vbr errors: {:?}", shapes_rs.diagnostics);

    check_snapshot("geometry_main", "rs", &main_rs.rust);
    check_snapshot("geometry_shapes", "rs", &shapes_rs.rust);

    let dir = std::env::temp_dir().join("vbr_multifile");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("shapes.rs"), &shapes_rs.rust).unwrap();
    let main_path = dir.join("main.rs");
    fs::write(&main_path, &main_rs.rust).unwrap();

    let output = Command::new("rustc")
        .arg("--edition")
        .arg("2021")
        .arg("-o")
        .arg(dir.join("bin"))
        .arg(&main_path)
        .output()
        .expect("failed to run rustc");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "rustc rejected multifile project:\n{stderr}");
    assert!(stderr.trim().is_empty(), "rustc emitted warnings:\n{stderr}");
}

/// Cross-module *interfaces* (pass 1 of the project compile): collections,
/// strings, and constants cross the module boundary with the full local
/// argument treatment — `&mut` for a ByRef Vec, `&` for ByVal Vec/String,
/// `crate::module::CONST` for a `Public Const`. We snapshot both files,
/// rustc-compile them as one crate, and check the visibility diagnostics
/// (private function / private constant / unknown member).
#[test]
fn crossmodule_interfaces_compile() {
    let proj = examples_dir().join("life_project");
    let main_src = fs::read_to_string(proj.join("main.vbr")).unwrap();
    let life_src = fs::read_to_string(proj.join("life.vbr")).unwrap();
    let modules = vec![vbr::module_name("life")];
    let mut interfaces = vbr::resolver::ProjectInterfaces::new();
    interfaces.insert(vbr::module_name("life"), vbr::module_interface(&life_src));

    let main_rs = vbr::compile_module(&main_src, &modules, &interfaces, true);
    let life_rs = vbr::compile_module(&life_src, &modules, &interfaces, false);
    assert!(!main_rs.has_errors, "main.vbr errors: {:?}", main_rs.diagnostics);
    assert!(!life_rs.has_errors, "life.vbr errors: {:?}", life_rs.diagnostics);

    check_snapshot("life_main", "rs", &main_rs.rust);
    check_snapshot("life_life", "rs", &life_rs.rust);

    let dir = std::env::temp_dir().join("vbr_crossmodule");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("life.rs"), &life_rs.rust).unwrap();
    let main_path = dir.join("main.rs");
    fs::write(&main_path, &main_rs.rust).unwrap();

    let output = Command::new("rustc")
        .arg("--edition")
        .arg("2021")
        .arg("-o")
        .arg(dir.join("bin"))
        .arg(&main_path)
        .output()
        .expect("failed to run rustc");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "rustc rejected the project:\n{stderr}");
    assert!(stderr.trim().is_empty(), "rustc emitted warnings:\n{stderr}");

    // The other side of visibility: private members earn teaching errors, and
    // an unknown member points at the parentheses rule.
    let bad = "Function Main()\n    Debug.Print Life.Hidden()\n    Debug.Print Life.SECRET\n    Debug.Print Life.Wdith\nEnd Function\n";
    let bad_rs = vbr::compile_module(bad, &modules, &interfaces, true);
    assert!(bad_rs.has_errors);
    let all = bad_rs.diagnostics.join("\n");
    for expect in ["'Hidden' is Private", "'SECRET' is Private", "no public constant 'Wdith'"] {
        assert!(all.contains(expect), "missing diagnostic {expect:?} in:\n{all}");
    }
}

/// A mixed project: a `.vbr` entry calling a hand-written `.rs` module. The `.rs`
/// file is included verbatim; we snapshot the generated entry, then compile the
/// two together to prove the qualified call into hand-written Rust links.
#[test]
fn mixed_rs_project_compiles() {
    let proj = examples_dir().join("mixed_project");
    let main_src = fs::read_to_string(proj.join("main.vbr")).unwrap();
    let text_rs = fs::read_to_string(proj.join("text.rs")).unwrap();
    let modules = vec![vbr::module_name("text")];
    // A verbatim `.rs` module has no VBR interface to harvest — its calls stay
    // name-qualified, matching the Rust side by hand.
    let interfaces = vbr::resolver::ProjectInterfaces::new();

    let main_rs = vbr::compile_module(&main_src, &modules, &interfaces, true);
    assert!(!main_rs.has_errors, "main.vbr errors: {:?}", main_rs.diagnostics);
    check_snapshot("mixed_main", "rs", &main_rs.rust);

    let dir = std::env::temp_dir().join("vbr_mixed");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("text.rs"), &text_rs).unwrap();
    let main_path = dir.join("main.rs");
    fs::write(&main_path, &main_rs.rust).unwrap();

    let output = Command::new("rustc")
        .arg("--edition")
        .arg("2021")
        .arg("-o")
        .arg(dir.join("bin"))
        .arg(&main_path)
        .output()
        .expect("failed to run rustc");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "rustc rejected mixed project:\n{stderr}");
    assert!(stderr.trim().is_empty(), "rustc emitted warnings:\n{stderr}");
}

/// `Use <crate> <version>` declarations become Cargo dependencies. We can't
/// `rustc`-link an external crate here, so we check the parsed deps and snapshot
/// the generated Rust (the inline block) rather than compiling it.
#[test]
fn use_declares_dependencies() {
    let result = vbr::compile(&read_example("dice"));
    assert!(!result.has_errors, "dice errors: {:?}", result.diagnostics);
    assert_eq!(
        result.dependencies,
        vec![("rand".to_string(), "0.8".to_string())]
    );
    check_snapshot("dice", "rs", &result.rust);
}
