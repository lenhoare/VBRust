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

## Gpu Draw

`Gpu Draw` is the same nested `For y` / `For x` / `Set Pixel` picture as frost, compiled to one fragment shader. `Gpu Function` helpers go into WGSL, not Rust — including a `Public Gpu Function` in another file (`goodexamples/frost` pulls `lace.vbr`). CPU `Draw` (`Text` / `Fill` / `Stroke`) stacks on top. State numbers become uniforms (`t`, …). `mouse_x` / `mouse_y` are uniforms too (sketch pixels; last position if the pointer leaves). `Noise(x, y)` / `Noise(x, y, t)` is 0-to-1 value noise. `Sample(spr, u, v)` (or `frame`) reads a `Pixels` in the kernel. `goodexamples/plasma` is the first kernel; frost's Julia is the same shape. Hail's 320 stones are reconstructed in the kernel from a tick uniform (no `Vec` on the GPU yet); the FPS line stays CPU `Text`.

`Copy` / `Clear` / `Pixels` / last `frame` sit beside that kernel as extra GPU passes (not a CPU overlay):

- `Clear Color.Navy` fills the paper with no pixel loop.
- `Copy spr, x, y` / `Copy spr, dx, dy, dw, dh` / `Copy spr, dx, dy, sx, sy, w, h` — whole buffer, dest size (GPU filter, not a stretch loop), or a source rect.
- `Copy …, ColorKey, Color.Magenta` skips that colour; `Blend Add` / `Blend Multiply` (default overwrite).
- `Copy frame, 3, 1` is last paper, shifted. A kernel after `Copy` only writes the pixels it `Set`s — the rest stay see-through so the smear shows. Kernel-only sketches (frost) still start opaque black.
- `Dim spr As Pixels = Pixels.Of(18, 18)` is a GPU texture in State (white rectangle until you draw into it), not a `Vec`.
- `Into spr` … `End Into` paints that `Pixels` (`Clear` / kernel / `Copy`). `width` / `height` inside are the buffer's size, not the window's.
- `Copy spr, x, y, Using mask` samples a second `Pixels` (white keeps, black skips — coverage is the mask's RGB, so opaque black punches a hole).

`goodexamples/trails` is Copy + `frame` + a stamp. `goodexamples/badge` paints a `Pixels`, punches it with `Using hole`, and smears. `goodexamples/pond` is `Noise` + `Sample(frame, …)` + `mouse_x` / `mouse_y`. `goodexamples/aurora` is Noise curtains (a same-file `Gpu Function`) with the mouse shifting the ribbons. `goodexamples/ember` is `Copy frame` rising plus a heat kernel at the pointer.

## Forms Window

`goodexamples/desk` is the GUI surface with the everyday widgets (`TextInput`, `TextArea`, `Toggler`, `Slider`, `ProgressBar`, `Checkbox`, `Radio`, `Button`, `Match` / `If` in View) under `Theme JellyFish`. Theme is chosen when the window opens — it isn't a live picker.

`goodexamples/folio` is the next layer on the same palette: `Tabs` / `Frame` / `List` / `Table` (the Screen names, on a Window), `Chooser` (a `Vec` of options, not an enum), `Scrollable` / `Rule`, `TextInput` `Secure` + `On Submit`, `Button` `Enabled`, `Slider` `Step`, `Markdown` (a `String` field), and `Svg`. A `String` field assigned from an event parameter is moved — `fruit = value` then `status = value` will not compile; set a literal (or `status = fruit`) for the second write.
