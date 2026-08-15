# 31 — Guess the Number

A terminal TUI (`Screen`) version of the classic number-guessing game: the
computer picks 1..100, the player types guesses into an **Input** widget and
presses Enter; the hint line says "too high" / "too low" / "correct", ten
guesses allowed. First project to exercise TUI text input.

## Bust language features tested

**TUI (`Screen`):**
- `Input <field>` widget with `On Submit` — the event receives the typed
  text as a parameter
- Focus ring (the Input is the only focusable widget here)
- `Text` lines, `Column` + `Spacing` layout, `On Key` bindings, `Quit`
- View reads a mirrored state field (`remaining`) — cross-module calls are
  not allowed in views
- Events can't use a bare `Return` (they lower to `Result<()>`) — the game
  logic is an If/ElseIf chain instead

**Core language (in guess.vbr, unit-tested):**
- `Public Const`; `Public Function` returning `Long`/`String`
- `Val()` for parsing the typed number; deterministic seeded secret

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/31_guess_the_number    # run the TUI (q to quit)
vbr test        projects/31_guess_the_number   # run the 5 logic tests
```

## Expected output

`expected_output.txt` is the captured 80x24 TUI frame at the deterministic
startup state (secret seeded, 10 guesses left, empty input). Captured with
`tmux capture-pane`, byte-identical across runs. A Screen has no stdout;
interactive frames were also verified manually (typing "50" → "too low — 9
guesses left").
