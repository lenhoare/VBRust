# 40 — Leetspeak

Converts plain English into 1337-speak using a fixed character mapping
(a→4, b→8, e→3, g→9, i→1, l→1, o→0, s→5, t→7, z→2). The book's version
picks a random variant per letter; this one is **deterministic** (one
canonical substitution per letter) so the output is exact and testable.

## VBR language features tested

- `Mid` / `Len()` per-character loop; `&` string building
- `LCase()` builtin for case-insensitive lookup
- `Public Function` returning `String`, called cross-module qualified
- A multi-branch `If / ElseIf` mapping chain
- Note: `Match` on a `String` scrutinee against `&str` literal patterns
  does not compile (mismatched types) — use `If/ElseIf` with `=` instead.
  See notes.md Quirk 14.

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/40_leetspeak    # build + run
vbr test        projects/40_leetspeak   # run the 5 tests
```
