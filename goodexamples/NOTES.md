# Notes from the sketch five

Written as ordinary Bust, then compiled. Sketch Draw *does* run helpers (unlike View).

## Worked as hoped

- `Every 1 Tick` — iced asks for a millisecond timer. Hail counts ticks per wall-clock second with `DateTime.Now().Format("%S")` in the Event, so the window itself reports FPS.
- `Dim stones As Vec<Stone> = Storm.Seed()` in State, `Storm.Fall(stones)` on Tick — sibling type + ByRef, same path as fireflies.
- Draw `Floret.At(i)` / `Reel.At(t)` / `Sky.Planet(t, i)` / `Lace.Escape(zr, zi)` — helpers from Draw, including once per pixel on frost.
- `Match i` with `Return Place(...)` in every arm and `RaiseError` on `_` — Planet has no dummy value after the Match.
- Still `Set Pixel` Julia, then `Text "frost"` on top (pixels flush before Fill/Stroke/Text).
- Golden-angle bloom (`Sqr`, `Cos`/`Sin` on `i * π * (3 - Sqr(5))`).
- Hypotrochoid ribbon: Draw walks 480 points behind `t`; Tick only advances `t`.
- Orrery moon from `Sky.Moon(t, earth)` after a second `Planet(t, 2)` for Earth.

## Had to spell it the mill way

- `Int(...)` in a `Type` literal stays a float (`Peg { x: Int(...) }` → `.floor()` with no `as i64`). `Dim x As Long = Int(...)` then `Peg { x: x }` is what mill already did.
- Assigning to a `ByVal` parameter (`zr = tmp` inside Julia) — rustc wants `mut`. A `Dim zr As Double = zr0` copy is the local.

## Still a View projection

No View on a Sketch. Draw is the picture and it does go through the resolver.

## Language, not a compiler miss

- No `Rnd()` — hail velocities are clockwork from the index; bloom / coil / orrery are `Cos` / `Sin` on a phase.
- `Log` is still the logging verb.
