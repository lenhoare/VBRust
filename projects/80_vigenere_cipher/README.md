# 80 — Vigenère Cipher

Polyalphabetic substitution cipher: each letter is shifted by a per-position
key derived from a repeating keyword (A=0, B=1, ... Z=25). `Encrypt` shifts
forward, `Decrypt` shifts backward. Non-letters pass through unchanged and
do **not** advance the key position. Case is preserved.

## VBR language features tested

- `Public Function` with a `Boolean` mode flag
- Alphabet-string lookup (`FindIn` via `Mid`) — no `Asc()` needed
- `Mod` wrap-around for both key position and letter index
- Case preservation via `LCase()` comparison
- `Mid(keyword, (keyPos Mod keyword.Len()) + 1, 1)` — wrapping key access
- Cross-module qualified calls in main and tests
- Known-answer test: `Encrypt("ATTACKATDAWN", "LEMON") = "LXFOPVEFRNHR"`

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/80_vigenere_cipher    # build + run
vbr test        projects/80_vigenere_cipher   # run the 6 tests
```
