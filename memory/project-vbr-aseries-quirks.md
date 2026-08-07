---
name: project-vbr-aseries-quirks
description: A-series example-project testing surfaced ~20 quirks; triaged into clusters; slice 1 (quick wins) BUILT; coercion cluster + `/` semantics still pending a decision
metadata:
  type: project
---

A "lesser AI" built the **A-series example projects** (Aug 2026) to stress-test VBR and logged ~20 quirks (bugs/missing/doc-gaps). I triaged them by **root cause** (several "quirks" are one bug seen from different angles) — see the conversation for the full list. Clusters:

- **Coercion cluster (Quirks 1, 2, 12, 19, 21)** — numeric widening + String auto-borrow applied inconsistently across argument positions: int-literal→Double fails on *qualified* (cross-module) calls but works locally; `For` counter (i32) never widened to Double; `usize` (from `InStr`/`.find`) not widened to Long in a Match-arm return; a String-returning *call expr* isn't auto-borrowed to `&str` (only *variables* are); `Val`(Double)→Long not narrowed on a qualified call. **The real fix = apply adaptation uniformly at every arg position; this is exactly what the deferred IR consolidation would centralise.** PENDING DECISION: patch now vs. fold into consolidation.
- **Quirk 24 — `/` semantics** — `done / total` (two Longs) does *integer* division then casts (25/100→0.0). This is a **VB6 semantic divergence**: VB6 `/` is always float division, `\` was integer. Silently-wrong-answer. PENDING Len's design call: should `/` always produce a float?
- **Rust-keyword collisions (18, 32, field #29)** and **VBR reserved words as names (17, 23, 27)** — handled in slice 1.

**SLICE 1 (quick wins) BUILT 2026-08-07** — `tests/quirks_slice1.rs` (4 tests) + the harness digit test:
- **Q31** `escape()` (transpiler.rs) now escapes `\r` (was missing) — `vbCrLf` no longer emits a raw CR into `format!`.
- **Q14** owned-`String` `Match` scrutinee lowered through `.as_str()` (resolver.rs `Stmt::Match`), gated on `infer(..).is_owned_string()` (NOT `VType::Str`, which is `&str` — `.as_str()` on `&str` is the unstable `str_as_str`) AND a `"…"` literal pattern present.
- **Q18/Q32** `rust_name()` now escapes Rust keywords: `is_rust_keyword` + `escape_rust_keyword` → `r#move` etc. **CRUCIAL**: `self`/`crate`/`super`/`_` pass through UNTOUCHED (codegen generates `self` from `Me`, `crate::`/`super::` paths — escaping them broke the Godot runner). `snake` in resolver is aliased to `rust_name`, so escaping is consistent across resolver+transpiler (incl. `passed_by_ref`).
- **Q17/Q23/Q27** `expect_ident` (parser.rs) gives a targeted "`To` is a VBR keyword…" message via new `lexer::keyword_word(&Tok)`, instead of cryptic "expected a name, found To".
- **Q9** `.Length` on a Vec → `message_hint()` in main.rs `report_errors` (message-content hint, since `teaching_hint` is code-only) suggests `.Len()`/`.Count()`.
- **Q10** `test_fn_names` prefixes `t_` when a test description starts with a digit (Rust ident can't).
- **Q25** (bare `Return` in an event) is **surface-specific**, NOT a blanket fix: TUI events return `io::Result<()>` (want `Ok(())`), but GUI `update` returns `()`/`Task<Message>` and Web returns `bool` — a shared rewrite would break GUI/Web. Handled as a **doc note in tui_spec.md**; a proper per-surface lowering could be a later slice.

Related: [[project-vbr-numeric-widening-gap]] (the earlier widening fix), [[project-vbr-internals-refactor]], [[project-vbr-projects-run]] (#29 reserved field names). Still-open quirks for later slices: Enum Display (Q3), nested-`Vec` `For Each` mis-deref (Q28), local-shadows-module (Q16), `Asc`/`IIf` (Q11), multiline list literals (Q15), plus the coercion cluster + `/` decision above.
