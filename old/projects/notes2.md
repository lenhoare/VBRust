#

### Quirk 25 — [DOC GAP] Events can't use a bare `Return`

```vb
Event TryGuess(text As String)
    If won Then
        Return              ' � ✘ `return;` in a function whose return type is not `()`
    End If
```

Screen events lower to `fn ... -> std::io::Result<()>`, so a bare `Return`
in an event is a type error. Workaround: restructure as If/ElseIf so the
event always falls through (what the game does). Worth a line in the TUI
spec's Events section.

### Quirk 26 — [CONFIRMED] View expressions can't call cross-module functions

`Text "Guesses left: " & Guess.Remaining(...)` in a View fails at rustc
level ("expected value, found module `guess`") — the view doesn't run the
resolver, exactly as the spec's life_screen note warns. Workaround: compute
into a state field in the event, bind the widget to the field.

### What worked

- `Input entry` + `On Submit TryGuess` — typed text arrives as the event's
  `text As String` parameter; Backspace/Enter behave.
- `Val(text)` parsing of the input, deterministic `SecretFor(seed)`.
- Tab focus is trivial here (single input), but the wiring is in place.

---

## 5 — Bouncing DVD Logo (TUI) (2026-08-07)

Project: `projects/5_bouncing_dvd/`. 6/6 logic tests pass; the TUI animates
under tmux (position moved 16,4 → 2,10 between captures; corner counter
incremented; `q` quits).

### Quirk 27 — [MISSING] `Step` is a reserved word for function names too

```vb
Public Function Step(...)    ' � ✘ Expected a name for the function, found Step
```

Like `To` (Quirk 17) and `Sub` (Quirk 23), `Step` is reserved (For loops).
Renamed to `Advance`.


### Quirk 28 — [BUG] `For Each` over a nested `Vec<Vec<Long>>` mis-compiles

```vb
Dim lines As Vec<Vec<Long>> = [[0,1,2], ...]
For Each line In lines
    Dim a As Long = line[0]    ' � ✘ type `i64` cannot be dereferenced
Next
```

Iterating a `Vec<Vec<Long>>` with `For Each` and indexing the inner Vec
fails at rustc ("type `i64` cannot be dereferenced") — the transpiler emits
a bad deref for the inner indexing. Workaround: flatten to explicit checks
via a helper (`LineWins(board, a, b, c)` called eight times), or index the
outer Vec with a numeric `For` instead of `For Each`.

### Quirk 29 — [CONFIRMED] Events can't call other events

`Event Move1` calling `TryMove(0)` (a sibling event) fails: "cannot find
function `trymove` in this scope". Events lower to handler methods, not
callable functions. Workaround: put the shared logic in a `Public Sub` in
the logic module and call it qualified from each event (the life_screen
`Life.SetCell(grid, …)` pattern) — `Ttt.TryMove(board, moves, over,
message, cell)`.

### Quirk 30 — [BUG] Returning a Vec element needs `.Clone()`

```vb
Public Function Winner(...) As String
    Return board[0]    ' � ✘ cannot move out of index of `Vec<String>`
```

Indexing a `ByVal Vec<String>` (a `&Vec` borrow) and returning the element
fails — the element must be cloned: `Return board[0].Clone()`.

### What worked

- `Public Sub TryMove(ByRef board As Vec<String>, ByRef moves As Long,
  ByRef over As Boolean, ByRef message As String, ByVal cell As Long)`
  mutating four state fields in place from events — the ByRef path handles
  `Vec` and scalars alike.
- Nine near-identical key events are verbose but the pattern is dead
  simple; a `Match`-key or per-cell loop would need resolver help.

---


### Quirk 31 — [BUG] `vbCrLf` in a `&` chain produces a bare CR in format!

```vb
out = out & vbCrLf & line    ' � ✘ bare CR not allowed in string, use `\r` instead
```

`vbCrLf` lowers to a literal `\r\n` inside the `format!` string, and Rust
rejects the raw CR. Workaround: use `vbLf` (lowers to `\n`), which is what
a TUI wants anyway. (Quirk: the constant exists but is only safe outside
format strings.)

### Notes

- A `Text` widget defaults to one row — a multi-line frame needs a
  `Length N` (or `Fill`) size line, or only the first line shows.
- Events still can't call events (Quirk 29) — the tick inlines both the
  advance and the render loops.
- State initialisers CAN call module functions (`Dim cols As
  Vec<StreamColumn> = Stream.NewColumns(16)`), like `Life.NewGrid`.

---


### Quirk 32 — [CONFIRMED] `move` is a reserved word for parameter names too

`ByVal move As String` fails exactly like Hanoi's `Function Move` (Quirk
18) — `move` is a Rust keyword and the transpiler doesn't escape it.
Renamed to `choice`.

### Notes


### Quirk 33 — [CONFIRMED] Window events can't call other events (same as TUI)

