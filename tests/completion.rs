//! Completion: `completions_at(source, offset)` answers "what can follow the
//! cursor?" from the compiler's own knowledge — receiver-typed members after
//! a dot, in-scope names in bare position. The cursor sits on a half-typed
//! line by definition, so these tests all place it inside a broken statement:
//! error recovery keeping the rest of the file analysed is what makes the
//! lookups possible.

use vbr::complete::{completions_at, CompletionKind};

/// Insert `typed` on a new line after the line containing `after`, and
/// complete at the end of it.
fn complete(src: &str, after: &str, typed: &str) -> Vec<(String, CompletionKind)> {
    let idx = src.find(after).expect("marker") + after.len();
    let text = format!("{}\n    {}{}", &src[..idx], typed, &src[idx..]);
    let offset = idx + 5 + typed.len();
    completions_at(&text, offset)
        .into_iter()
        .map(|c| (c.label, c.kind))
        .collect()
}

fn labels(items: &[(String, CompletionKind)]) -> Vec<&str> {
    items.iter().map(|(l, _)| l.as_str()).collect()
}

const SRC: &str = "\
Type Person
    Name As String
    Age As Long
End Type

Function Person.Greet() As String
    Return \"hi\"
End Function

Enum Color
    Red
    Green
End Enum

Function Main()
    Dim s As String = \"hello\"
    Dim nums As Vec<Long> = [1, 2]
    Dim p As Person = Person { Name: \"Len\", Age: 48 }
    Debug.Print s
End Function
";

#[test]
fn a_string_variable_offers_the_real_rust_methods() {
    let items = complete(SRC, "Debug.Print s", "s.");
    let l = labels(&items);
    assert!(l.contains(&"trim") && l.contains(&"to_uppercase") && l.contains(&"len"));
}

#[test]
fn a_vec_variable_offers_push_and_the_iterator_adapters() {
    let items = complete(SRC, "Debug.Print s", "nums.");
    let l = labels(&items);
    assert!(l.contains(&"Push") && l.contains(&"filter") && l.contains(&"map"));
}

#[test]
fn a_struct_variable_offers_its_fields_and_methods() {
    let items = complete(SRC, "Debug.Print s", "p.");
    assert_eq!(
        items,
        vec![
            ("Name".to_string(), CompletionKind::Field),
            ("Age".to_string(), CompletionKind::Field),
            ("Greet".to_string(), CompletionKind::Method),
        ]
    );
}

#[test]
fn a_stdlib_namespace_offers_its_functions() {
    let items = complete(SRC, "Debug.Print s", "Http.");
    assert_eq!(labels(&items), vec!["Get", "Post"]);
    // A partial member keeps the member context (the editor filters by it).
    let items = complete(SRC, "Debug.Print s", "FileSystem.Rea");
    assert!(labels(&items).contains(&"ReadLines"));
}

#[test]
fn an_enum_name_offers_its_variants() {
    let items = complete(SRC, "Debug.Print s", "Color.");
    assert_eq!(labels(&items), vec!["Red", "Green"]);
}

#[test]
fn bare_position_offers_scope_then_program_items() {
    let items = complete(SRC, "Debug.Print s", "");
    let l = labels(&items);
    // In-scope variables (from this function only), program items, namespaces.
    assert!(l.contains(&"s") && l.contains(&"nums") && l.contains(&"p"));
    assert!(l.contains(&"Main") && l.contains(&"Color") && l.contains(&"Person"));
    assert!(l.contains(&"Http") && l.contains(&"Dim"));
}

#[test]
fn nothing_inside_a_string_literal() {
    let src = "Function Main()\n    Debug.Print \"Http.\"\nEnd Function\n";
    let offset = src.find("Http.").unwrap() + 5;
    assert!(completions_at(src, offset).is_empty());
}
