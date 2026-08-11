# 77 — Tower of Hanoi

Solves the classic disk puzzle with the textbook **recursive** algorithm:
move n−1 disks to the spare peg, move the largest disk, move n−1 back.
Returns the move list as `Vec<String>` in "src->dst" form. Deterministic.

## VBR language features tested

- **Recursion** — `Public Sub MoveDisk` calls itself (VBR supports it
  directly; functions lower to ordinary Rust fns)
- `Public Sub` with multiple `ByVal Long` params and a `ByRef Vec<String>`
  accumulator — recursion appends to the caller's Vec
- `2 ^ n` exponentiation builtin
- `CStr()` number→string for building "1->3" move strings
- `For Each` over the returned moves in main and in a validation test
- `Val()` builtin to parse a single digit back for validation

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/77_tower_of_hanoi    # build + run
vbr test        projects/77_tower_of_hanoi   # run the 5 tests
```
