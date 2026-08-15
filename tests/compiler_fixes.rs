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
fn datetime_argument_is_borrowed() {
    // `a.Diff_Days(b)` — a DateTime argument to a stdlib method is taken by ref.
    let rust = packed(&rust_of(
        "Function Main()\n\
        \x20   Dim a As DateTime = DateTime.Now()\n\
        \x20   Dim b As DateTime = DateTime.Now()\n\
        \x20   Debug.Print a.Diff_Days(b)\n\
        End Function\n",
    ));
    assert!(rust.contains("diff_days(&b)"), "DateTime arg should be borrowed: {rust}");
}

#[test]
fn option_does_not_auto_propagate() {
    // Options stay Options — a discarded Option is an error. Errors (Result)
    // propagate automatically; absence is Match / Unwrap_Or.
    let c = vbr::compile(
        "Function Main()\n\
        \x20   Dim xs As Vec<Long> = [1, 2, 3]\n\
        \x20   xs.First()\n\
        End Function\n",
    );
    assert!(c.has_errors, "discarded Option should error: {:?}", c.diagnostics);
    let joined = c.diagnostics.join("\n");
    assert!(
        joined.contains("Option") || joined.contains("thrown away"),
        "should mention the discarded Option: {joined}"
    );
}

#[test]
fn option_return_passes_through() {
    let rust = packed(&rust_of(
        "Function Head(ByVal xs As Vec<Long>) As Option<Long>\n\
        \x20   Return xs.First()\n\
        End Function\n",
    ));
    assert!(
        rust.contains(".first()") && (rust.contains("copied") || rust.contains("cloned") || rust.contains("Option")),
        "option return passes through: {rust}"
    );
}

#[test]
fn main_always_wraps_as_a_sink() {
    let rust = rust_of(
        "Function Main()\n\
        \x20   Dim v As Long = Parse(\"5\")\n\
        \x20   Debug.Print v\n\
        End Function\n\
        Function Parse(ByVal s As String) As Long\n\
        \x20   Return CLng(s)\n\
        End Function\n",
    );
    assert!(
        rust.contains("fn vbr_main() -> Result<(), String>"),
        "main helper is fallible: {rust}"
    );
    assert!(rust.contains("fn main()"), "entry wrapper exists: {rust}");
    assert!(packed(&rust).contains("Ok(())"), "main closes with Ok(()): {rust}");
}

#[test]
fn main_declared_fallible_is_rejected() {
    let c = vbr::compile(
        "Function Main() As Result<Long>\n\
        \x20   Return 0\n\
        End Function\n",
    );
    assert!(c.has_errors, "Main As Result should error: {:?}", c.diagnostics);
    let joined = c.diagnostics.join("\n");
    assert!(
        joined.contains("As Result") || joined.contains("Main has no return type"),
        "message steers off As Result on Main: {:?}",
        c.diagnostics
    );
}

