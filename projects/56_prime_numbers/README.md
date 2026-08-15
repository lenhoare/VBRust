# 56 — Prime Numbers

Brute-force prime finding. `IsPrime` tests odd numbers for divisibility up
to the square root (2 handled specially); `FirstPrimes` collects a run; a
`RangePrimes` helper in main.vbr scans an interval. Deterministic and fast.

## Bust language features tested

- `Public Function` returning `Boolean` and `Vec<Long>`
- Square-root-bound `Do While` with `Mod`, odd-only stepping (`i = i + 2`)
- `Do While out.Len() < count` loop termination on a growing Vec
- Cross-module qualified calls in main and tests
- `Assert Not` negation in tests

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/56_prime_numbers    # build + run
vbr test        projects/56_prime_numbers   # run the 5 tests
```
