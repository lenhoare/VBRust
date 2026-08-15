# 34 — Hangman

The classic word-guessing game as a desktop GUI (`Window` on Iced). A
secret word is shown masked; 26 letter buttons reveal it one guess at a
time. Six wrong guesses and you're hanged. Words pick deterministically by
round. All rules live in `hangman.vbr` and are unit-tested.

## Bust language features tested

**GUI (`Window`):**
- 26 `Button` widgets in two `Row`s; each has its own event calling the
  shared `Hangman.Guess` Sub qualified
- Conditional view — the letter grid shows only while `phase = 0`, a
  "Play again" button appears after the game ends
- `Text` masked word from a state field (views can't call module
  functions, Quirk 26 — the mask is recomputed inside `Guess`)
- No stdout — see expected output below

**Core language (in hangman.vbr, 5 unit tests):**
- `Public Function` returning `String`/`Boolean`/`Long`
- `Mask` — per-character reveal with `Mid`; `Contains` Mid-scan
- `Public Sub Guess(ByRef word, ByRef guessed, ByRef masked, ByRef phase,
  ByRef message, ByVal letter)` — the shared-Sub pattern
- `.Clone()` on Vec element reads (Quirk 30)

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/34_hangman    # build + open the window
vbr test        projects/34_hangman   # run the 5 logic tests
```

## Expected output

A `Window` has no stdout; `expected_output.txt` documents that. The window
was verified by launch + screenshot under WSLg/X11 (masked word, 26 letter
buttons in two rows).