#[test]
fn vec_get_casts_a_variable_index_to_usize() {
    // `.Get(i)` on a Vec takes a usize, but the index is usually a Long — cast
    // it, just as `xs[i]` does. A literal index already coerces, so it's left be.
    let rust = packed(&rust_of(
        "Function Main()\n\
        \x20   Dim xs As Vec<Long> = [10, 20, 30]\n\
        \x20   Dim i As Long = 1\n\
        \x20   Debug.Print xs.Get(i).Unwrap_Or(-1)\n\
        \x20   Debug.Print xs.Get(0).Unwrap_Or(-1)\n\
        End Function\n",
    ));
    assert!(rust.contains("get(iasusize)"), "variable index cast to usize: {rust}");
    assert!(rust.contains("get(0)"), "literal index not needlessly cast: {rust}");
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

#[test]
fn dim_named_handle_is_not_empty_init() {
    let rust = rust_of(
        "Public Type Entry\n    Public N As Long\nEnd Type\n\
         Function Parse() As Entry\n    Return Entry { n: 1 }\nEnd Function\n\
         Function Main()\n\
         \x20   Dim e As Entry = Parse() Handle err\n\
         \x20       Return\n\
         \x20   End Handle\n\
         \x20   Debug.Print e.N\n\
         End Function\n",
    );
    assert!(!rust.contains("Entry = ;"), "Handle Dim must not emit empty init:\n{rust}");
    assert!(rust.contains("= match"), "Handle Dim assigns from match:\n{rust}");
}

#[test]
fn assign_from_vec_string_clones() {
    let rust = packed(&rust_of(
        "Function Main()\n\
        \x20   Dim lines As Vec<String> = [\"a\", \"b\"]\n\
        \x20   Dim s As String = \"\"\n\
        \x20   s = lines[0]\n\
        End Function\n",
    ));
    assert!(rust.contains(".clone()"), "indexing a Vec<String> into a String clones: {rust}");
}

#[test]
fn field_push_marks_outer_mut() {
    let rust = packed(&rust_of(
        "Public Type Book\n    Public Items As Vec<Long>\nEnd Type\n\
         Function Main()\n\
         \x20   Dim b As Book = Book { items: [] }\n\
         \x20   b.Items.Push(1)\n\
         End Function\n",
    ));
    assert!(rust.contains("letmutb"), "b.Items.Push must mark b mut: {rust}");
}

#[test]
fn byval_enum_compares_by_value() {
    let rust = packed(&rust_of(
        "Public Enum Lane\n    Inbox\n    Doing\nEnd Enum\n\
         Function Check(ByVal lane As Lane) As Boolean\n\
         \x20   If lane = Lane.Doing Then\n\
         \x20       Return True\n\
         \x20   End If\n\
         \x20   Return False\n\
         End Function\n\
         Function Main()\n\
         \x20   Debug.Print Check(Lane.Inbox)\n\
         End Function\n",
    ));
    assert!(
        rust.contains("lane:Lane") && !rust.contains("lane:&Lane"),
        "ByVal enum is passed by value: {rust}"
    );
}

#[test]
fn double_field_widens_int_literal() {
    let rust = packed(&rust_of(
        "Public Type Bug\n    Public X As Double\nEnd Type\n\
         Function Main()\n\
         \x20   Dim b As Bug = Bug { x: 1.0 }\n\
         \x20   b.X = 8\n\
         End Function\n",
    ));
    assert!(
        rust.contains("b.x=8.0") || rust.contains("b.x=8.0f64") || rust.contains("=8.0"),
        "int literal into Double field becomes 8.0: {rust}"
    );
}

#[test]
fn method_byval_string_borrows_owned_local() {
    let rust = packed(&rust_of(
        "Public Type Book\n    Public N As Long\nEnd Type\n\
         Public Function Book.Save(ByVal path As String)\n\
         End Function\n\
         Function Main()\n\
         \x20   Dim b As Book = Book { n: 0 }\n\
         \x20   Dim path As String = \"ledger.txt\"\n\
         \x20   b.Save(path)\n\
         End Function\n",
    ));
    assert!(
        rust.contains("save(&path)") || rust.contains(".save(&path)"),
        "ByVal String method arg must borrow: {rust}"
    );
}

#[test]
fn state_user_fn_uses_init_not_default() {
    let rust = rust_of(
        "Function Seed() As Vec<Long>\n    Return [1, 2, 3]\nEnd Function\n\
         Screen App\n    State\n        Dim xs As Vec<Long> = Seed()\n    End State\n\
         View\n        Column\n            Text \"hi\"\n        End Column\n    End View\n\
         End Screen\n\
         Function Main()\n    App.Run\nEnd Function\n",
    );
    assert!(rust.contains("fn init()"), "fallible State init must use init():\n{rust}");
    assert!(
        !rust.contains("fn default()"),
        "must not emit Default with ? :\n{rust}"
    );
    assert!(rust.contains("seed()?"), "Seed() is unwrapped in init():\n{rust}");
}

#[test]
fn dim_vec_handle_is_not_dummy_empty() {
    let rust = rust_of(
        "Function Load() As Vec<String>\n    Return [\"a\"]\nEnd Function\n\
         Function Main()\n\
         \x20   Dim lines As Vec<String> = Load() Handle err\n\
         \x20       Return\n\
         \x20   End Handle\n\
         \x20   Debug.Print lines.Len()\n\
         End Function\n",
    );
    assert!(
        !packed(&rust).contains("Vec<String>=Vec::new()"),
        "Handle Dim of a Vec must not emit a dummy empty init:\n{rust}"
    );
    assert!(rust.contains("= match"), "Handle Dim assigns from match:\n{rust}");
}
