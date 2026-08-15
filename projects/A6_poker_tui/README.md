# A6 — Poker TUI: Texas Hold'em Equity Calculator

A terminal TUI (`Screen`) that models Texas Hold'em and estimates your
probability of having the **winning hand** at each stage of the board.

## What it does

1. Type your two **hole cards** (e.g. `Ah Kd`) and the number of **players**.
2. Press `f` (flop, 3 cards), `t` (turn, +1), `r` (river, +1) to reveal the
   board in stages.
3. After each stage the app shows **"N% to win vs M players"** — the Monte
   Carlo equity: random opponent hands and the remaining board are dealt
   many times and the fraction where your best hand beats every opponent is
   reported.

The simulation is **deterministic** (fixed-seed LCG), so the same cards and
stage always give the same number.

## The hand evaluator

`poker.vbr` implements a real 7-card Texas Hold'em evaluator:

- Best 5-card hand from any 5–7 cards (tries all C(7,5)=21 combinations)
- Full category ladder: high card → pair → two pair → trips → straight →
  flush → full house → quads → straight flush, including the **wheel**
  (A,2,3,4,5) as a 5-high straight
- Scores are a single comparable `Long` (category × 15⁵ + base-15 rank
  signature), so "better hand" is just `>` — and equity simulation compares
  scores directly

## Bust language features tested

**TUI (`Screen`):**
- Three `Input` widgets (card text, players) with `On Submit`; Tab cycles
  focus between them
- Keymap: `f`/`t`/`r` reveal the board, `q` quits
- State as `Vec<Card>` (the board), pre-rendered status/equity lines
- Events can't call other events (Quirk 29) — shared logic lives in
  `poker.vbr` as `Public Sub`s (`TrySetup`, `ShowEquity`) mutating state ByRef

**Core language (in poker.vbr, 15 unit tests):**
- `Public Type Card` + `Public Enum`-free design; `Vec<Card>` deck
- Fisher-Yates shuffle with a fixed-seed LCG (`NextInt`)
- The full hand evaluator (`Score5`, `BestScore`) and the category ladder
- Monte Carlo `WinProbability` with per-trial shuffled deck slices
- `.Clone()` discipline on `Vec<Card>` element reads (Quirk 30)

## Standard-library features tested

None — pure core language.

## Running it

```sh
vbr runproject projects/A6_poker_tui    # run the TUI (f/t/r, q quit)
vbr test        projects/A6_poker_tui   # run the 15 logic tests
```

## Expected output

`expected_output.txt` is the captured 80x24 frame at the deterministic
startup state (empty inputs, default 2 players). Captured with `tmux
capture-pane`, byte-identical across runs. Interactive play was verified:
Ah Kd vs 2 players, flop `7h Js 6c` → **63% to win**.
