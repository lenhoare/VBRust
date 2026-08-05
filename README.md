# VBR

A transpiler that turns VB-flavoured source into idiomatic Rust, compiles it, and
runs it. It's a teaching tool: the syntax is familiar VB, the semantics are Rust's,
and the generated Rust is always there to read.

- **`language_reference.md`** — the readable guide (start here).
- **`gui_and_tui_guide.md`** — a friendly tour of building `Window` (GUI) and `Screen` (TUI) interfaces.
- **`language_spec.md`** — the terse, normative reference.
- **`gui_spec.md`** — graphical apps: a `Window` → an Iced application.
- **`tui_spec.md`** — terminal apps: a `Screen` → a ratatui application.
- **`web_spec.md`** — browser apps: a `Page` → a Yew (WebAssembly) application.
- **`stdlib_spec.md`** — the standard library.
- **`dataframe_spec.md`** — native dataframes: a `DataFrame` → the polars crate.
- **`targets_spec.md`** — the alternative transpile targets: `vbr py` (Python) and `vbr c` (C).

## Building

VBR is a Rust project. Build the `vbr` binary with Cargo:

```sh
cargo build              # debug build → target/debug/vbr
cargo build --release    # optimised  → target/release/vbr
```

The examples below use `cargo run --` (which builds as needed); once built you can
call `target/debug/vbr` directly instead.

## Running a program

A single `.vbr` file is transpiled, compiled with `rustc`, and executed in one step:

```sh
cargo run -- run examples/hello.vbr
```

`run` is for self-contained, dependency-free programs. A program that uses the
standard library or an external crate needs the project build instead — `run`
will tell you so and point you at `runproject`.

## Transpiling (seeing the Rust)

To inspect the generated Rust without running it:

```sh
cargo run -- emit examples/hello.vbr           # print the Rust to stdout
cargo run -- transpile examples/hello.vbr      # write it to examples/hello.rs
cargo run -- transpile examples/hello.vbr -o out.rs
```

## Projects (multiple files, the stdlib, crates)

A folder of `.vbr` files is a project. The file containing `Function Main()`
(default `main.vbr`) is the entry point; other `.vbr` files become modules, and
any `.rs` file is included verbatim as a hand-written module.

```sh
cargo run -- runproject myapp     # generate a visible build/ Cargo project and run it
cargo run -- build myapp          # generate the project without running it
```

`runproject` writes an explorable Cargo project to `myapp/build/` and runs it with
`cargo run`. Projects that use `vbr_stdlib` link it by path; override its location
with the `VBR_STDLIB_PATH` environment variable if needed.

## Using the standard library

The standard library — `FileSystem`, `Regex`, `Http`, `DateTime`, `Json`,
`Database` (SQLite) — needs no setup. Just reference a namespace and run the
project:

```vb
' fetch.vbr
Function Main()
    Match Http.Get("https://example.com")
        Ok(body) => Debug.Print "got " & body.Len() & " bytes"
        Err(message) => Debug.Print "request failed: " & message
    End Match
End Function
```

```sh
cargo run -- runproject myapp
```

`runproject` detects which namespaces you used and pulls in the right
dependencies automatically — each one is behind a Cargo feature, enabled for you.
You never edit `Cargo.toml` or turn on a feature yourself; that's all internal.
(`Http` does simple, blocking, one-shot requests; for a reused client or session,
reach for an inline `Rust` block or a hand-written `.rs` module.)

## Web pages

A program with a `Page` compiles to a browser app (Yew, via WebAssembly) and is
served with:

```sh
cargo run -- runweb examples/web_counter.vbr
```

A `Screen` (terminal app) can be served the same way — the identical file runs
in the terminal via `run`/`runproject` and in the browser via `runweb`, where
Ratzilla draws the ratatui widgets into the DOM:

```sh
cargo run -- runweb examples/tui_counter.vbr
```

One-time setup (each checked with a friendly error): `rustup target add
wasm32-unknown-unknown` and `cargo install trunk --locked`. See `web_spec.md`.

## Games (Godot)

