//! Slice 5 — ownership/coercion smoothings found while testing example apps.
//! A HashMap key variable borrows like a literal; a returned Vec element clones;
//! a Long constant widens into a Double (and mixes with floats).

use vbr::compile;

fn main_body(body: &str) -> String {
    format!("Function Main()\n{}\nEnd Function\n", body)
}

/// The generated Rust with all whitespace removed, so assertions don't depend on
/// whether rustfmt was available to format it.
fn packed(rust: &str) -> String {
    rust.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn map_get_borrows_a_string_variable_key() {
    // `get`/`contains_key` take `&K`; a String *variable* key must be borrowed
    // (a literal already is a `&str`).
    let c = compile(&main_body(
        "    Dim ages As HashMap<String, Long>\n\
         \x20   ages.insert(\"Ada\", 30)\n\
         \x20   Dim who As String = \"Ada\"\n\
         \x20   If ages.contains_key(who) Then\n\
         \x20       Debug.Print ages.get(who).Unwrap()\n\
         \x20   End If",
    ));
    assert!(!c.has_errors, "{:?}", c.diagnostics);
    let r = packed(&c.rust);
    assert!(r.contains("contains_key(&who)"), "got: {}", c.rust);
    assert!(r.contains("get(&who)"), "got: {}", c.rust);
}

#[test]
fn byval_string_param_key_is_not_double_borrowed() {
    // A ByVal String param is already a `&str` — borrowing it again would be
    // `&&str` and fail `String: Borrow<&str>`.
    let c = compile(
        "Function Knows(ByVal m As HashMap<String, Long>, ByVal who As String) As Boolean\n\
         \x20   Return m.contains_key(who)\n\
         End Function\n\
         Function Main()\n\
         \x20   Dim m As HashMap<String, Long>\n\
         \x20   Debug.Print Knows(m, \"x\")\n\
         End Function\n",
    );
    assert!(!c.has_errors, "{:?}", c.diagnostics);
    // No `& who` — the param is already borrowed.
    assert!(packed(&c.rust).contains("contains_key(who)"), "got: {}", c.rust);
}

#[test]
fn return_of_a_vec_element_is_cloned() {
    // Can't move a String out of `names[0]` — it clones.
    let c = compile(
        "Function First(ByVal names As Vec<String>) As String\n\
         \x20   Return names[0]\n\
         End Function\n\
         Function Main()\n\
         \x20   Dim xs As Vec<String> = [\"a\", \"b\"]\n\
         \x20   Debug.Print First(xs)\n\
         End Function\n",
    );
    assert!(!c.has_errors, "{:?}", c.diagnostics);
    assert!(packed(&c.rust).contains("[0].clone()"), "got: {}", c.rust);
}

#[test]
fn numeric_vec_element_return_is_not_cloned() {
    // A Long is Copy — moving it out of an index is fine, no needless clone.
    let c = compile(
        "Function First(ByVal xs As Vec<Long>) As Long\n\
         \x20   Return xs[0]\n\
         End Function\n\
         Function Main()\n\
         \x20   Dim xs As Vec<Long> = [1, 2]\n\
         \x20   Debug.Print First(xs)\n\
         End Function\n",
    );
    assert!(!c.has_errors, "{:?}", c.diagnostics);
    assert!(!c.rust.contains("clone"), "should not clone a Copy element: {}", c.rust);
}

#[test]
fn long_const_widens_into_a_double() {
    let c = compile(
        "Public Const K As Long = 32\n\
         Function Main()\n\
         \x20   Dim d As Double = K\n\
         \x20   Debug.Print d\n\
         End Function\n",
    );
    assert!(!c.has_errors, "{:?}", c.diagnostics);
    assert!(c.rust.contains("as f64"), "expected a widening cast: {}", c.rust);
}

#[test]
fn const_mixes_with_a_float_in_arithmetic() {
    // `i64 * f64` must widen the const side.
    let c = compile(
        "Public Const K As Long = 32\n\
         Function Main()\n\
         \x20   Dim d As Double = K * 0.5\n\
         \x20   Debug.Print d\n\
         End Function\n",
    );
    assert!(!c.has_errors, "{:?}", c.diagnostics);
}
