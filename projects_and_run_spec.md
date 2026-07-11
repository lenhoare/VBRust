# VBR Projects & Run Modes — Spec

How VBR programs are organised into projects, and how they're built and run.
(Companion to `inline_rust_spec.md` and `stdlib_spec.md`. Agreed, not yet built.)

---

## Project model

A **VBR project is a folder of `.vbr` files.**

- `main.vbr` (the file with `Function Main()`) is the **entry point**.
- Every other `.vbr` file is a **module**, named by the file:
  `utils.vbr` → module `utils`, `MyHelpers.vbr` → module `myhelpers`
  (filename lowercased, like every other name).
- **Public** items (`Public Function`, `Public Type`, `Public Const`) are
  visible across modules. Bare/`Private` items stay file-local.
- A lone `hello.vbr` is just the **degenerate one-file project** — the same model
  with no extra modules. (So `run` and `runproject` share one generation core.)

### Cross-file calls are QUALIFIED

```vb
Utils.DoThing()      →   utils::do_thing()
MyHelpers.Format(x)  →   myhelpers::format(x)
```

This reuses the exact `.`→`::` translation built for the stdlib — the transpiler
just needs the set of project module names (the other `.vbr` files), the same way
it knows the stdlib type names (`FileSystem`, `Json`, …).

A VB6 person isn't lost here: `Module.Function` qualification existed in VB6, and
qualified calls make it obvious where each function comes from.

### Cross-module *interfaces* (two-pass compile) — BUILT

A project compiles in two passes. **Pass 1** parses every `.vbr` module and
harvests its interface: `Public Function` signatures (parameter modes and
types, return type) and `Public Const` names. **Pass 2** compiles each file
with the sibling interfaces in scope, so a qualified call gets the **same
argument treatment as a local call** — nothing about the call site changes when
a function moves to another file:

```vb
Life.SetCell(grid, x, y, 1)   →   crate::life::setcell(&mut grid, x, y, 1)   ' ByRef Vec
Life.CountLive(grid)          →   crate::life::countlive(&grid)              ' ByVal Vec
Life.FormatRule(birth, s)     →   crate::life::formatrule(&birth, &s)        ' ByVal String
Life.WIDTH                    →   crate::life::WIDTH                         ' Public Const
```

Return types cross too — `Dim g As Vec<Long> = Life.NewGrid()` infers like a
local call. Visibility is enforced with teaching errors: calling a `Private`
function or reading a private `Const` from another file says to mark it
`Public`; an unknown member points out that a function call needs parentheses.

One deliberate limit:

- **A verbatim `.rs` module has no VBR interface** — calls into it stay
  name-qualified only, and its argument types are matched by hand (as before).

Example: `examples/life_project/` (a miniature Game of Life split into
`main.vbr` + `life.vbr`); guarded by `crossmodule_interfaces_compile`.

### Types are PROJECT-GLOBAL (bare names) — BUILT

Functions are qualified; types are not. A `Public Type` or `Public Enum` in
any module is usable **by its bare name in every file** — exactly VB6's
semantics (a Public UDT/Enum in any `.bas` was global; `Module.Type` syntax
never existed), and *also* exactly idiomatic Rust, which imports types and
qualifies functions:

```vb
' life.vbr                                 ' main.vbr — no prefix on a type
Public Type Rule                           Dim r As Rule = Life.ClassicRule()
    Public Birth As String                 Debug.Print r.Describe()
    Public Survive As String               Match Life.StateOf(v)
End Type                                       CellState.Alive => …
```
```rust
// generated main.rs
mod life;

use crate::life::CellState;   // ← the translation of "Public Type is global"
use crate::life::Rule;
```

The interface harvest (pass 1) carries each Public `Type`'s fields, each
Public `Enum`'s name, and the public methods (with their `&self`/`&mut self`),
so a foreign type infers, borrows, and pattern-matches exactly like a local
one. The generated file gets one `use crate::module::Name;` per foreign type
it actually mentions. This works on every surface — a `Screen`'s State can
hold a sibling's type (`examples/life_screen/`).