`Event Pick0` calling a sibling `PickDoor(0)` fails ("cannot find function
`pickdoor`"). Same rule as Quirk 29 — put shared logic in a `Public Sub`
in the logic module and call it qualified: `Monty.PickDoor(round, car,
pick, host, phase, message, 0)`. The GUI and TUI event systems share this
behaviour.

### Quirk 34 — [BUG?] `vbr build` on a GUI project only generates; `cargo build` compiles

`vbr build projects/48_monty_hall` produced the project + Cargo.toml but
did not compile the binary; the Iced deps were cached so `cargo build` in
the project's `build/` dir took 3s. (The example run earlier used
`runproject`, which builds.) Worth confirming `vbr build` is meant to be
generate-only.


### Quirk 35 — [BUG?] HashMap method args don't auto-borrow a String key

```vb
Dim ratings As HashMap<String, Long>
Dim home As String = homes[i].Clone()
ratings.get(home)                ' � ✘ expected `&_`, found `String`
ratings.contains_key(home)       ' � ✘ expected `&_`, found `String`
```

`HashMap.get` / `contains_key` take a reference (`&str`), and the
transpiler borrows *literals* fine (`ages.get("Alice")` in the example
works) but not a `String` *variable* — no `&home` is emitted. The `get`
result is also `&i64`, so `.Unwrap()` can't widen. Workaround: keep a
`Vec<Rating>` registry with a linear `FindTeam` scan instead (20 teams ×
760 matches is trivial) — what elo.vbr does. Worth a resolver fix:
borrow String args to HashMap methods like literals.

### Quirk 36 — [BUG] A `Const` assigned into a `Double` doesn't auto-widen

```vb
Public Const K_FACTOR As Long = 32
Dim k As Double = K_FACTOR      ' � ✘ expected `f64`, found `i64`
```

The "assign a Long into a Double and you'll see `as f64`" teaching note
applies to *variables*, not `Public Const`s — the constant arrives as i64
and isn't cast. Workaround: use the numeric literal (`32.0`). Also, `i64 *
f64` in an expression never compiles — widen before multiplying.

### Quirk 37 — [MISSING] `DataFrame.Read_Csv` aborts on `N/A` strings

The raw `premier_league_23-26.csv` has `N/A` in numeric columns
(`attendance`, `Game Week`); the stdlib `read_csv` (dataframe.rs:29) has
no null-values/ignore-errors option, so the whole read panics. Workaround:
preprocess a trimmed copy (`pl_trim.csv`, 5 clean columns, via Python).
Worth a `null_values`/`ignore_errors` read option in a later slice.


### Quirk 38 — [BUG] `Count()` aggregates to u32 — can't extract as `Vec<Long>`

```vb
Dim byPb As DataFrame = df.Group_By("pb").Agg(Count(w1))
Dim counts As Vec<Long> = byPb.Column("w1")   ' � ✘ expected Int64, got u32
```

polars' `count()` returns a u32 column, and the stdlib `FromColumn for
i64` (dataframe.rs:267) refuses anything but Int64. Workaround: write the
grouped frame with `Write_Csv` and read it back — polars reinfers the
counts as i64, so `Column("w1")` works. (That round-trip also exercises
the Write_Csv verb, so it's not pure waste.)

### Quirk 39 — [BUG] Group_By AND Agg on the same column collides

```vb
df.Group_By("w1").Agg(Count(w1))    ' � ✘ column with name 'w1' has more than one occurrence
```

Grouping by `w1` and aggregating `w1` in one call duplicates the output
name (no aliases yet — spec §8). Workaround: aggregate a *different*
column (`Count(w2)`), which is fine for counting.

### Quirk 40 — [CONFIRMED] A `Dim`'d name inside `Agg` becomes a *value*, not a column

```vb
Dim w1 As Vec<Long> = df.Column("w1")
df.Group_By("pb").Agg(Count(w1))    ' � ✘ Vec<i64> doesn't implement Literal
```

The column-formula rule ("Dim'd names are values") bites: if you've
extracted `w1` into a variable, `Count(w1)` in Agg tries `lit(w1)` — a
Vec. Workaround: extract columns under *different* names (`white1`,
`powerball`) so the bare `w1` inside Agg stays a column reference. This is
a real footgun for the read→analyse pipeline: you can't extract and
aggregate the same column in one program without renaming.


### Quirk 41 — [CONFIRMED] `Dim` is a reserved word as a variable name

```vb
Dim dim As Long = DaysInMonth(...)    ' � ✘ Expected a name after `Dim`, found Dim
```

Naming a variable `dim` fails at parse ("Expected a name after Dim, found
Dim") — the keyword list includes `Dim` itself. Renamed to `days`. (No
diagnostic, just a confusing parse cascade — worth a reserved-name check
like the type-name one.)

### Quirk 42 — [DOC GAP] `vbr test` can't compile a Godot main.vbr

The test harness (`generate_project`) compiles every module with the plain
backend, so a `main.vbr` containing `Node2D` blocks fails ("cannot find
module `godot`"). Spec §10 lists ".test.vbr modules inside a Godot
project" as deferred. Workaround used here: a live self-check inside
`On Ready` (prints PASS/FAIL to Godot's output), plus the logic tests run
in a temp dir with a plain stub `main.vbr`. Worth a harness note: skip or
route Godot main.vbr in `cmd_test`.

### Quirk 43 — [BUG?] Sub handlers are only emitted when a Signal exists

A `Sub SelfCheck()` inside a Node2D with no signals isn't emitted at all —
`Sub` handlers lower to `#[func]`s only when there's a `Connect … To`
wiring (godot.rs emits the handler impl only if `!signals.is_empty() ||
!handlers.is_empty()`). Calling a signal-less Sub fails ("cannot find
value"). Workaround: inline the logic into `On Ready`, or declare a dummy
signal so the handler impl is emitted.

### Quirk 44 — [BUG] Field reads inside `Me.DrawRect(...)` args fail the borrow

```vb
Me.DrawRect(Rect2(x * TileSize, ...), ...)   ' � ✘ cannot use self.tilesize
'                                             because it was mutably borrowed
```

The spec says property *writes* are hoisted past the borrow, but reading a
member field (`TileSize`, `px`, `py`) inside a `Me.<method>` argument
still trips rustc's borrow checker (the `BaseMut` temporary lives across
the read). Workaround: copy fields to locals before the call
(`Dim ts As Single = TileSize`, `Dim lx As Long = px`).


