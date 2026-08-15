# 52 — Numeral Systems Counters

Shows the same integers in decimal, hexadecimal and binary as a padded
table (0..15). Hand-rolled base conversion (repeated division with `Mod`)
— no stdlib radix helpers needed.

## Bust language features tested

- Hand-rolled `ToBinary` / `ToHex` via `Do While` + `Mod` + integer division
- Prepending digits with `&` (`out = CStr(digit) & out`)
- `Mid(digits, d + 1, 1)` to look up a hex digit from a lookup string
- A `Pad` helper using `Do While s.Len() < width` for right-justification
- Cross-module qualified calls (`Numerals.Row(n)`) in main and tests
- `For n = 0 To 15` counting loop

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/52_numeral_systems    # build + run
vbr test        projects/52_numeral_systems   # run the 5 tests
```
