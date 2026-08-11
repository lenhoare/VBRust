# 49 — Multiplication Table

Prints the classic 0×0 to 12×12 multiplication grid, each product
right-justified into a 4-character cell. The book's version is a
pure-formatting exercise; here the grid is built by a `multable.vbr` module
(rows as `Vec<String>`) so the formatting logic is directly testable.

## VBR language features tested

- Nested `For` loops (`For r ... Next` / `For c ... Next`)
- `Vec<String>` with `Push`, `Len()`, bracket indexing `rows[i]`
- `Do While ... Loop` used as a padding loop
- `&` concatenation, `CStr()` number→string
- `For Each` over a `Vec` (with `.Clone()` when keeping the element)
- `Assert vec = [...]` list equality and `rows[i]` indexing in tests

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/49_multiplication_table    # build + run
vbr test        projects/49_multiplication_table   # run the 5 tests
```
