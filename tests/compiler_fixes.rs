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

#[test]
fn foreach_string_field_clones_on_dim() {
    let rust = packed(&rust_of(
        "Public Type Crate\n    Public Dest As String\n    Public Kg As Long\nEnd Type\n\
         Function Totals(ByVal items As Vec<Crate>) As HashMap<String, Long>\n\
         \x20   Dim sums As HashMap<String, Long>\n\
         \x20   For Each c In items\n\
         \x20       Dim dest As String = c.Dest\n\
         \x20       Dim kg As Long = c.Kg\n\
         \x20       sums.Insert(dest, kg)\n\
         \x20   Next\n\
         \x20   Return sums\n\
         End Function\n\
         Function Main()\nEnd Function\n",
    ));
    assert!(
        rust.contains(".dest.clone()"),
        "For-Each String field used as a value must clone: {rust}"
    );
    assert!(
        !rust.contains(".kg.clone()"),
        "For-Each Copy field must not clone: {rust}"
    );
}

#[test]
fn foreach_struct_clones_on_push() {
    let rust = packed(&rust_of(
        "Public Type Lot\n    Public Kind As String\nEnd Type\n\
         Function Keep(ByVal kept As Vec<Lot>) As Vec<Lot>\n\
         \x20   Dim lots As Vec<Lot>\n\
         \x20   For Each k In kept\n\
         \x20       lots.Push(k)\n\
         \x20   Next\n\
         \x20   Return lots\n\
         End Function\n\
         Function Main()\nEnd Function\n",
    ));
    assert!(
        rust.contains("(*k).clone()"),
        "For-Each struct pushed onto a Vec must clone: {rust}"
    );
}

#[test]
fn foreach_byval_struct_stays_borrowed() {
    let rust = packed(&rust_of(
        "Public Type Lot\n    Public Kind As String\nEnd Type\n\
         Function LabelOf(ByVal lot As Lot) As String\n\
         \x20   Return lot.Kind\n\
         End Function\n\
         Function Labels(ByVal lots As Vec<Lot>) As Vec<String>\n\
         \x20   Dim rows As Vec<String>\n\
         \x20   For Each lot In lots\n\
         \x20       rows.Push(LabelOf(lot))\n\
         \x20   Next\n\
         \x20   Return rows\n\
         End Function\n\
         Function Main()\nEnd Function\n",
    ));
    assert!(
        rust.contains("labelof(&") || rust.contains("labelof(&*lot)"),
        "ByVal struct arg must borrow, not clone: {rust}"
    );
    assert!(
        !rust.contains("(*lot).clone()"),
        "LabelOf(lot) must not clone the loop var: {rust}"
    );
}

#[test]
fn table_accepts_sibling_public_type() {
    let pit = "Public Type Lot\n    Public Kind As String\n    Public Bags As Long\nEnd Type\n";
    let main = "\
Screen Floor\n\
    State\n\
        Dim lots As Vec<Lot>\n\
    End State\n\
    View\n\
        Table lots\n\
        End Table\n\
    End View\n\
End Screen\n\
Function Main()\n    Floor.Run\nEnd Function\n";
    let mut interfaces = vbr::resolver::ProjectInterfaces::new();
    interfaces.insert("pit".into(), vbr::module_interface(pit));
    let compiled = vbr::compile_module(main, &["pit".into()], &interfaces, true);
    assert!(
        !compiled.has_errors,
        "Table of sibling Type must compile: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled.rust.contains("use crate::pit::Lot"),
        "entry must import the sibling type: {}",
        compiled.rust
    );
    assert!(
        compiled.rust.contains("row.kind.clone()"),
        "Table columns come from the sibling struct: {}",
        compiled.rust
    );
}

#[test]
fn crate_root_type_is_imported_by_sibling() {
    let entry = "Public Type Lot\n    Public Kind As String\nEnd Type\nFunction Main()\nEnd Function\n";
    let pit = "Public Function Take(ByVal lot As Lot)\nEnd Function\n";
    let mut interfaces = vbr::resolver::ProjectInterfaces::new();
    interfaces.insert(
        vbr::resolver::CRATE_ROOT.into(),
        vbr::module_interface(entry),
    );
    let compiled = vbr::compile_module(pit, &["pit".into()], &interfaces, false);
    assert!(
        !compiled.has_errors,
        "sibling using crate-root Type must compile: {:?}",
        compiled.diagnostics
    );
    assert!(
        compiled.rust.contains("use crate::Lot;"),
        "sibling must `use crate::Lot`, not `use crate::main::Lot`: {}",
        compiled.rust
    );
    assert!(
        !compiled.rust.contains("use crate::main::"),
        "must not invent a `mod main`: {}",
        compiled.rust
    );
}

