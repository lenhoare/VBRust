# 14 — Countdown

A terminal TUI (`Screen`) countdown timer with classic **seven-segment**
text graphics: a fixed 90-second countdown, rendered as segment rows,
start/pause on Space, reset on `r`. The segment shapes and the
countdown/format logic live in `count.vbr` and are unit-tested; the Screen
only owns the timer.

## Bust language features tested

**TUI (`Screen`):**
- `Every 1000 Tick` — one-second timer; `On Key " " Toggle`, `"r" Reset`
- All rendered text pre-computed into state fields (`segRow1..3`,
  `statusText`) — views can't call functions (Quirk 26)
- Events inline their refresh (events can't call events, Quirk 29)

**Core language (in count.vbr, unit-tested):**
- `Type Seg` (struct) returned from a `Match` — the 7-seg shapes
- `FormatTime` — `/` and `Mod` for M:SS; zero-padding with `&`
- `SegRows` — per-character loop building three rows, joining with `&`
- `Val(ch)` to convert a digit char to Long

## Standard-library features tested

None — pure core language. (The seven-segment shapes mirror project 64.)

## Running it

```sh
vbr runproject projects/14_countdown    # run the TUI (space start, q quit)
vbr test        projects/14_countdown   # run the 6 logic tests
```

## Expected output

`expected_output.txt` is the captured 80x24 frame at the deterministic
startup state (paused at 1:30, "Seconds left: 90"). Captured with `tmux
capture-pane`, byte-identical across runs. Interactive ticking was also
verified (Space → 3s later the display shows 1:27).
