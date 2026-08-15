# 26 — Fibonacci

Prints the first 15 Fibonacci numbers and spot-checks larger terms. The
logic module builds the sequence as a `Vec<Long>` and also exposes an
`Fib(n)` accessor, both directly testable.

## Bust language features tested

- `Public Function` returning `Vec<Long>` with `Push`, and one returning `Long`
- `For` loop with three tracked locals (swap-based iteration)
- Cross-module qualified calls (`Fib.Fibs(15)`)
- `Assert vec = [...]` list equality in tests

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/26_fibonacci    # build + run
vbr test        projects/26_fibonacci   # run the 5 tests
```
