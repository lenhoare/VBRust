//! A-series quirks, slice 2 — three standalone bugs.
//! (Q16, a local variable shadowing a sibling module name, needs a multi-module
//! project to reproduce and is covered by manual verification + the resolver
//! guard in the `Module.method` arm.)

use vbr::compile;

/// Q3 — an `Enum` gets a `Display` impl (delegating to `Debug`), so an enum value
/// can be `Debug.Print`ed or `&`-concatenated the way VB prints enums.
#[test]
fn an_enum_can_be_printed() {
    let src = "Enum Light\n    Red\n    Green\nEnd Enum\n\
               Function Main()\n    Dim l As Light = Light.Red\n    Debug.Print l\nEnd Function\n";
    let compiled = compile(src);
    assert!(!compiled.has_errors, "{:?}", compiled.diagnostics);
    assert!(
        compiled.rust.contains("impl std::fmt::Display for Light"),
        "expected a Display impl: {}",
        compiled.rust
    );
}

/// Q28 — indexing a `For Each` binding over a `Vec<Vec<_>>` parenthesises the
/// deref'd receiver: `(*line)[0]`, not the mis-parsed `*line[0]` (= `*(line[0])`,
/// a deref of an `i64`).
#[test]
fn indexing_a_nested_vec_for_each_binding_parenthesises_the_deref() {
    let src = "Function Main()\n    Dim lines As Vec<Vec<Long>> = [[0, 1], [2, 3]]\n\
               \x20   For Each line In lines\n        Dim a As Long = line[0]\n        Debug.Print a\n    Next\n\
               End Function\n";
    let compiled = compile(src);
    assert!(!compiled.has_errors, "{:?}", compiled.diagnostics);
    assert!(
        compiled.rust.contains("(*line)[0]"),
        "the deref receiver must be parenthesised: {}",
        compiled.rust
    );
}
