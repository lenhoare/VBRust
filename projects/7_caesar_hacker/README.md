# 7 — Caesar Hacker

Brute-force cryptanalysis: tries all 26 possible Caesar keys on an
encrypted message and scores each candidate for "English-ness" using a
small common-word list, then reports the best guess. Self-contained (it
re-implements the Caesar shift locally — each project folder is its own
world).

## Bust language features tested

- `Public Function` returning `Vec<String>` of all 26 candidates
- Alphabet-string lookup (`FindIn` + `Mid`) for the shift — same idiom as
  the Caesar project, local copy here
- `LCase()` + a substring `Contains` scan (Mid-based) for word detection
- A scoring loop tracking `bestKey`/`bestScore` with `For` and `If`
- Cross-module qualified calls in main and tests
- Known-answer test: `BestGuess("QEB ...")` returns the key-23 plaintext

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/7_caesar_hacker    # build + run
vbr test        projects/7_caesar_hacker   # run the 5 tests
```
