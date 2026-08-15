# 48 — Monty Hall

The classic probability paradox as a desktop GUI (`Window` on Iced). Three
doors hide one car and two goats: pick a door, the host opens a goat door,
then stay or switch. Scores track wins/losses across rounds. All rules
live in `monty.vbr` and are unit-tested; the Window is a state machine
over three phases (pick → stay/switch → reveal).

## Bust language features tested

**GUI (`Window`):**
- `Button "Door 1"` … `On Click Pick0` — three pick events; a `Row` of
  buttons
- **Conditional view** — `If phase = 0 Then … End If` shows different
  widgets per phase (buttons swap out between pick / stay-switch / reveal)
- `Text` lines, `Column` + `Row` + `Spacing` layout
- Events as the only place state changes; the shared logic is a `Public
  Sub` called qualified (events can't call other events — same rule as
  TUI, Quirk 29)
- Note: a `Window` replaces `Function Main()` with `iced::run` — there is
  **no stdout** (see expected output below)

**Core language (in monty.vbr, 5 unit tests):**
- `Public Const`; `Public Function` returning `Long`/`Boolean`
- `Mod`-seeded car placement; a `For` scan for the host's goat door
- `Public Sub PickDoor(ByRef …)` mutating six state fields in place

## Standard-library features tested

None — pure core language.

## Running it

A `Window` opens a real desktop window:

```sh
vbr runproject projects/48_monty_hall    # build + open the window
vbr test        projects/48_monty_hall   # run the 5 logic tests
```

## Expected output

A `Window` has no stdout (the generated `fn main` calls `iced::run`), so
`expected_output.txt` documents this instead of program text. The window
was verified by launch + screenshot under WSLg/X11 (title "Monty Hall",
three door buttons, phase-0 state) — see the notes.md GUI entry for the
verification recipe.
