# 24 — Factor Finder

Finds all multiplicative factors of a positive whole number, in ascending
order. Uses the classic optimisation: test divisors only up to the square
root, pushing both the divisor and its quotient (avoiding a duplicate for
perfect squares), then sorts ascending.

## Bust language features tested

- `Public Function` returning `Vec<Long>`; square-root-bound `Do While` loop
- `Public Sub` with `ByRef xs As Vec<Long>` — mutating the caller's list in
  place (insertion sort)
- Bracket indexing `xs[j]` and swaps with a temp
- `Mod`, integer division `n / i`
- Cross-module qualified calls (`Factors.Factors(60)`) in main and tests
- `Assert vec = [...]` list equality, `xs.Len() - 1` tail index

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/24_factor_finder    # build + run
vbr test        projects/24_factor_finder   # run the 5 tests
```
