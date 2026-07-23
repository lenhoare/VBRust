# VBR IDE

A desktop editor for VBR: type VB-flavoured source on the left, watch the
idiomatic Rust appear on the right — the whole point of the language, in one
window. It's a thin shell around the `vbr` compiler itself, so the Rust you see
is exactly what the CLI would produce.

**Status: slices 1 + 6.** Two panes (editable VBR, read-only Rust) with a
live-updating transpile, a diagnostics strip, and **syntax highlighting** for
the VBR pane (a Monarch grammar in `src/vbrLanguage.ts`, mirroring the lexer's
keywords, with the verbatim `Rust`/`Python`/`Text` blocks handled so their
interiors aren't mis-coloured). Build/Run, the project tree, and LSP-driven
completion/hover land in later slices (see the task list).

## How it's put together

Three pieces, each doing one job:

- **`vbr-ide-core/`** — a plain Rust crate that wraps `vbr::compile`. It knows
  nothing about the desktop; it just turns source into `{ rust, diagnostics }`.
  Because it has no webview dependency, it builds and unit-tests on any
  platform (`cd vbr-ide-core && cargo test`).
- **`src-tauri/`** — the Tauri (Rust) shell. It owns the window and exposes
  `transpile_source` as a command the frontend calls. Later slices add commands
  that shell out to `cargo`/`vbr` and spawn `vbr-lsp`.
- **`src/` + `index.html`** — the frontend: [Monaco](https://microsoft.github.io/monaco-editor/)
  (VS Code's editor) in two panes, wired to the backend over Tauri's IPC.

## Running it (Windows — the primary target)

You need, once:

1. **Rust** with the MSVC toolchain (`rustup default stable-msvc`).
2. **Node.js** (18+) and npm.
3. **WebView2** — preinstalled on Windows 10/11; if not, grab the Evergreen
   runtime from Microsoft.
4. The Tauri CLI is pulled in as a dev-dependency, so `npm run tauri …` works
   without a global install.

Then, from `vbr-ide/`:

```powershell
npm install                 # frontend deps (Monaco etc.)
npm run tauri icon assets\logo.png   # once: generate the app icons (any square PNG)
npm run tauri dev           # launches the app with hot-reload
```

To produce a distributable:

```powershell
npm run tauri build         # → src-tauri\target\release\bundle\  (.msi / .exe)
```

> **Icons:** `tauri build` (and packaging) needs the icon set referenced in
> `src-tauri/tauri.conf.json`. Run `npm run tauri icon <a-square-png>` once to
> generate them into `src-tauri/icons/`. `tauri dev` will run without them.

## Running it on Linux / WSL

The frontend and the core crate build anywhere, but the Tauri **shell** needs
the WebKitGTK/GTK dev libraries to build and run on Linux:

```sh
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev \
                 libayatana-appindicator3-dev
```

(WSL also needs WSLg for a GUI window.) On Windows none of this applies — it
uses WebView2.

## What you can verify without the desktop toolchain

```sh
cd vbr-ide-core && cargo test   # the compiler integration (3 tests)
npm install && npm run build    # the frontend bundles to dist/
```

Both pass today; the Tauri shell is verified by running `npm run tauri dev` on
Windows.
