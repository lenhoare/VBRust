# Bust Godot Specification

A **`Node2D` (…) block** is a **Godot game object** — one node class the Godot
engine instantiates and drives. It compiles to a **GDExtension**: a Rust `cdylib`
(via [godot-rust / gdext](https://godot-rust.github.io/)) that the Godot editor
loads. You write Bust; Godot owns the scene and the game loop.

> **Godot is an optional extra, not a core Bust target.** Bust's heart is
> VB-flavoured source → idiomatic Rust, with the GUI/TUI/Web surfaces and the
> Python/C targets. Godot is a bolt-on for people who want to make 2D games — the
> same way the standard-library namespaces bolt onto the language. It's built on
> the same machinery (the resolver runs on node bodies just like the other
> surfaces), so it feels like the rest of Bust, but you can ignore it entirely.

> Status: **slices 1–9 BUILT + verified live on Godot 4.7.1** (2026-08-05) — nodes
> and lifecycle events, the general property/method passthrough, signals (declare,
> emit, connect + handlers), scene-tree access (`GetNode`), spawning (`Spawn` +
> `AddChild`), input events (`On Input`), multi-file **project folders** with asset
> copying, and **3D** (`Node3D`, `CharacterBody3D`, `Camera3D`, `Vector3` …).

---

## 1. Design goals

- **It reads like the rest of Bust.** A node is a familiar-looking block with
  `Export` fields and `On <Event>` bodies; inside a body you write ordinary Bust.
  The one Godot-ish idea is *inversion of control* — Godot calls your events; you
  don't write a `Main`.
- **Passthrough over wrapping.** Godot's API is enormous. Bust doesn't wrap it —
  it forwards calls straight to gdext (`Me.MoveAndSlide()` → `move_and_slide()`),
  so the whole API is reachable without per-method work. A wrong name is caught by
  the Rust compiler and translated back to your `.vbr` line.
- **The engine owns the scene.** You build scenes, lay out nodes, and import
  assets in the Godot editor; Bust contributes the *behaviour scripts*. A `.tscn`
  you make in Godot is yours; `rungodot` never overwrites it.

## 2. A node

```vb
Node2D "Player"
    Export Speed As Single = 200

    On Ready
        Debug.Print "Player ready"
    End Ready

    On Process(delta)
        Dim velocity As Vector2 = Vector2.Zero
        If Input.IsPressed("ui_right") Then velocity.X = velocity.X + 1.0
        Me.Position = Me.Position + velocity * Speed * delta
    End Process
End Node2D
```

- **`<BaseClass> "Name"`** opens a node. The leading keyword is the Godot **base
  class** — `Node2D`, `Node`, `CharacterBody2D`, `Area2D`, `Sprite2D`. The string
  is the registered class / struct name. Close with **`End <BaseClass>`** (like
  `End Screen` / `End Window` — the opening keyword, not the instance name).
- **`Export <Name> As <Type> [= <default>]`** — a member field visible in Godot's
  inspector. A plain **`Dim <Name> As <Type> = <default>`** is a private member.
- **`Me`** is the node itself (it lowers to Rust's `self`). A **bare name** in a
  body is a member field (`Speed` → this node's `speed`).
- Lowers to a gdext `#[derive(GodotClass)] #[class(base = …)]` struct plus a
  `#[godot_api] impl I<Base>` with the lifecycle methods.

### Lifecycle events

| Bust | Runs |
|-----|------|
| `On Ready` | once, when the node enters the tree |
| `On Process(delta)` | every frame (`delta` = seconds since last, a `Single`) |
| `On PhysicsProcess(delta)` | every physics tick |
| `On Input(event)` | on each input event (see §6) |
| `On EnterTree` / `On ExitTree` / `On Draw` | the matching Godot callbacks |

## 3. The passthrough

Inside a body, Godot's API is reached by forwarding:

- **Properties** — `Me.<Prop>` reads/writes a base-class property:
  `Me.Position` → `get_position()`, `Me.Velocity = v` → `set_velocity(v)`
  (any property; the write is hoisted past the borrow for you).
- **Methods** — `Me.<Method>(…)` calls a base-class method:
  `Me.MoveAndSlide()` → `move_and_slide()`, `Me.IsOnFloor()` → `is_on_floor()`.
  Names are snake-cased automatically (`GetGlobalPosition` → `get_global_position`).
- **`Input`** — the input singleton: `Input.IsPressed("ui_right")`,
  `Input.GetAxis("ui_left", "ui_right")`.
- **Value types** — `Vector2`, `Vector2i`, `Vector3`, `Color`, `Rect2`:
  `Vector2(x, y)` constructs, `Vector2.Zero` / `Color.Red` are named constants.

`examples/godot_runner.vbr` is a `CharacterBody2D` platformer showing gravity,
jumping, and `MoveAndSlide` through this passthrough.

## 4. Signals

A node's **outgoing** events. Declared and fired:

```vb
Node2D "Emitter"
    Signal Pinged(count As Long)

    On Process(delta)
        ' … when something happens …
        Emit Pinged(count)
    End Process
End Node2D
```

- **`Signal <Name>[(<params>)]`** declares a signal (optional typed payload).
- **`Emit <Name>[(args)]`** fires it.

Another node **connects** a handler to it — explicitly, no name-matching magic:

```vb
Node2D "Listener"
    On Ready
        Dim emitter As Emitter = Me.GetNode("Emitter")
        Connect emitter.Pinged To OnPinged
    End Ready

    Sub OnPinged(count As Long)
        Debug.Print "heard ping " & count
    End Sub
End Node2D
```

- **`Sub <Name>(<params>) … End Sub`** — a handler method (a callable `#[func]`).
- **`Connect <source>.<Signal> To <Handler>`** — wires the signal to it.
  `<source>` is `Me` (own signal) or a `GetNode` handle.

## 5. Scene tree & spawning

- **`Dim h As T = Me.GetNode("Path")`** — reach another node in the scene as a
  typed handle. Call methods on it: `h.SetText("Score: " & n)`.
- **`Dim h As T = Spawn("res://scene.tscn")`** — load a scene and instantiate a
  fresh node. Set it up, then **`Me.AddChild(h)`** to put it in the tree.

The type after `As` is the node's Godot class (`Label`, `Node2D`, or one of your
own node classes). `examples/godot_scene.vbr` and `examples/godot_spawn.vbr` show
each.

## 6. Input events

```vb
Node2D "Controller"
    On Input(event)
        If event.IsActionPressed("ui_accept") Then
            Debug.Print "accept pressed"
        End If
    End Input
End Node2D
```

`On Input(event)` runs on every input event; `event` is a Godot `InputEvent` you
query — the cleanest check is by **action** (mapped in Godot's Input Map), so it
works for keyboard and gamepad alike. Use `On Input` for discrete, one-shot events
(a jump, a menu toggle); poll `Input.IsPressed` in `On Process` for continuous
movement.

## 7. Projects (folders, modules, assets)

Because games are asset-heavy and multi-file, `rungodot` takes a **project folder**
(like `runproject`):

```
mygame/
  main.vbr      the entry — holds the main scene's root node
  enemy.vbr     more node classes
  combat.vbr    shared logic (no nodes) — Public functions, Types, Consts
  main.tscn     your scenes and assets (textures, audio, …)
```

- The entry **`main.vbr`** must contain the main scene's root node. Sibling `.vbr`
  files are modules; nodes can live in any of them (gdext registers them all).
- Modules call each other by **qualified name** — `Combat.Damage(hp, hit)`.
- **Assets** (`.tscn`, `.png`, `.ogg`, an `assets/` folder …) are copied into the
  generated Godot project so `res://…` paths resolve. Your own `main.tscn` is kept
  if you supply one; otherwise a starter scene is generated.

`examples/godot_game/` is a worked multi-file project.

## 8. Building & running

```sh
vbr rungodot examples/godot_player.vbr    # a single file
vbr rungodot examples/godot_game          # a project folder
```

`rungodot` assembles a self-contained Godot 4 project **beside the source**
(`<name>_godot/`): `project.godot`, a `.gdextension`, the scene, and a `rust/`
crate (the generated cdylib). It builds the crate, then opens the project in Godot
— press **Play ▶**. The folder is stable: keep editing scenes in the Godot editor
while `rungodot` regenerates the Rust. (The generated `*_godot/` folder is
git-ignored.)

**Requirements:** **Godot 4** (from [godotengine.org](https://godotengine.org) or
`snap install godot4`; set `GODOT4_BIN` if it isn't on your `PATH`). *Building* the
crate needs nothing extra — gdext bundles the Godot API, so the cdylib compiles
even without Godot installed; you only need Godot to run.

## 9. 3D

3D is the same surface as 2D — a different base class and 3-component vectors:

```vb
Node3D "Spinner"
    Export Speed As Single = 1.0

    On Process(delta)
        Me.Rotation = Me.Rotation + Vector3(0, Speed * delta, 0)
    End Process
End Node3D
```

- **Base classes**: `Node3D`, `CharacterBody3D`, `Area3D`, `MeshInstance3D`,
  `Camera3D`, `RigidBody3D`, `StaticBody3D`.
- **Value types**: `Vector3` (`Vector3(x, y, z)`, `Vector3.Up`).
- The property/method passthrough is **dimension-agnostic**: `Me.Position` (now a
  `Vector3`), `Me.Rotation`, `Me.LookAt(…)`, a `CharacterBody3D`'s `Me.Velocity`
  and `Me.MoveAndSlide()` — all the same as their 2D cousins.
- `rungodot` generates a 3D starter scene (a box mesh, a camera, a light) so the
  node is visible. 3D scenes are asset-heavy, though, so for a real game build the
  scene in the Godot editor and drop a `main.tscn` beside the source.

`examples/godot_3d.vbr` is a spinning cube.

## 10. Scope

- Deferred niceties: cross-node *property* sugar (use `h.GetPosition()`, not
  `h.Position`, on a fetched node); a bare `String` *variable* passed to a Godot
  string method needs a manual `&` (a concatenation is handled); `.rs` and
  `.test.vbr` modules inside a Godot project; richer 3D value types (`Transform3D`,
  `Basis`, `Quaternion` construction — their *methods* already pass through).
- Signal connection is by code (`Connect … To`) or in the Godot editor.