/// rustc --edition 2021, warning-free. Same bar as the happy-path snapshots.
fn assert_rustc_clean(tag: &str, rust: &str) {
    use std::fs;
    use std::process::Command;
    let dir = std::env::temp_dir().join(format!("vbr_fix_{tag}"));
    fs::create_dir_all(&dir).unwrap();
    let rs = dir.join("out.rs");
    fs::write(&rs, rust).unwrap();
    let output = Command::new("rustc")
        .arg("--edition")
        .arg("2021")
        .arg("-o")
        .arg(dir.join("out_bin"))
        .arg(&rs)
        .output()
        .expect("failed to run rustc");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "{tag}: rustc rejected generated Rust:\n{stderr}"
    );
    assert!(
        stderr.trim().is_empty(),
        "{tag}: rustc emitted warnings:\n{stderr}"
    );
}

#[test]
fn infinite_do_has_no_trailing_ok() {
    let rust = rust_of(
        "Function Main()\n\
        \x20   Do\n\
        \x20       Return\n\
        \x20   Loop\n\
        End Function\n",
    );
    let p = packed(&rust);
    assert!(p.contains("loop{"), "bare Do becomes loop: {rust}");
    assert!(
        !p.contains("loop{returnOk(());}Ok(())"),
        "infinite Do must not be followed by unreachable Ok(()): {rust}"
    );
    assert_rustc_clean("infinite_do", &rust);
}

#[test]
fn raiseerror_last_has_no_trailing_ok() {
    let rust = rust_of(
        "Function Fail()\n\
        \x20   RaiseError \"nope\"\n\
        End Function\n\
        Function Main()\n\
        \x20   Fail()\n\
        End Function\n",
    );
    let p = packed(&rust);
    assert!(
        !p.contains("returnErr(\"nope\".to_string());Ok(())"),
        "RaiseError as last stmt must not be followed by Ok(()): {rust}"
    );
    // Main calls Fail, so rustc doesn't warn about a dead helper. An empty
    // Main in other tests still needs the success value — checked there.
    assert!(p.contains("fail()?"), "Main calls Fail: {rust}");
    assert_rustc_clean("raiseerror_last", &rust);
}

#[test]
fn handle_as_last_stmt_still_has_ok() {
    // Handle's error arm can return; the success path still falls through.
    let rust = rust_of(
        "Function Boom()\n\
        \x20   RaiseError \"x\"\n\
        End Function\n\
        Function Main()\n\
        \x20   Boom() Handle err\n\
        \x20       Debug.Print err\n\
        \x20   End Handle\n\
        End Function\n",
    );
    assert!(
        packed(&rust).contains("Ok(())"),
        "Handle as last stmt still needs Ok(()) on the success path: {rust}"
    );
    assert_rustc_clean("handle_last", &rust);
}

#[test]
fn exit_do_still_has_trailing_ok() {
    let rust = rust_of(
        "Function Main()\n\
        \x20   Do\n\
        \x20       Exit Do\n\
        \x20   Loop\n\
        End Function\n",
    );
    let p = packed(&rust);
    assert!(
        p.contains("loop{break;}Ok(())"),
        "Do that can Exit still needs Ok(()): {rust}"
    );
    assert_rustc_clean("exit_do", &rust);
}

#[test]
fn if_both_branches_leave_has_no_trailing_ok() {
    let rust = rust_of(
        "Function Pick(ByVal ok As Boolean)\n\
        \x20   If ok Then\n\
        \x20       Return\n\
        \x20   Else\n\
        \x20       RaiseError \"no\"\n\
        \x20   End If\n\
        End Function\n\
        Function Main()\n\
        \x20   Pick(True)\n\
        End Function\n",
    );
    let p = packed(&rust);
    assert!(
        !p.contains("}Ok(())}"),
        "If/Else that both leave must not grow a trailing Ok(()): {rust}"
    );
    assert_rustc_clean("if_both_leave", &rust);
}

