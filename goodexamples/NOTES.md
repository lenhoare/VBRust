# Notes from the second five

Written as ordinary Bust, then compiled. These are the places the source looked right and the compiler disagreed.

## Worked as hoped

- `Dim bay As Hold = Cargo.Load(path) Handle` on a named type.
- `bay.Save(path)` with `path As String` (ByVal borrows).
- `Cargo.CAPACITY` from another module.
- Screen Events calling `Pit.ListLot` / `Oven.Beat` / `Oven.OpenDoor` with Handle.
- Sketch **Draw** calling `Sea.Height(t, x)` and `Sea.Foam(t, x)` — Draw is a statement list with a Result catch, so module calls are real Bust. View is not.
- `x * 0.16` with `x As Long` widens in a Function (no `As Double` cast).
- `On Key " "` for space (same as the countdown project).

## Sensible Bust that did not behave

1. **`Table lots` with `Lot` declared in `pit.vbr`.**  
   `lots As Vec<Lot>` is a Vec of a Public Type, but the Screen checker only looks at structs in the *same file*. Error: `lots isn't a Vec<Struct>`.  
   Moving `Type Lot` into `main.vbr` made Table parse, then `pit.vbr` failed to compile (`Lot` not in scope — no `use crate::Lot`).  
   **Workaround:** `List` of string rows, labels rebuilt in Events (`Pit.RowLabels`), same shape as workshop.

2. **`For Each c In Me.Items` then `sums.Insert(c.Dest, …)`.**  
   `c.Dest` is behind a shared ref; HashMap keys must be owned. Auto-clone covers `vec[i]`, not a For-Each field.  
   `Dim dest As String = c.Dest` still moved.  
   **Workaround:** `Dim dest As String = c.Dest.Clone()`.

3. **`For Each k In kept` then `lots.Push(k)`.**  
   Same hole: the loop variable is a borrow of a struct.  
   **Workaround:** `lots.Push(kept[j])` — indexing clones.

4. **View helper calls.**  
   Almost wrote `Text "door " & DoorWord(door)`. That is a user function in View; View does not go through the resolver and is not `Result`.  
   **Workaround:** a `doorLabel As String` field updated in Events.

## Warnings, not failures

- A `Do … Loop` in `Main` still gets a trailing `Ok(())` rustc marks unreachable.
- `RaiseError` as the last line of a Function still gets the same trailing `Ok(())`.

## Language, not a compiler miss

- No `Rnd()` — tide is two sine waves on a clock.
- `Log` is still the logging verb / `Ln`; modules stay clear of that name.
