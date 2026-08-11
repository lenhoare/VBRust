# 12 — Collatz Sequence

The Collatz conjecture demo: starting from any positive integer n, the next
number is n/2 if n is even, else 3n+1; the sequence is believed to always
reach 1. This example computes and prints full sequences for 12, 19 and 27
(the famous long one), and exposes a length function for tests.

## VBR language features tested

- `Public Function` returning a `Vec<Long>`, built with `seq.Push(n)`
- `Do While ... Loop` with `Exit` conditions, `Mod` and integer division
- Cross-module qualified calls (`Collatz.Collatz(12)`)
- A free `FormatSeq` helper in main.vbr (entry module is not testable, so it
  just formats) using `For Each`, `CStr()` and `&` concatenation
- `Assert vec = [ ... ]` — Vec equality against an inline list literal

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/12_collatz    # build + run
vbr test        projects/12_collatz   # run the 5 tests
```