#[test]
fn stroke_circle_is_a_polyline_not_beziers() {
    // `Path::circle` is cubic Béziers; stroking them crashes WSLg's compositor
    // (`Io error: Connection reset by peer`). A closed polyline does not.
    let rust = rust_of(
        "Sketch S\n\
        \x20   Draw\n\
        \x20       Stroke Circle(100, 100, 40), Color.White, 1\n\
        \x20   End Draw\n\
        End Sketch\n\
        Function Main()\n\
        \x20   S.Run\n\
        End Function\n",
    );
    assert!(
        rust.contains("Path::line") && rust.contains("while __i < 64"),
        "stroked circle should be line chords: {rust}"
    );
    assert!(
        !rust.contains("Path::circle") && !rust.contains("Path::new"),
        "stroked circle must not use Path::circle / a closed path: {rust}"
    );
}

#[test]
fn fill_circle_is_a_pixel_stamp_not_beziers() {
    // Hundreds of `Path::circle` fills crash WSLg (bloom). A packed RGBA disk
    // is the same primitive `Set Pixel` already uses.
    let rust = rust_of(
        "Sketch S\n\
        \x20   Draw\n\
        \x20       Fill Circle(100, 100, 20), Color.White\n\
        \x20   End Draw\n\
        End Sketch\n\
        Function Main()\n\
        \x20   S.Run\n\
        End Function\n",
    );
    assert!(
        rust.contains("pix[") && rust.contains("pix_dirty"),
        "filled circle should stamp pixels: {rust}"
    );
    assert!(
        !rust.contains("Path::circle"),
        "filled circle must not use Path::circle: {rust}"
    );
}

#[test]
fn fill_circles_do_not_flush_the_buffer_between_stamps() {
    // Painter's order: disks accumulate in `pix`. Flushing (and zeroing) before
    // each `Fill Circle` used to emit one full-window image per disk.
    let rust = rust_of(
        "Sketch S\n\
        \x20   Draw\n\
        \x20       Fill Circle(10, 10, 3), Color.White\n\
        \x20       Fill Circle(40, 40, 3), Color.Red\n\
        \x20       Text \"x\", 0, 0\n\
        \x20   End Draw\n\
        End Sketch\n\
        Function Main()\n\
        \x20   S.Run\n\
        End Function\n",
    );
    let flushes = rust.matches("pix.fill(0)").count();
    assert_eq!(
        flushes, 1,
        "exactly one flush, before Text, not between disks: {rust}"
    );
}

#[test]
fn int_in_struct_literal_narrows_to_field_type() {
    // `Int(...)` is `.floor()` (f64). A Long field must get the same `as i64`
    // that `Dim x As Long = Int(...)` already inserts.
    let rust = rust_of(
        "Public Type Peg\n\
        \x20   Public X As Long\n\
        \x20   Public Y As Long\n\
        End Type\n\
        Function Make() As Peg\n\
        \x20   Return Peg { x: Int(3.9), y: Int(1.1) }\n\
        End Function\n\
        Function Main()\n\
        \x20   Dim p As Peg = Make()\n\
        \x20   Debug.Print p.X\n\
        End Function\n",
    );
    let p = packed(&rust);
    assert!(
        p.contains("asi64"),
        "Int() in a Long struct field should narrow: {rust}"
    );
    assert_rustc_clean("int_struct_lit", &rust);
}

#[test]
fn byval_param_assigned_in_body_is_mut() {
    // VBA ByVal is a local copy; assigning to it is legal, so the binding is mut.
    let rust = rust_of(
        "Function Nudge(ByVal zr As Double) As Double\n\
        \x20   zr = zr + 1.0\n\
        \x20   Return zr\n\
        End Function\n\
        Function Main()\n\
        \x20   Debug.Print Nudge(1.0)\n\
        End Function\n",
    );
    let p = packed(&rust);
    assert!(
        p.contains("fnnudge(mutzr:f64)"),
        "ByVal param written in the body should be mut: {rust}"
    );
    assert_rustc_clean("byval_mut", &rust);
}

