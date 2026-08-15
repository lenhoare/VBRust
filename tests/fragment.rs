//! Fragment mode (`compile_fragment`) — the core of embedding Bust in Rust. A
//! fragment is a statement list, not a whole program; it's transpiled by
//! wrapping in `Function Main()`, compiling, and lifting out the body. These
//! tests pin that it produces bare, dedented Rust statements (no `fn` wrapper),
//! leaves `Dim`med names in scope, and reports errors instead of Rust.

use vbr::compile_fragment;

#[test]
fn plain_statements_become_a_bare_block() {
    let frag = compile_fragment("Dim total As Long = 0\nDim i As Long\nFor i = 1 To 10\n    total = total + i\nNext");
    assert!(!frag.has_errors, "{:?}", frag.diagnostics);
    // No function wrapper — just the statements, dedented to column 0.
    assert!(!frag.rust.contains("fn main"));
    assert!(frag.rust.starts_with("let mut total: i64 = 0;"));
    assert!(frag.rust.contains("for i in 1..=10 {"));
    // A `Dim`med variable lowers to a `let` the surrounding Rust can then use.
    assert!(frag.rust.contains("let mut total"));
}

#[test]
fn an_unknown_name_passes_through_for_rustc_to_check() {
    // COHERENCE GUARD: `compute` isn't defined in Bust — at the embedding seam it's
    // the surrounding Rust, so fragment mode must emit the call and let rustc
    // check it. If this ever fails, something (likely task #24, an "unknown
    // function" diagnostic) has started rejecting unknown names WITHOUT exempting
    // fragments — which breaks `vbr embed`. See the note in resolver.rs's Call arm.
    let frag = compile_fragment("Dim r As Long = compute(3)");
    assert!(!frag.has_errors, "{:?}", frag.diagnostics);
    assert!(frag.rust.contains("compute(3)"), "got: {}", frag.rust);

    // The same must hold for an unknown *variable* read in from the host Rust.
    let var = compile_fragment("Dim doubled As Long = host_value * 2");
    assert!(!var.has_errors, "{:?}", var.diagnostics);
    assert!(var.rust.contains("host_value"), "got: {}", var.rust);
}

#[test]
fn a_syntax_error_yields_diagnostics_not_rust() {
    let frag = compile_fragment("For i = 1 To 10\n    Debug.Print i");
    assert!(frag.has_errors);
    assert!(frag.rust.is_empty());
    assert!(!frag.diagnostics.is_empty());
}

#[test]
fn error_line_numbers_are_the_fragments_own() {
    // The error is on the fragment's line 2 — it must report line 2, not 3
    // (the internal `Function Main()` wrapper must not leak into the numbers).
    let frag = compile_fragment("Dim a As Long = 1\nReturn Return");
    assert!(frag.has_errors);
    assert!(
        frag.diagnostics.iter().any(|d| d.contains("[line 2]")),
        "expected a [line 2] diagnostic, got: {:?}",
        frag.diagnostics
    );
}
