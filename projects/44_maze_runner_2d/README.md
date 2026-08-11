# 44 — Maze Runner 2D

A playable **Godot 2D game** (GDExtension) — the classic maze runner: WASD
to move through a 12×12 maze, walls block, reaching the glowing exit wins.
Built with VBR's **Godot target**: `Node2D` blocks compiled to a Rust
cdylib that Godot 4 loads as a native extension.

## What it does

- `main.vbr` holds two nodes: **MazeRunner** (draws the maze with
  `On Draw`, moves the player on `On Process` from `Input.IsJustPressed`,
  emits a `Finished` signal on the win) and **RunnerHud** (connects to the
  signal and prints the win message).
- `maze.vbr` holds all the grid logic — maze layout, wall/exit checks,
  blocked-move rules — in plain testable VBR.

## VBR language features tested

**Godot target (first project):**
- `Node2D "MazeRunner"` block: `Export`, `Dim` members, `On Ready` /
  `On Process(delta)` / `On Draw`
- Property/method passthrough: `Me.Position`, `Me.QueueRedraw()`,
  `Me.DrawRect(Rect2(...), Color(...))`
- `Input.IsJustPressed("ui_right")`, `Vector2`, `Rect2`, `Color`
- Signals: `Signal Finished(moves As Long)`, `Emit Finished(moves)`,
  `Connect runner.Finished To OnFinished` + `Sub OnFinished(moves)`
- `Me.GetNode("../MazeRunner")` scene-tree access
- Cross-module calls from node bodies (`Maze.TryMove(px, py, dx, dy)`)

**Core language (in maze.vbr, 9 unit tests):**
- Grid as `Vec<String>`; `Mid` per-tile access; wall/exit predicates
- `TryMove` — blocked moves keep position (returns `Vec<Long>` pair)
- `Public Const TILE`

## Standard-library features tested

None — pure core language + the Godot target.

## Running it

Requires Godot 4 (on PATH as `godot4` or `GODOT4_BIN`):

```sh
vbr rungodot projects/44_maze_runner_2d    # build + open in Godot, press Play
```

The logic tests can't run through `vbr test` directly — the test harness
compiles `main.vbr` with the plain backend, which can't emit Godot imports
(spec §10 defers `.test.vbr` inside Godot projects). The logic is verified
two ways: a live self-check in `On Ready` (prints `MAZE LOGIC SELF-CHECK:
PASS` in Godot's output), and the 9 standalone tests, which run via a
plain stub main in a temp dir (see notes.md).

## Expected output

A Godot game has no stdout; the console output (self-check PASS + win
message) is the runtime evidence. The logic self-check is byte-verifiable:
`MAZE LOGIC SELF-CHECK: PASS` in Godot's output on every launch.
