# 57 — Progress Bar

A terminal TUI (ratatui `Screen`) that simulates a file download with a
live progress **Gauge**. This is the natural Bust home for the book's
"download task simulation" — a single-line animated progress bar becomes a
real gauge widget with a status line, refreshed by an `Every` timer.

## Bust language features tested

**TUI (`Screen`)** — this project is the first in the series to use the
terminal interface:
- `Screen` / `State` / `View` / `Event` — the Elm-architecture core
- `Gauge 0..=100, field` widget bound to a state field
- `Every 200 Tick` — timer-driven animation
- `On Key "q" Quit` — keyboard binding (Quit is built in)
- `Column` + `Spacing` layout
- Cross-module calls from events (`Progress.NextChunk(tick)`)
- Note: view expressions can't call modules directly, so the gauge reads a
  state field computed in the event

**Core language (in progress.vbr, fully unit-tested):**
- `Public Function` returning `Long`/`String`/`Boolean`
- `Round()` for percentage; a Double-widening idiom for true float division
- A deterministic pseudo-random chunk walk (`tick * 7 Mod 5 + 1`)

## Standard-library features tested

None — pure core language.

## Running it

A `Screen` takes over the terminal, so run it in a real terminal (not a
pipe):

```sh
vbr runproject projects/57_progress_bar    # run the TUI (q to quit)
vbr test        projects/57_progress_bar   # run the 5 logic tests
```

## Expected output

`expected_output.txt` holds the **captured TUI frame at 80x24** once the
simulated download has completed (the stable final state — the gauge reads
100%, status "45 MB / 45 MB"). A Screen has no stdout to diff, so the frame
was captured with `tmux capture-pane` after the animation settled; it is
byte-identical across runs. Capturing an *intermediate* frame would be
timing-dependent, so the completed state is the deterministic artifact.
