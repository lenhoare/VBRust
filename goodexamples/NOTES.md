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
- `Int(...)` in a `Type` literal (`Peg { x: Int(...) }`) narrows like `Dim x As Long = Int(...)`.
- Assigning to a `ByVal` parameter is a local copy, so `zr = tmp` inside Julia is legal.

## Still a View projection

No View on a Sketch. Draw is the picture and it does go through the resolver.

## Language, not a compiler miss

- No `Rnd()` — hail velocities are clockwork from the index; bloom / coil / orrery are `Cos` / `Sin` on a phase.
- `Log` is still the logging verb.

## WSLg present path (tiny-skia vs wgpu)

Animated 640×480 sketches were dying with `Io error: Connection reset by peer` (weston). Static bloom at 640×480 was fine; hail at 800×480 with a timer was fine. Same orrery drawing:

- tiny-skia/softbuffer at 640×480 + `Every` — dies in ~1s
- tiny-skia at 800×480 — lives (800×600 dies; 641 and 700 still die — not “just make it bigger”)
- iced wgpu at 640×480 — lives

So it is the software present path at particular buffer sizes, not the planets. Generated GUIs now enable wgpu with tiny-skia as fallback. `Fill Circle` stamps also no longer flush/zero the pixel buffer between disks (coil was emitting one full-window image per dot).

## Gpu Draw (v1 kernel)

`Gpu Draw` is the same nested `For y` / `For x` / `Set Pixel` picture as frost, compiled to one fragment shader. Same-file `Gpu Function` helpers go into WGSL, not Rust. CPU `Draw` (`Text` / `Fill` / `Stroke`) stacks on top. State numbers become uniforms (`t`, …). Copy / masks / `Pixels` come later — and a `Gpu Function` in another file is not wired yet (interfaces don't carry bodies). `goodexamples/plasma` is the first kernel.
