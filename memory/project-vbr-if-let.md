---
name: project-vbr-if-let
description: VBR if-let — `If <expr> Is <pattern> Then …` handles one enum/Option/Result case; emits real Rust `if let`
metadata:
  type: project
---

**`if let` for VBR — BUILT 2026-08-09.** VB-flavoured spelling: **`If <expr> Is <pattern> Then … End If`** (single-line form too), where `Is` is VB's own word (VB6's `If obj Is Nothing`) repurposed to mean "matches the pattern". Handles just the one case you care about (usually `Some`/`Ok`), the gap between `Match` (both cases — verbose), `?` (propagate — fallible fns only), and `.Unwrap()` (crash). Len picked this "Option B" over the Rust-order `If Some(v) = expr Then` because `Is` avoids overloading `=` and has real VB heritage.

**Design (key win — near-zero backend ripple):** rather than a new `Stmt` variant, added a bool field **`if_let` to `Stmt::Match`** (ast.rs). Every existing `Stmt::Match { .., .. }` match site uses `..`, so none broke; only the 3 destructure-and-rebuild rewrite passes (surface.rs ×2, gui.rs ×1) needed the field threaded. The parser (`parse_if` → `parse_if_let`) detects a peeked `Is` (an `Ident` compared case-insensitively — **no new token**, so no enum ripple), captures the pattern raw up to `Then` (same `pattern_tok_src` as Match arms), and builds `Stmt::Match { if_let: true, arms: [<pattern>→body, "_"→{}] }`. The `_ => {}` arm is only for non-Rust backends.

**Rendering:** the Rust transpiler's `Stmt::Match { if_let: true, .. }` arm emits real **`if let <pattern> = <scrutinee> { body }`** (uses arms[0]; ignores the synthesized `_`), so the learner sees the idiomatic construct. C and Python **ignore the flag** and render their normal `match` (which they already support) over both arms — so if-let works on all three backends: Rust `if let`, Python `match/case _`, C if-chain. Verified `vbr run`/`vbr py` output + `if_let` in C_BEHAVIOUR test.

**`Else` + `while let` BUILT 2026-08-09 (same session).**
- **`Else`**: `If x Is Some(v) Then … Else … End If` (block + single-line). The else-body becomes the **`_ => …` arm's body** — which doubles as the Rust `else` block (transpiler if_let arm emits `} else {` when `arms[1].body` is non-empty) AND the wildcard arm the other backends already render. Zero new plumbing.
- **`while let`**: `Do While <expr> Is <pattern> … Loop`. **Desugared entirely in the parser** — no DoCond variant, no backend edits: builds `DoLoop { cond: None, body: [Match{ if_let:true, arms:[P→body, "_"→[Break]] }] }`. Rust renders `loop { if let P = e { … } else { break } }`; C/Python render `while(1)/while True { match … case _: break }` — all correct. New `enum LoopHead { While, Until, WhileLet }` returned by `parse_loop_cond`; `parse_do`'s (pre,post) match handles it (WhileLet only valid as pre; `Loop While … Is` errors). Reused the DoCond variants (all 9 exhaustive match sites) untouched.

Guard: `examples/if_let.vbr` (HAPPY + C_BEHAVIOUR) covers if-let/else/single-line/while-let; `tests/if_let.rs` (4). Docs: language_reference §8 + vb6 guide. See [[project-vbr-match]], [[project-vbr-firstclass-types]]. Nothing further deferred.
