# 64 — Seven-Segment Display

Renders a string of digits as calculator-style seven-segment text graphics.
Each digit is drawn on a 3-row × 3-column grid (top, middle, bottom rows);
digits are joined horizontally with one space. Supports 0–9 and `-`.
Designed as a reusable module (the book's project 14 Countdown builds on
this one).

## Bust language features tested

- `Public Type Segment` with three String fields, built complete with the
  `Segment { top: ..., mid: ..., bottom: ... }` literal constructor
- **Structs as return values** — `Public Function SegmentForDigit(...) As
  Segment` returning a freshly-built struct; a `Match` whose arms each
  `Return Segment { ... }`
- `Match` over a `Long` with integer literal arms (0–9) — exhaustive
- Cross-module use of a `Public Type` in the test file
- `Val()` → `Long` narrowing through a local (`Dim dv As Long = Val(d)`)
- `&` row building in a loop; `Vec<String>` of the three rows

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/64_seven_segment    # build + run
vbr test        projects/64_seven_segment   # run the 6 tests
```
