---
name: project-vbr-if-let
description: VBR if-let — `If <expr> Is <pattern> Then …` handles one enum/Option/Result case; emits real Rust `if let`
metadata:
  type: project
---

**`if let` for VBR — BUILT 2026-08-09.** VB-flavoured spelling: **`If <expr> Is <pattern> Then … End If`** (single-line form too), where `Is` is VB's own word (VB6's `If obj Is Nothing`) repurposed to mean "matches the pattern". Handles just the one case you care about (usually `Some`/`Ok`), the gap between `Match` (both cases — verbose), `?` (propagate — fallible fns only), and `.Unwrap()` (crash). Len picked this "Option B" over the Rust-order `If Some(v) = expr Then` because `Is` avoids overloading `=` and has real VB heritage.

**Design (key win — near-zero backend ripple):** rather than a new `Stmt` variant, added a bool field **`if_let` to `Stmt::Match`** (ast.rs). Every existing `Stmt::Match { .., .. }` match site uses `..`, so none broke; only the 3 destructure-and-rebuild rewrite passes (surface.rs ×2, gui.rs ×1) needed the field threaded. The parser (`parse_if` → `parse_if_let`) detects a peeked `Is` (an `Ident` compared case-insensitively — **no new token**, so no enum ripple), captures the pattern raw up to `Then` (same `pattern_tok_src` as Match arms), and builds `Stmt::Match { if_let: true, arms: [<pattern>→body, "_"→{}] }`. The `_ => {}` arm is only for non-Rust backends.

**Rendering:** the Rust transpiler's `Stmt::Match { if_let: true, .. }` arm emits real **`if let <pattern> = <scrutinee> { body }`** (uses arms[0]; ignores the synthesized `_`), so the learner sees the idiomatic construct. C and Python **ignore the flag** and render their normal `match` (which they already support) over both arms — so if-let works on all three backends: Rust `if let`, Python `match/case _`, C if-chain. Verified `vbr run`/`vbr py` output + `if_let` in C_BEHAVIOUR test.

Guard: `examples/if_let.vbr` in HAPPY (rustc-compiled, no-warnings) + C_BEHAVIOUR; `tests/if_let.rs` (2). Docs: language_reference §8 + vb6 guide errors section & cheat sheet.

**DEFERRED (Len wants to discuss, needs explaining first):** `Else` (`if let … else`) and `while let`. See [[project-vbr-match]] (the `Match`/Select Case replacement this complements) and [[project-vbr-firstclass-types]] (Option/Result).