A program with a `Node2D` (…) block is a **Godot** game object — one node class
Godot instantiates and drives. It compiles to a **GDExtension** (a cdylib, via
[godot-rust](https://godot-rust.github.io/)) and is opened in the Godot editor:

```sh
cargo run -- rungodot examples/godot_player.vbr
```

That assembles a self-contained Godot 4 project beside the source
(`godot_player_godot/`: `project.godot`, a `.gdextension`, a starter scene, and
the `rust/` crate), builds it, and opens it in Godot — press Play ▶ to steer the
square with the arrow keys. `examples/godot_runner.vbr` is a fuller one — a
`CharacterBody2D` platformer (gravity, jump, run) showing how base-class
properties (`Me.Velocity`) and methods (`Me.MoveAndSlide()`) pass straight
through to Godot's API; `examples/godot_signal.vbr` shows a node's outgoing
events — `Signal Pinged(count As Long)` to declare one, `Emit Pinged(count)` to
fire it; `examples/godot_scene.vbr` reaches another node with `Me.GetNode("Path")`
(typed by the `Dim`'s `As`) and calls methods on it; `examples/godot_connect.vbr`
wires a signal to a handler — `Sub OnPinged(count As Long)` plus
`Connect emitter.Pinged To OnPinged`; `examples/godot_spawn.vbr` spawns nodes at
runtime — `Dim b As Node2D = Spawn("res://bullet.tscn")` then `Me.AddChild(b)`;
`examples/godot_input.vbr` handles discrete input with `On Input(event)` +
`event.IsActionPressed(…)`. Requires **Godot 4** (from
[godotengine.org](https://godotengine.org) or `snap install godot4`; set
`GODOT4_BIN` if it isn't on your PATH). Building the crate needs nothing extra —
gdext bundles the Godot API, so the cdylib compiles without Godot installed.

## Playground

The transpiler itself compiles to WebAssembly: `playground/` is a two-pane
browser app — type VBR on the left, read the generated Rust (and the teaching
diagnostics) on the right, with an example picker covering the language. No
server; the compiler runs in the page.

```sh
cd playground && trunk serve --open
```

`trunk build --release` produces a fully static `dist/` (three files, ~760 KB
of wasm) that can be hosted anywhere — GitHub Pages included.

## Other targets: Python and C

VBR is Rust-first — the language is defined around Rust, and everything above
describes that target. As an **additive bolt-on**, the same source can also be
transpiled to **Python** or **C**, to show one program in three languages:

```sh
cargo run -- py examples/hello.vbr        # → idiomatic Python (stdout, or -o out.py)
cargo run -- c  examples/hello.vbr        # → self-contained C (stdout, or -o out.c)
```

Both cover the **core language and the full standard library** — Python as a
`main.py` + `vbrpy/` project, C as a self-contained `.c` (or a project folder +
`Makefile` when a namespace needs a library). The GUI/TUI/Web surfaces stay
Rust-only. For deterministic programs the output is checked **byte-for-byte
against `vbr run`**. The C target adds `x = Nothing` — an explicit "release this
now" hook (`drop` on Rust, `None` on Python, `free` on C) that makes manual
memory visible. Full detail — packaging, external libraries (`Use` → pip, inline
`Python`), and the shared front-end that keeps the three in sync — is in
**`targets_spec.md`**.

```sh
cargo run -- c examples/hello.vbr -o hello.c && cc hello.c -lm && ./a.out
```

A C program that uses a *linked* standard-library namespace becomes a project
folder and needs that library's development package at build time: **`Database`**
needs `libsqlite3-dev` (`-lsqlite3`) and **`Http`** needs `libcurl4-openssl-dev`
(`-lcurl`). The vendored ones (`Json` → cJSON) need nothing but a C compiler.
Python's stdlib and the other C namespaces (FileSystem, DateTime, Shell, Regex)
have no external requirements.

## Running the tests

The test suite snapshots every example in `examples/` and, for the runnable ones,
compiles the generated Rust with `rustc` to prove it is valid and warning-free.

```sh
cargo test
```

After an *intended* change to code generation, regenerate the stored snapshots and
review the diff:

```sh
UPDATE_SNAPSHOTS=1 cargo test
```

The standard library is a separate crate with its own tests. Its
dependency-bearing modules are behind Cargo features, so run them with all
features enabled:

```sh
cargo test --manifest-path vbr_stdlib/Cargo.toml --all-features
```

Stdlib/GUI/TUI examples can't be compiled by the rustc-only snapshot check
(they link crates), so a separate **compile guard** builds one representative
example per backend as a real cargo project and requires it to be warning-free.
It compiles Iced/polars/ratatui, so it's skipped by default — run it before a
release or after touching codegen:

```sh
cargo test -- --ignored
```

Examples live in `examples/`; their expected output (generated Rust or
diagnostics) lives in `tests/snapshots/`.

## Try it

```vb
' hello.vbr
Function Main()
    Debug.Print "hello, world"
End Function
```

```sh
cargo run -- run hello.vbr
```
