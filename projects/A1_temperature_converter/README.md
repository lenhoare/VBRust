# A1 — Temperature Converter

The first of the A-series projects: simple, deterministic programs written to
shake out the VBR transpiler end to end (project layout → transpile → build →
run → test) before tackling the numbered list.

## What it does

A tiny temperature-conversion module plus a demo entry point. `temps.vbr`
holds the conversion formulas (Celsius ⇄ Fahrenheit, Celsius ⇄ Kelvin) and a
`Describe` classifier; `main.vbr` prints a Celsius → Fahrenheit conversion
table and a few spot checks. Everything is pure arithmetic on `Double`, so
output is deterministic and easy to pin.

```text
projects/A1_temperature_converter/
  main.vbr           entry point — Function Main(); prints the table
  temps.vbr          the logic: Public CtoF / FtoC / CtoK / KtoC / Describe
  temps.test.vbr     Test / Assert specs for temps.vbr
  expected_output.txt exact stdout of running main.vbr
```

## Which VBR language features it tests

- A **multi-module project**: `main.vbr` calls the sibling module with a
  **qualified name** — `Temps.CtoF(...)`.
- **`Public` functions** as the tested contract, with `ByVal … As Double`
  parameters and `Return`.
- A **`Public Const`** (`AbsoluteZeroC`) read cross-module as
  `Temps.AbsoluteZeroC` (constants are uppercased on the Rust side).
- **`If / ElseIf / Else`** in the `Describe` classifier.
- **`For … Step`** loop building the conversion table, with the loop counter
  widened into a `Double` before it crosses into the module (`Dim d As Double
  = c` — VBR emits the `as f64` cast for you).
- **String concatenation** with `&` (numbers are formatted automatically).
- The **`Test` / `Assert` harness**: `Assert a = b` lowers to `assert_eq!`, so
  a failure shows both operands and the `.vbr` line.

**Standard library:** none — pure core language, deliberately.

## Run it

```sh
vbr runproject projects/A1_temperature_converter
# or, without the vbr binary installed:
cargo run -- runproject projects/A1_temperature_converter
```

Its stdout matches `expected_output.txt` exactly.

## Test it

```sh
vbr test projects/A1_temperature_converter
# or:
cargo run -- test projects/A1_temperature_converter
```

Expected:

```
  ✓ celsius to fahrenheit at the freezing and boiling points
  ✓ fahrenheit to celsius round-trips the same two points
  ✓ kelvin conversions anchor at absolute zero
  ✓ absolute zero is a public constant near -460 Fahrenheit
  ✓ describe sorts temperatures into five bands

  5 passed
```

## Notes for the transpiler

Two quirks surfaced (both worked around here, both logged in
`projects/notes.md`):

1. **Qualified calls don't adapt integer literals to `Double` parameters.**
   `Temps.CtoF(100)` fails to compile ("expected `f64`, found integer") while
   the same-file call `CtoF(100)` succeeds. Write `100.0` explicitly, or widen
   through a local `Dim`.
2. **`For`-loop counters are never adapted to `Double` parameters**, even
   locally. Widen the counter into a `Double` before passing it on.

Test values are chosen to be exactly representable in binary floating point
(0, 32, 100, 212, 273.15, −459.67-area checks are done with a `< -450` bound
rather than float equality).