fn gpu_sketch(body: &str) -> String {
    format!(
        "Sketch S\n\
        \x20   Gpu Draw\n\
        {body}\
        \x20   End Draw\n\
        End Sketch\n\
        Function Main()\n\
        \x20   S.Run\n\
        End Function\n"
    )
}

#[test]
fn gpu_draw_emits_an_iced_shader() {
    let rust = rust_of(&gpu_sketch(
        "        For y = 0 To height - 1\n\
        \x20           For x = 0 To width - 1\n\
        \x20               Set Pixel x, y, Color.Red\n\
        \x20           Next x\n\
        \x20       Next y\n",
    ));
    assert!(
        rust.contains("iced::widget::shader"),
        "Gpu Draw should place an iced Shader widget: {rust}"
    );
    assert!(
        rust.contains("ShaderSource::Wgsl"),
        "Gpu Draw should embed WGSL: {rust}"
    );
    assert!(
        !rust.contains("iced::widget::Canvas::new"),
        "a kernel-only Gpu Draw should not emit a CPU canvas: {rust}"
    );
}

#[test]
fn gpu_function_is_wgsl_not_rust() {
    let rust = rust_of(
        "Sketch S\n\
        \x20   Gpu Draw\n\
        \x20       For y = 0 To height - 1\n\
        \x20           For x = 0 To width - 1\n\
        \x20               Set Pixel x, y, Color(Wave(x), 0, 0)\n\
        \x20           Next x\n\
        \x20       Next y\n\
        \x20   End Draw\n\
        End Sketch\n\
        Gpu Function Wave(ByVal p As Double) As Double\n\
        \x20   Return Sin(p)\n\
        End Function\n\
        Function Main()\n\
        \x20   S.Run\n\
        End Function\n",
    );
    let p = packed(&rust);
    assert!(
        p.contains("fnwave(_p:f32)"),
        "Gpu Function should lower to a WGSL helper: {rust}"
    );
    assert!(
        !p.contains("fnwave(p:f64)"),
        "Gpu Function must not be emitted as a Rust fn: {rust}"
    );
}

#[test]
fn gpu_draw_rejects_fill_and_stroke() {
    let c = vbr::compile(&gpu_sketch(
        "        For y = 0 To height - 1\n\
        \x20           For x = 0 To width - 1\n\
        \x20               Fill Rect(0, 0, 1, 1), Color.Red\n\
        \x20               Set Pixel x, y, Color.White\n\
        \x20           Next x\n\
        \x20       Next y\n",
    ));
    assert!(c.has_errors, "Fill in Gpu Draw should error: {:?}", c.diagnostics);
    let joined = c.diagnostics.join("\n");
    assert!(
        joined.contains("CPU `Draw`") || joined.contains("Gpu Draw"),
        "should point Fill/Stroke at CPU Draw: {joined}"
    );
}

#[test]
fn gpu_draw_with_text_overlay_stacks() {
    let rust = rust_of(
        "Sketch S\n\
        \x20   Gpu Draw\n\
        \x20       For y = 0 To height - 1\n\
        \x20           For x = 0 To width - 1\n\
        \x20               Set Pixel x, y, Color.Navy\n\
        \x20           Next x\n\
        \x20       Next y\n\
        \x20   End Draw\n\
        \x20   Draw\n\
        \x20       Text \"hi\", 16, 22, Color.Gray\n\
        \x20   End Draw\n\
        End Sketch\n\
        Function Main()\n\
        \x20   S.Run\n\
        End Function\n",
    );
    assert!(
        rust.contains("iced::widget::stack"),
        "CPU Text over a kernel should stack: {rust}"
    );
    assert!(
        rust.contains("iced::widget::shader"),
        "overlay sketch still has the shader: {rust}"
    );
}

