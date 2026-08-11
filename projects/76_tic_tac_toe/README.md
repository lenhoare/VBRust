# 76 — Tic-Tac-Toe

The classic 3×3 game as a terminal TUI (`Screen`). Two players take turns
placing X and O using the number keys 1–9 (keypad layout: 1=top-left …
9=bottom-right); the board renders as `a b c | d e f | g h i`. Win, draw,
and illegal moves are handled; `r` restarts, `q` quits. All rules live in
`ttt.vbr` and are unit-tested; main.vbr only renders and wires keys.

## VBR language features tested

**TUI (`Screen`):**
- Keymap with digit keys (`On Key "1" Move1` … `On Key "9" Move9`)
- State holding a `Vec<String>` board; the board text pre-rendered into a
  state field (`display`) because views can't call functions
- Events calling a shared `Public Sub` qualified (`Ttt.TryMove(board, …)`)
  — events can't call other events, so the shared logic lives in the module
- `On Key "r" Restart`, `On Key "q" Quit`

**Core language (in ttt.vbr, 11 unit tests):**
- `Vec<String>` board, bracket indexing, `.Clone()` when returning an
  element (a borrowed Vec can't move a String out)
- `Public Sub TryMove(ByRef board As Vec<String>, …)` mutating state in
  place — the life_screen pattern
- `Public Function` returning `Vec<String>` (fresh board on `Place`)
- Win detection via an explicit `LineWins` helper — a `Vec<Vec<Long>>` of
  winning lines mis-compiles (see notes.md Quirk 28)

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/76_tic_tac_toe    # run the TUI (1-9 place, q quit)
vbr test        projects/76_tic_tac_toe   # run the 11 logic tests
```

## Expected output

`expected_output.txt` is the captured 80x24 frame at the deterministic
startup state (empty board, "X to move"). Captured with `tmux
capture-pane`, byte-identical across runs. A Screen has no stdout;
interactive play was also verified (moves 1,5,2,3,6 → board `X X O | - O X
| - - -`).
