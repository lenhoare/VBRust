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
- **Error recovery** — a half-typed line costs one error; everything below it
  is still analysed, so the rest of your diagnostics don't vanish while you
  type mid-statement.

Not yet: completion (planned — needs receiver-type knowledge at the cursor),
go-to-definition for functions and parameters.

## Architecture

- **`vbr-lsp`** (a Rust crate at the repo root) — the language server. It speaks
  the Language Server Protocol over stdio and reuses the `vbr` library as its
  compiler front-end. One `vbr::compile` gives it everything: structured
  diagnostics with byte spans, a hover table (span → type display), and
  go-to-definition pairs (use span → declaration span).
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
   - press F12 on a variable use to jump to its `Dim`.
