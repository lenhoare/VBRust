# VBR

A transpiler that turns VB-flavoured source into idiomatic Rust, compiles it, and
runs it. It's a teaching tool: the syntax is familiar VB, the semantics are Rust's,
and the generated Rust is always there to read.

- **`vb6_to_vbr_guide.md`** — coming from VB6? Start here: the short list of what's different.
- **`language_reference.md`** — the readable guide (the full story).
- **`gui_and_tui_guide.md`** — a friendly tour of building `Window` (GUI) and `Screen` (TUI) interfaces.
- **`language_spec.md`** — the terse, normative reference.
- **`gui_spec.md`** — graphical apps: a `Window` → an Iced application.
- **`tui_spec.md`** — terminal apps: a `Screen` → a ratatui application.
- **`web_spec.md`** — browser apps: a `Page` → a Yew (WebAssembly) application.
- **`stdlib_spec.md`** — the standard library.
- **`dataframe_spec.md`** — native dataframes: a `DataFrame` → the polars crate.
- **`targets_spec.md`** — the alternative transpile targets: `vbr py` (Python) and `vbr c` (C).
- **`godot_spec.md`** — an optional extra: a `Node2D`/`Node3D` (…) → a Godot game, via godot-rust.

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

## Games (Godot) — an optional extra

An **optional** bolt-on, not a core VBR target: a `Node2D`/`Node3D` (…) block is a
**Godot** game object that compiles to a GDExtension (a cdylib, via
[godot-rust](https://godot-rust.github.io/)) the Godot editor loads. It's for
making 2D (and 3D) games, built on the same machinery as the other surfaces — so
it reads like the rest of VBR, but you can ignore it entirely.

```sh
cargo run -- rungodot examples/godot_player.vbr    # a single node
cargo run -- rungodot examples/godot_game          # a multi-file project
```

`rungodot` assembles a Godot 4 project beside the source, builds the crate, and
opens it in Godot — press Play ▶. It covers the full 2D game loop (movement,
signals, scene tree, spawning, input) and asset-aware project folders. Needs
**Godot 4** installed (`snap install godot4`, or set `GODOT4_BIN`). See
**`godot_spec.md`** for the whole surface.

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

## Editing in VS Code

`editors/vscode/` is a VS Code extension that turns `.vbr` files into a real
editing experience:

- **Syntax colours** — a TextMate grammar (static; works with nothing else set
  up). Inline `Rust … End Rust` / `Python … End Python` blocks even get their own
  language's highlighting.
- **Autocomplete, hover, go-to-definition, live error squiggles** — provided by
  the `vbr-lsp` language server, which runs the real compiler front-end as you
  type (so the teaching diagnostics show up in the editor).

**Setup:**

```sh
# 1. Build the language server (once per machine — needs Rust). On Windows this
#    produces vbr-lsp\target\release\vbr-lsp.exe; on Linux, .../vbr-lsp.
cd vbr-lsp && cargo build --release

# 2. Install the packaged extension. The `code` CLI puts it in the right place.
code --install-extension editors/vscode/vbr-lsp-0.0.1.vsix

# 3. Reload VS Code (Ctrl+Shift+P → "Developer: Reload Window").
```

Open the **VBRust folder** in VS Code and then any `.vbr` file. Colours are on
immediately; the smart features light up because the extension auto-finds the
server at `vbr-lsp/target/release/vbr-lsp(.exe)` inside the open workspace — so
the same checkout works on Linux and Windows with no per-machine path. (To point
elsewhere, set `vbr.serverPath`.)

**See the Rust it becomes, side by side.** With a `.vbr` open, run
*"VBR: Open Rust Output to the Side"* (the split icon in the editor title bar, or
the Command Palette). A read-only pane opens in the second column showing the
generated Rust; it refreshes every time you save. If the program doesn't compile,
the pane shows the transpiler's teaching diagnostics instead. (For a snappy
refresh, `cargo build --release` once so the extension can use the prebuilt `vbr`
binary rather than `cargo run`.)

**Run it.** With a `.vbr` open, click the ▶ button in the editor title bar (or
press **Ctrl+Alt+R**, or *"VBR: Run File"* in the Command Palette) — it runs the
file in a terminal, no `launch.json` needed. The ▶ also works on a `.rs` file
that embeds VBR (a `/* vbr … */` block): it expands the block with `vbr embed`,
reloads the file, and — if it's a standalone file with a `fn main` — compiles it
with `rustc` and runs it. Inside a Cargo project it stops after expanding and
leaves running to cargo's own build/run (Ctrl+Shift+B / the ▶ over `fn main`).

A `.vscode/tasks.json` also adds build tasks (in a VBRust checkout):
**Ctrl+Shift+B** runs the current `.vbr` file; Terminal → Run Task offers
*Run project*, *Run in Godot*, and *Test*. Note that in an ordinary Rust project
Ctrl+Shift+B keeps its usual meaning (cargo build the crate) — the VBR run verb
lives on its own **Ctrl+Alt+R** / ▶ so the two never collide.

**Debugging.** VBR is a transpiler, so debugging means debugging the Rust it
produces — and the live view above already shows you that Rust. If you ever want
to step through it, `vbr debugbuild <file.vbr>` compiles a symbol-carrying binary
(plus the generated `.rs`) into `.vbrdebug/` (git-ignored) and prints its path;
debug that binary with your normal Rust debugger. (VS Code integration for
stepping was tried and pulled back — juggling CodeLLDB, rust-analyzer, and a
synthetic `.rs` proved more friction than value; the live view is where the
teaching payoff is.)

If you change the extension or the grammar, repackage with
`cd editors/vscode && npx @vscode/vsce package` and reinstall.

## Embedding VBR inside Rust

The mirror of inline Rust (`Rust … End Rust` inside VBR): you can drop a snippet
of VBR into a Rust file. Write it in a `/* vbr … */` block comment, then run

```sh
cargo run -- embed path/to/file.rs
```

and the transpiler fills a managed `// vbr:gen … // vbr:gen-end` region right
after it with the Rust your VBR became, indented to match. Re-running overwrites
that region (idempotent), and because the VBR stays a comment the `.rs` compiles
either way.

The expansion is spliced into the *same* Rust function, so there's no runtime
boundary — the embedded VBR can call Rust functions, read Rust variables in
scope, and leave its own variables for the surrounding Rust to use:

```rust
fn sum_of_squares(limit: i64) -> i64 {
    /* vbr
        Dim total As Long = 0
        Dim i As Long
        For i = 1 To limit
            total = total + square(i)   ' square() and limit are Rust
        Next
    */
    // vbr:gen (generated by `vbr embed` — do not edit)
    let mut total: i64 = 0;
    for i in 1..=limit {
        total = total + square(i);
    }
    // vbr:gen-end
    total
}
```

VBR is permissive at the seam — a name it doesn't recognise (`square`, `limit`)
is assumed to be Rust in scope and passed straight through, so **Rust's own type
checker is the source of truth at the boundary** (a mismatch surfaces as a normal
`rustc` error). The VS Code extension colours the VBR inside the block via a
grammar injection. Block comments end at the first `*/`, so embedded VBR can't
contain a literal `*/` (only realistic inside a string — split it). See
`examples/rust_embedding/`.

`vbr embed --check <file.rs>` verifies without writing — it exits non-zero if the
generated region is out of date (VBR edited but not re-expanded) or the VBR has
errors, so a pre-commit hook or CI can guarantee the committed Rust matches its
source.

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
