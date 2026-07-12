# VBR Language Support (VS Code)

Editor assistance for `.vbr` files, powered by the VBR compiler itself:

- **Live diagnostics** — as you type, the compiler runs and its errors,
  warnings, and teaching notes appear as squiggles, underlining the exact
  offending token (the same messages you get on the command line).
- **Hover** — point at a variable to see its VB type *and* the Rust type it
  lowers to (`total As Long · Rust: i64`), the same teaching pair the
  diagnostics speak in. Constants and opaque `Rust …` handles explain
  themselves too.
- **Go to definition** — F12 on a variable use jumps to its `Dim`.
- **Completion** — type `.` after a variable and get the members of *its
  type*: a `String` offers the real Rust methods (`trim`, `to_uppercase` — the
  same names the generated code uses), a `Vec` offers `Push` and the iterator
  adapters, a struct its fields and methods, an enum its variants, and a
  stdlib namespace (`Http.`, `FileSystem.`…) its functions with VB-facing
  signatures. In bare position: variables in scope, your functions, constants,
  types, namespaces, keywords.
- **Error recovery** — a half-typed line costs one error; everything below it
  is still analysed, so the rest of your diagnostics don't vanish while you
  type mid-statement. (This is also what makes completion work: the line
  you're typing on never parses, but the symbol table around it survives.)

Not yet: go-to-definition for functions and parameters.

## Architecture

- **`vbr-lsp`** (a Rust crate at the repo root) — the language server. It speaks
  the Language Server Protocol over stdio and reuses the `vbr` library as its
  compiler front-end. One `vbr::compile` gives it everything: structured
  diagnostics with byte spans, a typed symbol table (hover + the receiver
  types completion needs), and go-to-definition pairs. `vbr::complete::
  completions_at(source, offset)` answers completion from that plus curated
  member catalogues mirroring `vbr_stdlib`'s API.
- **this extension** — a thin VS Code client that launches the server and tells
  it which files are VBR. All the real work is in the server.

## Try it

1. Build the server (release):

   ```sh
   cargo build --release --manifest-path vbr-lsp/Cargo.toml
   ```

   That produces `vbr-lsp/target/release/vbr-lsp`, which the extension looks for
   by default. (Override with the `vbr.serverPath` setting or the
   `VBR_LSP_SERVER` environment variable.)

2. Install the client dependencies:

   ```sh
   cd editors/vscode && npm install
   ```

3. Open `editors/vscode/` in VS Code and press **F5** to launch an Extension
   Development Host. Open any `.vbr` file (e.g. from `examples/`) and:
   - introduce a mistake — `Debug.Print 1 + ]` — to see the squiggle sit
     exactly on the `]`;
   - hover a variable to see its VB and Rust types;
   - press F12 on a variable use to jump to its `Dim`;
   - type `.` after a variable and watch the member list match its type.
