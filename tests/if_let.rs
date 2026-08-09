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