### Quirk 45 — [CONFIRMED] `Step` is a reserved word (again)

`On Click Step` / `Event Step` fails like Quirk 27's `Function Step` —
renamed to `StepOnce`. The reserved list (`To`, `Sub`, `Move`, `Dim`,
`Step`, …) keeps growing; worth a single documented list.

### Quirk 46 — [CONFIRMED] Named colours are a fixed list

`Color.LightGray` and `Color.ForestGreen` are rejected — the named set is
exactly: Black, White, Red, Green, Blue, Gray, Yellow, Orange, Purple,
Navy, Cyan, Magenta. Use `Color(r, g, b)` for anything else, and note it
takes **3** args, not 4 (no alpha).

### Quirk 47 — [BUG?] A state field initialiser can't read a sibling field

```vb
State
    Dim grid As Vec<Long> = Life.BlinkerSeed(20, 15)
    Dim rects As Vec<CellRect> = Life.LiveRects(grid, 20)   ' � ✘ cannot find value `grid`
End State
```


### Quirk 48 — [MISSING] `vbr py` has no multi-module project mode

`cmd_py` reads and transpiles **one file**; a project folder is rejected
("Is a directory"). There's no parallel of `runproject` for the Python
target — sibling modules aren't compiled, so a `main.vbr` calling
`Periodic.ParseRow` emits Python that references an undefined
`periodic.parserow`. Workaround: keep the multi-module version for Rust
and maintain a single-file variant (`py/main.vbr`) for the Python target.
Worth a spec note: `vbr py <folder>` should behave like `runproject`.

### Quirk 49 — [MISSING] Python backend lacks the VB string builtins

A probe showed the Python target lowers `.Len()` → `len()` (works) but
passes `Mid`, `Left`, `Right`, `Val`, `UCase`, `LCase`, `Chr` straight
through as **undefined names** (`NameError: name 'mid' is not defined`).
The Rust backend has all of these; the Python `call()` table only covers
maths builtins, `CStr`, `Sleep`. Workaround: avoid them in py-target
source (hardcode tables, use `.Len()`/`&`/comparisons only), or write a
small helper in the program. This is a real coverage gap worth closing —
string code is the first thing a Python-target program reaches for.

### Quirk 50 — [CONFIRMED] `ByRef` scalar params can't be emulated in Python

```vb
Sub SetFlag(ByRef found As Boolean)   ' ⚠ assignment won't reach the caller
```

The transpiler warns: "`ByRef` parameter can't be emulated for a scalar in
Python — passed by value". Workaround: return a sentinel value instead of
a ByRef flag (the `NoneElement()` pattern — empty Element means "not
found").


### Quirk 51 — [CONFIRMED] `vbr c` is single-file only, like `vbr py`

`vbr c <folder>` is rejected ("Is a directory") and a multi-module
main.vbr references sibling functions that don't exist in the emitted C.
Same gap as Quirk 48 — the alternative targets have no `runproject`
equivalent. Workaround: single-file variant in a `c/` subfolder.

### Quirk 52 — [CONFIRMED] C backend lacks the VB string builtins too

`Mid`/`UCase`/`Left` in a C-target program become implicit-declaration
warnings and undefined references at link time (`undefined reference to
'mid'`). Same gap as Python (Quirk 49). The C target's usable string
surface is `.Len()` and `&` concatenation. The hex-grid project is
deliberately built from loops + concat only so it transpiles cleanly to
all three targets.

