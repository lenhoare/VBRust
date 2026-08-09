//! `If <expr> Is <pattern> Then …` — VB-flavoured `if let`. The Rust backend
//! renders a real `if let`; the block or single-line form both work.

use vbr::compile;

fn with_getter(body: &str) -> String {
    format!(
        "Function Get() As Option<Long>\n    Return Some(5)\nEnd Function\n\
         Function Main()\n{}\nEnd Function\n",
        body
    )
}

#[test]
fn if_is_lowers_to_a_real_if_let() {
    let c = compile(&with_getter(
        "    If Get() Is Some(v) Then\n        Debug.Print v\n    End If",
    ));
    assert!(!c.has_errors, "{:?}", c.diagnostics);
    assert!(c.rust.contains("if let Some ( v ) ="), "got: {}", c.rust);
    // Not a plain `match` — the whole point is the `if let` idiom.
    assert!(!c.rust.contains("match "), "should be if let, not match: {}", c.rust);
}

#[test]
fn single_line_if_is_works() {
    let c = compile(&with_getter("    If Get() Is Some(v) Then Debug.Print v"));
    assert!(!c.has_errors, "{:?}", c.diagnostics);
    assert!(c.rust.contains("if let Some ( v ) ="), "got: {}", c.rust);
}

#[test]
fn if_is_with_else() {
    let c = compile(&with_getter(
        "    If Get() Is Some(v) Then\n        Debug.Print v\n    Else\n        Debug.Print 0\n    End If",
    ));
    assert!(!c.has_errors, "{:?}", c.diagnostics);
    assert!(c.rust.contains("if let Some ( v ) ="), "got: {}", c.rust);
    assert!(c.rust.contains("} else {"), "expected an else block: {}", c.rust);
}

#[test]
fn do_while_is_lowers_to_while_let() {
    // `Do While <expr> Is <pattern>` desugars to `loop { if let … else break }`.
    let c = compile(&with_getter(
        "    Do While Get() Is Some(v)\n        Debug.Print v\n    Loop",
    ));
    assert!(!c.has_errors, "{:?}", c.diagnostics);
    assert!(c.rust.contains("loop {"), "got: {}", c.rust);
    assert!(c.rust.contains("if let Some ( v ) ="), "got: {}", c.rust);
    assert!(c.rust.contains("break;"), "the else arm should break: {}", c.rust);
}