The rules, each a teaching error when broken:

- **Local wins** — a file's own `Type X` shadows a sibling's Public `X` (no
  `use` is emitted).
- **Two siblings exporting the same name** is ambiguous the moment a third
  file uses it: *"'Rule' is Public in more than one file — rename one."*
- **A sibling's Private type** stays home: *"'Hidden' is Private to
  'life.vbr' — declare it Public Type to use it from another file."*
- **Qualifying a type** (`Life.Rule`) is redirected: *"types are shared across
  the whole project by their bare name — write `Rule`."*

Two practical notes: fields another file touches must be `Public` (they become
`pub` on the Rust side; a private field trips rustc's own error, translated
back). And a public struct's methods cross only when declared
`Public Function Type.Method` — like any function.

Guarded by `crossmodule_interfaces_compile` (happy path, in the snapshots) and
`crossmodule_type_diagnostics` (the errors).

**Surfaces join projects too**: a `Screen`/`Window`/`Page` entry emits the
`mod` declarations and resolves qualified calls from its State initialisers,
events, and helper functions — so the natural shape "UI in `main.vbr`, logic
in `life.vbr`" works (`examples/life_screen/`). A fallible cross-module
initialiser (`Dim db As Database = Store.OpenDb()`) gets the clean-bail
`init()` exactly like a local one.

---

## Mixed `.vbr` + `.rs` projects (and stateful libraries)

