# 50 — Ninety-Nine Bottles

The classic folk-song stanza generator. Prints a shortened run (5 down to 0)
with correct singular/plural handling. The lyric-building logic lives in
`bottles.vbr` so each stanza shape is directly testable.

## VBR language features tested

- `Public Function` returning `Vec<String>` of lines
- `If / ElseIf / Else` branching on the bottle count
- `For ... Step -1` — counting down (Rust reverses the range for you)
- `&` concatenation with `CStr(n)` number→string
- `For Each` over a `Vec<String>` in main
- `Assert lines[i] = "..."` — indexing a Vec of Strings in tests

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/50_ninety_nine_bottles    # build + run
vbr test        projects/50_ninety_nine_bottles   # run the 5 tests
```
