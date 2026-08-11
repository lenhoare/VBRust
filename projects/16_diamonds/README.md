# 16 — Diamonds

Draws ASCII-art diamonds with loops: an outline diamond (`/` `\` sides) and
a filled diamond (stars). A diamond of size n has 2n−1 rows — the widest
row appears once in the middle. Each diamond is returned as a `Vec<String>`
of rows so individual rows are directly testable.

## VBR language features tested

- Nested `For` loops; `For i = size - 1 To 1 Step -1` for the mirrored
  bottom half
- A `Spaces(n)` helper built with a loop, reused for left padding and gaps
- `&` string building in a loop
- `Public Function` returning `Vec<String>`; cross-module qualified calls
- `Sub PrintRows(ByVal rows As Vec<String>)` in main.vbr
- `For Each` over a `Vec<String>` in main

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/16_diamonds    # build + run
vbr test        projects/16_diamonds   # run the 5 tests
```