#[test]
fn gpu_copy_clear_and_pixels_emit_runtime() {
    let rust = rust_of(
        "Sketch S\n\
        \x20   State\n\
        \x20       Dim spr As Pixels = Pixels.Of(18, 18)\n\
        \x20   End State\n\
        \x20   Gpu Draw\n\
        \x20       Clear Color.Navy\n\
        \x20       Copy frame, 3, 1\n\
        \x20       Copy spr, 10, 20\n\
        \x20       Copy spr, 40, 40, 36, 36, Blend Add\n\
        \x20       Copy spr, 80, 80, ColorKey, Color.Magenta\n\
        \x20       For y = 0 To height - 1\n\
        \x20           For x = 0 To width - 1\n\
        \x20               Set Pixel x, y, Color.Red\n\
        \x20           Next x\n\
        \x20       Next y\n\
        \x20   End Draw\n\
        End Sketch\n\
        Function Main()\n\
        \x20   S.Run\n\
        End Function\n",
    );
    assert!(
        rust.contains("struct Pixels"),
        "Pixels type should be generated: {rust}"
    );
    assert!(
        rust.contains("Pixels::of(18, 18)"),
        "Pixels.Of should lower to Pixels::of: {rust}"
    );
    assert!(rust.contains("fs_copy_"), "Copy should emit a fragment: {rust}");
    assert!(rust.contains("fs_clear_"), "Clear should emit a fragment: {rust}");
    assert!(rust.contains("fs_blit"), "paper blit should be present: {rust}");
    assert!(
        rust.contains("BlendFactor::One"),
        "Blend Add should set an additive pipeline: {rust}"
    );
    assert!(
        rust.contains("distance(c.rgb, key.rgb)"),
        "ColorKey should land in WGSL: {rust}"
    );
}

#[test]
fn gpu_copy_in_cpu_draw_is_an_error() {
    let c = vbr::compile(
        "Sketch S\n\
        \x20   Draw\n\
        \x20       Copy frame, 3, 1\n\
        \x20   End Draw\n\
        End Sketch\n\
        Function Main()\n\
        \x20   S.Run\n\
        End Function\n",
    );
    assert!(c.has_errors, "Copy in CPU Draw should error: {:?}", c.diagnostics);
    let joined = c.diagnostics.join("\n");
    assert!(
        joined.contains("Gpu Draw") || joined.contains("CPU"),
        "should point Copy at Gpu Draw: {joined}"
    );
}

#[test]
fn gpu_copy_using_mask_binds_a_second_texture() {
    let rust = rust_of(
        "Sketch S\n\
        \x20   State\n\
        \x20       Dim spr As Pixels = Pixels.Of(8, 8)\n\
        \x20       Dim mask As Pixels = Pixels.Of(8, 8)\n\
        \x20   End State\n\
        \x20   Gpu Draw\n\
        \x20       Copy spr, 0, 0, Using mask\n\
        \x20   End Draw\n\
        End Sketch\n\
        Function Main()\n\
        \x20   S.Run\n\
        End Function\n",
    );
    assert!(
        rust.contains("mask_tex"),
        "Using mask should sample a second texture: {rust}"
    );
    assert!(
        rust.contains("layout01m") || rust.contains("bind_group_layouts: &[&bgl0, &bgl1m]"),
        "masked Copy should share group 1 (iced max_bind_groups is 2): {rust}"
    );
    assert!(
        rust.contains("bind_tex_mask") && rust.contains("bg_copy_"),
        "masked Copy should bind src+mask in one group: {rust}"
    );
    assert!(
        !rust.contains("set_bind_group(2,"),
        "masked Copy must not use bind group 2: {rust}"
    );
}

#[test]
fn gpu_into_pixels_writes_a_named_target() {
    let rust = rust_of(
        "Sketch S\n\
        \x20   State\n\
        \x20       Dim hole As Pixels = Pixels.Of(32, 32)\n\
        \x20   End State\n\
        \x20   Gpu Draw\n\
        \x20       Into hole\n\
        \x20           Clear Color.Black\n\
        \x20           For y = 0 To height - 1\n\
        \x20               For x = 0 To width - 1\n\
        \x20                   Set Pixel x, y, Color.White\n\
        \x20               Next x\n\
        \x20           Next y\n\
        \x20       End Into\n\
        \x20       Copy hole, 10, 10\n\
        \x20   End Draw\n\
        End Sketch\n\
        Function Main()\n\
        \x20   S.Run\n\
        End Function\n",
    );
    assert!(
        rust.contains("pipe.view_hole"),
        "Into hole should render onto that Pixels: {rust}"
    );
    assert!(
        rust.contains("pipe.ubg_hole"),
        "Into hole should use hole-sized uniforms: {rust}"
    );
    assert!(
        rust.contains("fs_kernel_"),
        "Into should still emit a fragment kernel: {rust}"
    );
}
