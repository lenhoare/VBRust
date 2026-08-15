# 5 — Bouncing DVD Logo

The classic DVD screensaver as a terminal TUI (`Screen`): a "DVD" logo
bounces diagonally inside a fixed field, reflecting off the walls, with a
corner-hit counter. Driven by an `Every` timer. All the motion math lives in
`bdvd.vbr` and is unit-tested; main.vbr only renders it.

## Bust language features tested

**TUI (`Screen`):**
- `Every 150 Tick` — timer-driven animation (the screen redraws on its own)
- `Public Type Logo` held in State (a project-global type, by bare name)
- `Text` showing state fields; `On Key "q" Quit`
- State mutation in an event: `logo = Bdvd.Advance(logo, w, h)` and a
  counter

**Core language (in bdvd.vbr, unit-tested):**
- `Public Type Logo` with four `Long` fields, built complete via the
  `Logo { x: ..., y: ..., dx: ..., dy: ... }` constructor
- `Public Function` taking a struct `ByVal` and returning a new struct
  (`Advance`) — struct-in, struct-out, no mutation
- `Public Function` returning `Boolean` (`AtWall`)

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/5_bouncing_dvd    # run the TUI (q to quit)
vbr test        projects/5_bouncing_dvd   # run the 6 logic tests
```

## Expected output

`expected_output.txt` is the captured 80x24 frame at ~8s of animation
(position 28,6, corner hits 5) — byte-identical across three runs at that
sleep. Caveat: an *animation* has no stable terminal state, so the frame is
reproducible only with the same capture delay; the genuinely deterministic
regression artifact is the logic test suite, and the README notes this.
