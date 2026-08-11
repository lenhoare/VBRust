# 3 — Bitmap Message

Displays a text message arranged in a 2D pattern: every non-space character
of a bitmap is replaced by the next character of the message (repeating the
message as needed); spaces stay spaces. The bitmap and result are
`Vec<String>` rows. Deterministic.

## VBR language features tested

- `Public Function` taking a `Vec<String>` argument, returning `Vec<String>`
- `For Each` over a `Vec<String>`; inner `For` with `Mid`/`Len()` per char
- Message wrap-around via a counter that resets to 0 at `message.Len()`
- `&` string building
- `Assert func(...)[0] = "..."` — indexing a returned Vec directly in a test

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/3_bitmap_message    # build + run
vbr test        projects/3_bitmap_message   # run the 5 tests
```
