# VBR

A transpiler that turns VB-flavoured source into idiomatic Rust, compiles it, and
runs it. It's a teaching tool: the syntax is familiar VB, the semantics are Rust's,
and the generated Rust is always there to read.

- **`language_reference.md`** — the readable guide (start here).
- **`language_spec.md`** — the terse, normative reference.
- **`gui_spec.md`** — graphical apps: a `Window` → an Iced application.
- **`tui_spec.md`** — terminal apps: a `Screen` → a ratatui application.
- **`stdlib_spec.md`** — the standard library.

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

The standard library — `FileSystem`, `Regex`, `Http`, `DateTime`, `Json` — needs
no setup. Just reference a namespace and run the project:

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
