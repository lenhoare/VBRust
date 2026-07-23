# VBR IDE

A desktop editor for VBR: type VB-flavoured source on the left, watch the
idiomatic Rust appear on the right — the whole point of the language, in one
window. It's a thin shell around the `vbr` compiler itself, so the Rust you see
is exactly what the CLI would produce.

**Status: a working editor.** What's in:

- **Two live panes** — editable VBR left, read-only generated Rust right,
  updating as you type, with a draggable divider to resize them.
- **Inline diagnostics** — squiggles on the exact offending span (teaching
  message on hover), a summary strip you can click to jump to the problem, and
  counts in the status bar.
- **Run** — the ▶ Run button (or `Ctrl+Enter`) compiles and runs the current
  buffer, streaming its output to the console. (Single-file, std-only programs;
  stdlib/GUI programs are projects — that's a later slice.)
- **Intelligence, in-process** — completion (members after `.`, names in
  scope), hover (VB type · Rust type), and go-to-definition, all served by the
  `vbr` compiler directly through Tauri commands. No LSP server: for a native
  app the intelligence is a library call away.
- **Syntax highlighting** — a Monarch grammar (`src/vbrLanguage.ts`) mirroring
  the lexer's keywords, with the verbatim `Rust`/`Python`/`Text` blocks handled
  so their interiors aren't mis-coloured.
- **Files** — New / Open / Save (`Ctrl+N`/`O`/`S`, native dialogs via `rfd`),
  with the filename in the status bar; work also auto-persists to localStorage.
- **Comfort** — light/dark theme toggle, `Ctrl`-scroll zoom, a built-in example
  picker (loaded straight from the repo's `examples/`).

Still to come: a project tree (folder = project, so stdlib/GUI programs run),
`Graduate`/`Test` buttons, and Windows packaging.

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
cd vbr-ide-core && cargo test          # the compiler integration (11 tests:
                                       # transpile, run, ranges, completion,
                                       # hover, go-to-def, position mapping)
npm install && ./node_modules/.bin/tsc --noEmit   # the frontend typechecks
npm run build                          # the frontend bundles to dist/
```

All pass today. The Tauri shell (window, Run, file dialogs) is verified by
running `npm run tauri dev` on Windows — it needs WebView2 / the MSVC toolchain,
which don't exist on the Linux/WSL side.
