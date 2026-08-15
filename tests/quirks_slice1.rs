//! Quick-win fixes from the A-series example-project testing (slice 1).
//! Each test pins one previously-broken shape so it can't silently regress.

use vbr::compile;

/// Quirk 31 — `vbCrLf` in a `&` chain must emit the escape `\r\n`, not a raw CR
/// byte (which rustc rejects: "bare CR not allowed in string").
#[test]
fn vbcrlf_emits_an_escaped_carriage_return() {
    let src = "Function Main()\n    Debug.Print \"a\" & vbCrLf & \"b\"\nEnd Function\n";
    let compiled = compile(src);
    assert!(!compiled.has_errors, "{:?}", compiled.diagnostics);
    // The generated literal carries the two-char escape, never a real CR.
    assert!(compiled.rust.contains("\\r\\n"), "got: {}", compiled.rust);
    assert!(!compiled.rust.contains('\r'), "a raw CR leaked into the output");
}

/// Quirk 18/32 — a Bust name that collides with a Rust keyword (`Move`, and a
/// `move` parameter) must be emitted as a raw identifier `r#move`, not the bare
/// keyword (which won't parse).
#[test]
fn a_rust_keyword_name_becomes_a_raw_identifier() {
    let src = "Function Move(ByVal move As Long) As Long\n    Return move\nEnd Function\n\
               Function Main()\n    Debug.Print Move(3)\nEnd Function\n";
    let compiled = compile(src);
    assert!(!compiled.has_errors, "{:?}", compiled.diagnostics);
    assert!(compiled.rust.contains("r#move"), "got: {}", compiled.rust);
    // And the internal `self` receiver must NOT be escaped (regression guard).
    assert!(!compiled.rust.contains("self_"), "the self receiver got escaped");
}

/// Quirk 14 — matching a `String` scrutinee against `"…"` (`&str`) patterns is
/// lowered through `.as_str()` so the arms unify.
#[test]
fn a_string_match_lowers_the_scrutinee_through_as_str() {
    // An *owned* String scrutinee (from `LCase`) — a `&str` ByVal param wouldn't
    // (and mustn't) get `.as_str()`, since that method is unstable on `&str`.
    let src = "Function Classify(ByVal ch As String) As String\n\
               \x20   Dim lower As String = LCase(ch)\n\
               \x20   Match lower\n        \"a\" => Return \"vowel\"\n        _ => Return \"other\"\n    End Match\n\
               End Function\n";
    let compiled = compile(src);
    assert!(!compiled.has_errors, "{:?}", compiled.diagnostics);
    assert!(compiled.rust.contains(".as_str()"), "got: {}", compiled.rust);
}

/// Quirk 17/23/27 — a Bust keyword used as a name gives a targeted diagnostic
/// (naming the keyword), not the cryptic "expected a name, found To".
#[test]
fn a_keyword_used_as_a_parameter_name_is_explained() {
    let src = "Function Move(ByVal n As Long, ByVal to As Long)\nEnd Function\n";
    let compiled = compile(src);
    assert!(compiled.has_errors);
    assert!(
        compiled.diagnostics.iter().any(|d| d.contains("keyword") && d.contains("To")),
        "expected a keyword diagnostic naming `To`, got: {:?}",
        compiled.diagnostics
    );
}
