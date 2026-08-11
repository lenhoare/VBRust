# 20 — Digital Stream

The Matrix-style "digital rain" as a terminal TUI (`Screen`): columns of
cascading 1s and 0s animate down the screen, each column with its own head
position and speed, driven by an `Every` timer. The column simulation
lives in `stream.vbr` and is unit-tested; the Screen renders one frame per
tick.

## VBR language features tested

**TUI (`Screen`):**
- `Every 120 Tick` — fast animation timer
- `Vec<StreamColumn>` in State, mutated per tick (`cols[i] =
  Stream.StepColumn(cols[i], height)`)
- `Length 14` size constraint so the frame `Text` gets enough rows
- Multi-line frame pre-rendered into a state field (`vbLf`-joined)

**Core language (in stream.vbr, unit-tested):**
- `Public Type StreamColumn` with Head/Speed; struct-in/struct-out
  `StepColumn` (no mutation)
- `IsLit` — modular trail logic (wrap-around at the top)
- `NewColumns(count)` factory for the State initialiser
- `BitChar` — deterministic per-row bit pattern

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/20_digital_stream    # run the TUI (q to quit)
vbr test        projects/20_digital_stream   # run the 6 logic tests
```

## Expected output

`expected_output.txt` is the captured 80x24 frame at ~8s of animation
(byte-identical across two runs at that sleep). Caveat as with other
animations: the exact frame is timing-dependent; the deterministic
regression artifact is the logic test suite.
