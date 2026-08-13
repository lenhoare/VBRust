//! Regression tests for four bugs the help system's rustc gate surfaced
//! (2026-08-13): `Round(x, places)`, `HashMap[key]` reads, a mutating method in
//! an initialiser / if-let condition not marking the local `mut`, and a
//! single-line `Match` arm rejecting a trailing comment.

/// Strip all whitespace so assertions don't depend on rustfmt spacing.
fn packed(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

fn rust_of(src: &str) -> String {
    let c = vbr::compile(src);
    assert!(!c.has_errors, "unexpected errors:\n{:?}", c.diagnostics);
    c.rust
}

#[test]
fn round_with_decimal_places() {
    let rust = rust_of("Function Main()\n    Debug.Print Round(3.14159, 2)\nEnd Function\n");
    // Scale, round, unscale — not an undefined free `round(x, n)`.
    assert!(packed(&rust).contains("powi"), "round-2 should scale: {rust}");
    assert!(packed(&rust).contains(".round()"), "round-2 should round: {rust}");
}

#[test]
fn hashmap_index_by_literal_and_variable() {
    let src = "Function Main()\n\
        \x20   Dim ages As HashMap<String, Long>\n\
        \x20   ages.Insert(\"Ada\", 36)\n\
        \x20   Dim who As String = \"Ada\"\n\
        \x20   Debug.Print ages[\"Ada\"]\n\
        \x20   Debug.Print ages[who]\n\
        End Function\n";
    let rust = packed(&rust_of(src));
    // A string key must NOT be cast to usize; a variable key is borrowed.
    assert!(!rust.contains("asusize"), "map key wrongly cast to usize: {rust}");
    assert!(rust.contains("ages[who]") || rust.contains("ages[&who]"), "variable key: {rust}");
}

#[test]
fn mutating_call_in_initialiser_marks_mut() {
    let rust = packed(&rust_of(
        "Function Main()\n\
        \x20   Dim v As Vec<Long> = [1, 2, 3]\n\
        \x20   Dim last As Option<Long> = v.Pop()\n\
        \x20   Debug.Print v.Len()\n\
        End Function\n",
    ));
    assert!(rust.contains("letmutv"), "v should be mut (it is popped): {rust}");
}

#[test]
fn mutating_call_in_if_let_condition_marks_mut() {
    let rust = packed(&rust_of(
        "Function Main()\n\
        \x20   Dim w As Vec<Long> = [5, 6]\n\
        \x20   If w.Pop() Is Some(x) Then\n\
        \x20       Debug.Print x\n\
        \x20   End If\n\
        End Function\n",
    ));
    assert!(rust.contains("letmutw"), "w should be mut (it is popped): {rust}");
}

#[test]
fn get_first_last_return_owned_values() {
    // Read-in-place accessors hand back the *value*: `.copied()` for a scalar,
    // `.cloned()` for a String — so `.Unwrap_Or` works uniformly, no `&T` leak.
    let rust = packed(&rust_of(
        "Function Main()\n\
        \x20   Dim nums As Vec<Long> = [10, 20]\n\
        \x20   Debug.Print nums.Get(0).Unwrap_Or(-1)\n\
        \x20   Dim words As Vec<String> = [\"a\", \"b\"]\n\
        \x20   Debug.Print words.First().Unwrap_Or(\"?\")\n\
        End Function\n",
    ));
    assert!(rust.contains(".get(0).copied()"), "scalar get → copied: {rust}");
    assert!(rust.contains(".first().cloned()"), "string first → cloned: {rust}");
    // The string-literal default is owned so it fits the `Option<String>`.
    assert!(rust.contains("\"?\".to_string()"), "unwrap_or default owned: {rust}");
}

#[test]
fn match_arm_accepts_trailing_comment() {
    // The single-line arm form used to reject a trailing `' comment`.
    let src = "Function Main()\n\
        \x20   Match 2\n\
        \x20       1 => Debug.Print \"one\"     ' first\n\
        \x20       _ => Debug.Print \"other\"   ' fallback\n\
        \x20   End Match\n\
        End Function\n";
    let c = vbr::compile(src);
    assert!(!c.has_errors, "match arm comment should parse: {:?}", c.diagnostics);
}
