# VBR Language Support (VS Code) — proof of concept

Live diagnostics for `.vbr` files: as you type, the VBR compiler runs and its
errors, warnings, and teaching notes appear as squiggles — the same messages you
get on the command line, now in the editor.

This is **tier 1** (line-level diagnostics). Hover, completion, and
go-to-definition come later, once the compiler tracks column spans.

## Architecture

- **`vbr-lsp`** (a Rust crate at the repo root) — the language server. It speaks
  the Language Server Protocol over stdio and reuses the `vbr` library as its
  compiler front-end (`vbr::compile` → structured diagnostics → LSP diagnostics).
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
   Development Host. Open any `.vbr` file (e.g. from `examples/`) and introduce a
   mistake — say `Dim x = 5` with no `As` type — to see a red squiggle with the
   VBR error message.

## Status

Working: diagnostics on open and on every edit (errors, warnings, notes), each
spanning its whole line. The line-level span is deliberately coarse for now;
precise (column) squiggles arrive with the span work in the compiler.
