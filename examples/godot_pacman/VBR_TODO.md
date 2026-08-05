# VBR Godot gaps found while building Pac-Man

Features we don't have yet (worked around for now; discuss after Pac-Man). Clear
*bugs* get fixed on the spot and aren't listed here.

- **Load a texture/resource from code** — no `load("res://x.png")` for a
  `Texture2D`. *Workaround:* set textures in the `.tscn` (`ext_resource`); VBR only
  drives behaviour. (Fits the "editor owns the scene" model, so maybe fine as-is.)
- **Cross-node property assignment** — `handle.Position = v` doesn't work (only
  `Me.Prop = v` does). *Workaround:* use the method form `handle.SetPosition(v)`.
- **Keep a user-provided `project.godot`** — `rungodot` always overwrites it, so a
  project can't set window size / stretch. *Workaround:* scale the root node in the
  scene. (Parallels the `main.tscn`-is-kept behaviour — probably should keep
  `project.godot` too.)

## Bugs fixed on the spot (M1b)
- **`Rect2(x,y,w,h)` generated `Rect2::new(...)`** — gdext's `Rect2::new` takes two
  `Vector2`s; fixed to `Rect2::from_components` (same for `Rect2i`). *(godot.rs)*
- **`Abs(aSingle)` was typed `Double`** — `builtin_vtype` hard-coded `abs`→Double,
  but `.abs()` keeps the arg's type, so `f32` in stayed `f32` out and the spurious
  `as f64` broke the compare. Now `Abs` infers from its argument. *(resolver.rs)*

## More feature-gaps (worked around)
- **Case-insensitive name collisions pass silently** — a `Row` function and a `row`
  param both became `row` in Rust (`row(row)` → error). VBR should *diagnose* this.
  *Workaround:* renamed the param.
- **`Vector2` fields are opaque to the resolver** — `p.X` on a `Vector2` infers as
  Unknown, so numeric math through it loses types. Same root as "richer value
  types" (the resolver should know `Vector2.x/.y : Single`). *Workaround:* keep
  position as scalar `Single` fields, build a `Vector2` only to place the sprite.