A project is a folder of files that become Rust modules — so they needn't all be
`.vbr`. **A `.rs` file alongside them is included verbatim as a module**, called
from VBR exactly like any other module (the qualified-call machinery doesn't care
it's hand-written Rust — `.rs` files just skip the transpile step).

```
myapp/
├── main.vbr
├── utils.vbr
└── http.rs        ← hand-written Rust, a module like any other
```

This is the answer to *"I don't want a wrapper crate to exist for every library
I use."* A "wrapper" doesn't have to be a published crate — it can be a five-line
`.rs` file **in your own project**, where you keep the gnarly/stateful bits:

```rust
// http.rs — your own little helper, NOT a published crate
pub struct Session { client: reqwest::blocking::Client }
pub fn connect() -> Session { Session { client: reqwest::blocking::Client::new() } }
pub fn get(s: &Session, url: &str) -> Result<String, String> {
    s.client.get(url).send().and_then(|r| r.text()).map_err(|e| e.to_string())
}
```
```vb
Dim s = Http.Connect()
Dim body As String = Http.Get(s, "https://example.com").Unwrap()
```

The persistent `Session` (connection pool, cookies, auth) lives in `s` and is
reused across calls — stateful library use with **no wrapper crate and no global**.

It's also the purest graduation ramp: a project gradually accumulates `.rs` files
as the user gets comfortable, until one day it's just a Rust project.

### The spectrum for stateful / unwrapped libraries

None of these require a published wrapper:

- **Quick / throwaway** → an inline `Rust … End Rust` block (stateless), or an
  **opaque handle** threaded across blocks (stateful, with VBR driving the loop):
  ```vb
  Dim client = Rust  reqwest::blocking::Client::new()  End Rust   ' opaque, held by VBR
  For Each url In urls
      Dim body As String = Rust
          client.get(url).send().unwrap().text().unwrap()         ' reuses the same client
      End Rust
      Debug.Print body
  Next
  ```
  `client` is declared once, lives for the whole function, and every iteration
  reuses it — VBR owns the control flow, the Rust object just gets handed to each
  block. (An opaque handle is a value VBR holds but can't interpret — it can only
  carry it and pass it back into Rust blocks.)
- **Reusable in this project** → a **`.rs` module file** you write yourself (above).
- **Worth sharing with all VBers** → *then* it graduates into the curated stdlib.
  But that's an optimisation for popular libraries — never a prerequisite.

The stdlib wrappers (Json, DateTime, future Http) are just the *pre-polished*
version of the `.rs`-helper idea, done once for the common cases so most people
never have to. The door stays wide open for everything else.

---

## Commands

### `vbr run <file.vbr>` — quick single-file run

- Transpile → `rustc` → execute. Fast, no cargo overhead.
- Takes **any filename** (not just `main.vbr`); that file's `Main` becomes
  `fn main`.
- For simple, dependency-free scripts — where most early learning happens.
- **ERRORS** if the program uses any stdlib type *or* has any `Use` statement:
  > ✘ This program uses the standard library (or an external crate), which needs
  > the project build. Run it with `vbr runproject` instead.

  (Deliberate: `rustc` alone can't link crates, and surfacing the moment you need
  the project build is a small teaching beat — not silent magic.)

### `vbr runproject [dir]` — full project build & run

- Operates on a project folder (default: current directory).
- Generates a **visible** `build/` cargo project and `cargo run`s it (with
  `--quiet`, so only the program's own output shows).
- Handles the stdlib, external crates (`Use`), and multifile modules.

### (later) `vbr build [dir]` / `vbr emit <file>`

- `build` — generate `build/` without running.
- `emit` / `-o` — just write the `.rs` (no run).

---

## Generated layout (runproject)

```
myapp/                       ← you edit these
├── main.vbr
├── utils.vbr
└── build/                   ← GENERATED, visible, explorable
    ├── Cargo.toml
    └── src/
        ├── main.rs          ← fn main + `mod utils;`
        └── utils.rs
```

You edit `.vbr` files; `build/` is regenerated each run. It's visible on purpose —
ignore it while comfortable, peek when curious, run `cargo run` yourself when
ready, keep it and graduate to Rust one day. Honest, not hidden.

**Data files ride along.** The program runs with `build/` as its working
directory, so a folder project's build copies its data files there on every
build — the project folder is the source of truth. Copied: top-level files
that aren't sources (`.vbr`/`.rs`) or docs (`.md`), and whole subdirectories
(so `data/rules.txt` opens as `"data/rules.txt"`). Skipped: dotfiles and
`build/` itself. Edit `config.json` next to `main.vbr` and the next run sees
it; files the *program* writes (a SQLite database, say) live in `build/`.

---

## Cargo.toml generation

- `[package]`: name from the folder, `edition = "2021"`.
- `[dependencies]`:
  - `vbr_stdlib` (path dep) when any stdlib type is used. Path resolved via a
    compile-time default, overridable by `VBR_STDLIB_PATH`. (Long-term: publish
    `vbr_stdlib` to crates.io → `vbr_stdlib = "0.1"`, no path.)
  - each `Use rand 0.8` → `rand = "0.8"`.

---

## Philosophy

Same thread as inline Rust and the visible project: **seamless to run, but honest
and explorable.** The magic is in the convenience (`runproject` does everything),
never in concealment. For a teaching transpiler, the visible-but-effortless
project beats a hidden cache — hiding Cargo forever would undercut the very thing
VBR is for (the transition to Rust).

---

## Open / later

- `run` currently writes `<file>.rs` next to the source — maybe run from a temp
  dir to avoid littering.
- `build/` is generated — should be treated as disposable (gitignore-style).
- Cross-module **types** (`Public Type` / `Enum` used from another file) — the
  interface harvest covers functions and consts; types are the next slice.
- A surface *view* expression can't read a sibling module's constant (views
  don't run the resolver) — mirror it into state or read it through a helper
  (`projects/vbr_gaps.md` #26). Events, State initialisers, and helpers all
  resolve cross-module fine.
- Depends on inline Rust (`inline_rust_spec.md`) for `Use`'d crate calls to be
  worth anything.
