# 1 — Bagels

A deductive logic game from *The Big Book of Small Python Projects*. The
computer thinks of a secret three-digit number; for each guess it gives
clues: **Fermi** (a correct digit in the correct place), **Pico** (a correct
digit in the wrong place), or **Bagels** (no correct digits).

This example is a **deterministic** version for regression testing: the
secret is fixed (`"123"`) and the guesses are a fixed list, so the output is
reproducible. (The book version uses a random secret and interactive input.)

## Bust language features tested

- `Public Function` returning `String`, called cross-module qualified as
  `Bagels.GetClue(secret, guess)`
- `Mid(s, pos, len)` — the VB6-style substring builtin (1-based, char-counted)
- `Vec<Boolean>` flags with **bracket indexing** `used[i]` (this is Bust, not
  VB6 — `used(i)` fails)
- Nested `For` loops with `Exit For`, `If / And / Not`
- `&` concatenation; building a result list with `Vec.Push` and joining it
  in a `For Each` loop (`.Clone()` needed — `For Each` borrows)
- `CStr()` for number → string conversion

## Standard-library features tested

None — pure core language.

## Running it

From the repository root:

```sh
vbr runproject projects/1_bagels    # build + run the program
vbr test        projects/1_bagels   # run the 7 tests
```

## Expected output

```
I am thinking of a three-digit number. Try to guess what it is.
Here are some clues:
When I say:    That means:
  Pico         One digit is correct but in the wrong position.
  Fermi        One digit is correct and in the right position.
  Bagels       No digit is correct.

Guess #1: 111
Fermi
Guess #2: 212
Pico Pico
Guess #3: 321
Fermi Pico Pico
Guess #4: 123
Fermi Fermi Fermi
You got it!
```
