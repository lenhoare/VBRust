# 61 — ROT13

The ROT13 cipher: every letter rotated by exactly 13 positions. Because the
alphabet has 26 letters, applying ROT13 twice returns the original — the
same function both encrypts and decrypts. Case is preserved; numbers and
punctuation pass through.

## VBR language features tested

- `Mid` / `Len()` / string building with `&`
- `Mod` for wrap-around
- `Do While ... Loop` used as a linear search (`FindIn`)
- Cross-module qualified calls in main and tests
- Self-inverse property asserted directly in tests (`Rot13(Rot13(x)) = x`)

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/61_rot13    # build + run
vbr test        projects/61_rot13   # run the 6 tests
```
