# 35 — Hex Grid

Builds a repeating hexagonal grid pattern (the chicken-wire / interlocking
hexagon look) as text rows. This project exists to exercise the **C
target** (`vbr c`): the same VBR source transpiles to a single
self-contained `.c` that compiles with plain `cc`, and its output is
checked **byte-for-byte against the Rust build** (the ground-truth
discipline from `targets_spec.md`).

## Structure

- **`main.vbr` + `hexgrid.vbr` + `hexgrid.test.vbr`** (project root) — the
  multi-module version, run and tested on the **Rust** target (6/6 tests).
- **`c/main.vbr`** (subfolder) — the single-file variant that `vbr c`
  transpiles (the C target has no multi-module project mode — same gap as
  `vbr py`, notes.md Quirk 48).

The program is deliberately **string-builtin-free** (loops, `&`
concatenation, Vec only) because both the Python and C backends pass the VB
string builtins (`Mid`/`Left`/`Val`/`UCase`) through as undefined names —
notes.md Quirk 49.

## What the C target test shows

```sh
vbr runproject projects/35_hex_grid/c        # Rust ground truth
vbr c projects/35_hex_grid/c/main.vbr -o hex.c
cc hex.c -lm -o hex && ./hex                  # plain C compiler, no deps
```

The C output is **byte-identical** to the Rust output. The generated C is
idiomatic: a monomorphised `Vec_str` with growable push, `vbr_concat`
helper, `long long` for Long, `size_t` loops.

## VBR language features tested

**C target (the point of the project):**
- `vbr c` single-file transpilation → self-contained `.c`
- Monomorphised `Vec<String>`; `&` concatenation lowering
- Byte-for-byte output parity with Rust; builds with plain `cc -lm`

**Core language (in hexgrid.vbr, 6 unit tests):**
- Nested `For` loops building rows; `Mod`-based even/odd offset
- `If / ElseIf` phase dispatcher; `Spaces(n)` helper

## Running it

```sh
vbr runproject projects/35_hex_grid       # Rust: multi-module
vbr test        projects/35_hex_grid      # 6 logic tests
vbr runproject projects/35_hex_grid/c     # Rust: single-file variant
vbr c projects/35_hex_grid/c/main.vbr -o /tmp/hex.c
cc /tmp/hex.c -lm -o /tmp/hex && /tmp/hex # C target
```

## Expected output

`expected_output.txt` (root) is the multi-module Rust build;
`c/expected_output.txt` is the single-file variant's Rust output. The C
target matches the latter byte-for-byte (verified by diff).
