//! Spans: every token carries its byte range in the source, and diagnostics
//! that know their offending token carry it too — that's what lets an editor
//! underline the exact word. These tests slice the source with the recorded
//! spans and check the text matches, which locks in byte (not char) offsets —
//! the distinction bites as soon as a comment contains a `→`.

use vbr::lexer::{lex, Tok};
use vbr::span::LineIndex;

#[test]
fn token_spans_slice_back_to_their_source_text() {
    // The é and → force byte offsets ≠ char offsets for everything after them.
    let src = "' café notes → here\nDim total As Long = 41\n";
    let toks = lex(src);
    let texts: Vec<&str> = toks
        .iter()
        .filter(|t| !matches!(t.tok, Tok::Newline | Tok::Eof))
        .map(|t| &src[t.span.start..t.span.end])
        .collect();
    assert_eq!(
        texts,
        vec!["' café notes → here", "Dim", "total", "As", "Long", "=", "41"]
    );
}

#[test]
fn hover_knows_an_identifiers_vb_and_rust_types() {
    let src = "Function Main()\n    Dim total As Long = 0\n    total = total + 1\nEnd Function\n";
    let compiled = vbr::compile(src);
    // Every `total` — the declaration and both uses — hovers with the pair
    // a learner needs: the VB type and the Rust type it lowers to.
    let totals: Vec<&str> = compiled
        .hovers
        .iter()
        .filter(|(span, _)| &src[span.start..span.end] == "total")
        .map(|(_, text)| text.as_str())
        .collect();
    assert_eq!(totals.len(), 3, "declaration + assignment target + use");
    for t in totals {
        assert_eq!(t, "total As Long · Rust: `i64`");
    }
}

#[test]
fn go_to_definition_points_a_use_back_at_its_dim() {
    let src = "Function Main()\n    Dim total As Long = 0\n    Debug.Print total\nEnd Function\n";
    let compiled = vbr::compile(src);
    let (use_span, decl_span) = compiled
        .defs
        .iter()
        .find(|(u, _)| &src[u.start..u.end] == "total")
        .expect("the Debug.Print use should map to a definition");
    assert_eq!(&src[decl_span.start..decl_span.end], "total");
    assert!(decl_span.start < use_span.start, "declaration comes first");
    // The declaration is the one on the Dim line.
    let (line, _) = LineIndex::new(src).position(decl_span.start);
    assert_eq!(line, 1);
}

#[test]
fn a_broken_statement_does_not_silence_the_rest_of_the_file() {
    // Mid-typing, the editor recompiles on every keystroke — a half-typed
    // line must cost one error, with everything below it still analysed.
    let src = "Function Main()\n    Dim a As Long = ]\n    Dim b As Currency\nEnd Function\n\nFunction Later()\n    Dim c As Variant\nEnd Function\n";
    let compiled = vbr::compile(src);
    let msgs = compiled.diagnostics.join("\n");
    assert!(msgs.contains("Expected an expression"), "the broken line itself");
    assert!(msgs.contains("Currency"), "the next statement is still analysed");
    assert!(msgs.contains("Variant"), "so is the next function");
}

#[test]
fn parser_error_span_underlines_the_offending_token() {
    let src = "Function Main()\n    Debug.Print 1 + ]\nEnd Function\n";
    let compiled = vbr::compile(src);
    let with_span = compiled
        .diagnostic_items
        .iter()
        .find(|d| d.span.is_some())
        .expect("a parse error should carry a span");
    let span = with_span.span.unwrap();
    assert_eq!(&src[span.start..span.end], "]");
    // And the span converts to the position an editor would show: line 2
    // (0-based 1), just after `1 + `.
    let (line, col) = LineIndex::new(src).position(span.start);
    assert_eq!((line, col), (1, 20));
}
