# 6 — Caesar Cipher

The classic letter-shift cipher: each letter of a message is moved `key`
positions through the alphabet, wrapping at Z (ROT13 is just key 13).
Non-letters pass through unchanged. `Encrypt` shifts forward; `Decrypt`
shifts by `26 - key`.

## Bust language features tested

- `Mid(s, pos, len)` per-character access and `message.Len()`
- `Chr(code)` builtin
- `Mod` with a double-`Mod 26` idiom for positive wrap-around
- `Do While ... Loop`, `If / ElseIf / Else`
- String building with `&` in a loop
- Cross-module qualified calls (`Caesar.Encrypt(...)`) in main and tests
- Note: Bust has **no `Asc()`** and no `IIf()` — see notes.md. The cipher is
  implemented with an alphabet-string lookup (`FindIn`) instead of character
  codes, keeping it pure Bust.

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/6_caesar_cipher    # build + run
vbr test        projects/6_caesar_cipher   # run the 8 tests
```
