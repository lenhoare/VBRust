# 42 — Magic Fortune Ball

A desktop GUI (`Window` on Iced) version of the Magic 8-Ball toy: click
**Shake** and a fortune appears. The answer list is deterministic (chosen
by shake count) so the demo and tests agree. All logic lives in
`fortune.vbr` and is unit-tested.

## Bust language features tested

**GUI (`Window`):**
- `Button "Shake"` + `On Click ShakeBall` — a single-button app
- `Text` answer line updated from state
- A Window has no stdout — see expected output below

**Core language (in fortune.vbr, 4 unit tests):**
- `Vec<String>` answer list, `.Clone()` on element read (Quirk 30)
- `Mod` cycling; `Public Function` returning `String`

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/42_magic_fortune_ball    # build + open the window
vbr test        projects/42_magic_fortune_ball   # run the 4 logic tests
```

## Expected output

A `Window` has no stdout; `expected_output.txt` documents that. The window
was verified by launch + screenshot under WSLg/X11 (title, prompt text,
Shake button).
