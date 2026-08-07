//! VB6 `/` semantics: `/` is floating-point division (never integer), and the
//! payload of a returned `Ok(x)`/`Some(x)` narrows to the function's declared
//! inner type — so `Ok(a / b)` in a `Result<Long>` compiles.

use vbr::compile;

/// `Long / Long` divides as floats, so `25 / 100` is `0.25`, not `0`. Both
/// operands are cast to `f64` (concrete, so the quotient can take `.floor()`).
#[test]
fn slash_is_floating_point_division() {
    let src = "Function Main()\n    Dim done As Long = 25\n    Dim total As Long = 100\n\
               \x20   Dim p As Double = done / total\n    Debug.Print p\nEnd Function\n";
    let compiled = compile(src);
    assert!(!compiled.has_errors, "{:?}", compiled.diagnostics);
    assert!(
        compiled.rust.contains("done as f64") && compiled.rust.contains("total as f64"),
        "operands should be promoted to f64: {}",
        compiled.rust
    );
}

/// Storing a `/` result into a `Long` narrows with a truncating cast — Rust wins,
/// so `7 / 2` is `3`, not VB6's rounded `4`.
#[test]
fn dividing_into_a_long_truncates() {
    let src = "Function Main()\n    Dim n As Long = 7 / 2\n    Debug.Print n\nEnd Function\n";
    let compiled = compile(src);
    assert!(!compiled.has_errors, "{:?}", compiled.diagnostics);
    assert!(compiled.rust.contains("as i64"), "should narrow via cast: {}", compiled.rust);
}

/// The returned `Ok`/`Some` payload coerces to the function's declared inner
/// type, so a float quotient narrows back to the integer the signature promises.
#[test]
fn ok_payload_coerces_to_the_declared_inner_type() {
    let src = "Function Divide(ByVal a As Long, ByVal b As Long) As Result<Long>\n\
               \x20   Return Ok(a / b)\nEnd Function\n\
               Function Main()\n    Debug.Print Divide(10, 2).Unwrap()\nEnd Function\n";
    let compiled = compile(src);
    assert!(!compiled.has_errors, "{:?}", compiled.diagnostics);
    // The float quotient is narrowed to i64 inside the Ok(...).
    assert!(
        compiled.rust.contains("as i64"),
        "the Ok payload should narrow to i64: {}",
        compiled.rust
    );
}
