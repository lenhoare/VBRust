# 65 — Shining Carpet

Generates the interlocking hexagonal carpet pattern (the Overlook Hotel
design from *The Shining*) as ASCII rows. Hexagons are 6 rows tall × 6
columns wide; even row-groups tile them side by side, odd row-groups offset
them by half a hexagon (3 columns), producing the brickwork interlock.

## VBR language features tested

- Nested `For` loops building rows and columns
- `Public Function` returning `Vec<String>`; cross-module qualified calls
- A multi-branch `If / ElseIf` phase dispatcher (`HexRow`)
- `Mod` for the even/odd group offset
- `Spaces(n)` helper reused from earlier projects
- `For Each` over the returned rows in main; `Assert rows[i] = "..."` in tests

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/65_shining_carpet    # build + run
vbr test        projects/65_shining_carpet   # run the 6 tests
```
