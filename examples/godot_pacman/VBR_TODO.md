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
