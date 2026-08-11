# 13 — Conway's Game of Life

A desktop GUI (`Window` on Iced) implementation of Conway's Game of Life —
the classic 2D cellular automaton. A 20×15 grid starts with a **blinker**
oscillator; the Step button advances one generation, and Blinker/Glider/
Clear reseed. All the cellular-automaton rules live in `life.vbr` and are
unit-tested; the Window renders from state.

## VBR language features tested

**GUI (`Window`):**
- Four `Button`s in a `Row`, each firing its own event
- `Canvas Board Width 400 Height 300` with a `Draw` block — grid lines via
  `Stroke Line`, live cells via `Fill Rect` iterated with `For Each` over a
  state `Vec<CellRect>` (the documented data-driven drawing pattern)
- State as `Vec<Long>` grid + precomputed `Vec<CellRect>`; canvas bodies
  don't run the resolver, so all grid maths happens in the events
- A state field's initialiser can't read a sibling field — `rects` is
  built from a fresh identical seed instead (see notes.md Quirk 47)

**Core language (in life.vbr, 8 unit tests):**
- Flat `Vec<Long>` grid, row-major; `CellAt` with bounds checks
- `CountNeighbours` — 8-cell Moore neighbourhood
- `NextGen` — the classic rules (survive 2–3, birth 3)
- Seeds: `BlinkerSeed`, `GliderSeed`; `CountLive` status helper
- `LiveRects` — grid → renderer rects

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/13_game_of_life    # build + open the window
vbr test        projects/13_game_of_life   # run the 8 logic tests
```

## Expected output

A `Window` has no stdout; `expected_output.txt` documents that. The window
was verified by launch + screenshot under WSLg/X11 (title, status
"Generation 0 — 3 live cells", four buttons, blinker rendered on the
canvas). Blinker oscillation is verified by the unit tests
(horizontal → vertical → horizontal).
