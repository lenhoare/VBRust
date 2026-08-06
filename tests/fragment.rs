//! Fragment mode (`compile_fragment`) — the core of embedding VBR in Rust. A
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
    // `compute` isn't defined in VBR — at the embedding seam it's assumed to be
    // Rust in scope. Fragment mode should still emit the call (rustc checks it).
    let frag = compile_fragment("Dim r As Long = compute(3)");
    assert!(!frag.has_errors, "{:?}", frag.diagnostics);
    assert!(frag.rust.contains("compute(3)"), "got: {}", frag.rust);
}

#[test]
fn a_syntax_error_yields_diagnostics_not_rust() {
    let frag = compile_fragment("For i = 1 To 10\n    Debug.Print i");
    assert!(frag.has_errors);
    assert!(frag.rust.is_empty());
    assert!(!frag.diagnostics.is_empty());
}
