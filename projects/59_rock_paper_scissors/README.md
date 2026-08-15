# 59 — Rock Paper Scissors

The classic hand game as a terminal TUI (`Screen`): press `r`/`p`/`s` to
play Rock/Paper/Scissors against the computer, with running scores for
player, computer and ties. The computer's move is deterministic (seeded by
round), so tests and demo agree. All rules live in `rps.vbr` and are
unit-tested.

## Bust language features tested

**TUI (`Screen`):**
- Single-character keymap (`On Key "r" PlayRock`, `"p"`, `"s"`, `"q"`)
- Score/ties/message in State; events call the shared logic qualified
- Events can't call other events (Quirk 29) — the three key events all call
  `Rps.Play(round, playerScore, computerScore, ties, message, "R"|"P"|"S")`

**Core language (in rps.vbr, 7 unit tests):**
- `Public Function` returning `String` for move/result/name
- `Public Sub Play(ByRef round As Long, … ByVal choice As String)` mutating
  five state fields in place — the Tic-Tac-Toe pattern
- `Mod`-based deterministic computer move
- `If / ElseIf` outcome tree

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/59_rock_paper_scissors    # run the TUI (r/p/s, q quit)
vbr test        projects/59_rock_paper_scissors   # run the 7 logic tests
```

## Expected output

`expected_output.txt` is the captured 80x24 frame at the deterministic
startup state ("Press r, p or s to play.", zeroed score). Captured with
`tmux capture-pane`, byte-identical across runs. Interactive play was also
verified (r, s, p → the final round correctly reported "computer wins").
