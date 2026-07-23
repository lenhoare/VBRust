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
- **Projects** — Open a folder to get a file-tree sidebar; a folder with a
  `main.vbr` is a *project* (opens on its entry point), and Run then builds and
  runs the whole thing via `vbr runproject` — so stdlib/GUI programs run too.
- **Graduate & Test** — for an open project, **Test** runs `vbr test`, and
  **Graduate** promotes the selected module's generated Rust to source
  (`vbr graduate`) and refreshes the tree. Both stream output to the console.
- **Form designer** — the **Designer** button opens a visual builder: drop
  controls into `Column`/`Row` layouts, set their properties, and watch clean
  VBR `View` code appear on the right, one-way (design → code). *Insert into
  editor* drops it into your `Window` file. The palette is the genuinely
  GUI-valid widget set (charts/lists are Screen-only; a Window charts on a
  Canvas). Codegen lives in `vbr-ide-core::design` and is unit-tested.
- **Comfort** — light/dark theme toggle, `Ctrl`-scroll zoom, a built-in example
  picker (loaded straight from the repo's `examples/`), a `?` shortcuts overlay.

Still to come: installer packaging (`.msi` / `.deb` / `.AppImage`).

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

> **Icons** are already generated in `src-tauri/icons/` (from `assets/logo.png`).
> To rebrand, replace that PNG and run `npm run tauri icon assets\logo.png`.

## Building on Linux

The Tauri **shell** needs the WebKitGTK/GTK development libraries. On Ubuntu
24.04 (WebKitGTK 4.1) there's a one-shot script:

```sh
./scripts/setup-linux.sh      # installs the system deps (needs sudo)
npm install
npm run tauri dev             # run it (needs a display; on WSL that's WSLg)
npm run tauri build           # → src-tauri/target/release/bundle/  (.deb / .AppImage)
```

Notes from bringing it up on Linux:

- **Dialogs use the async `rfd` backend** (DBus portal on Linux), so Open/Save/
  Open-Folder don't hit GTK's main-thread rule. The portal needs
  `xdg-desktop-portal` running (present on most desktops; on **WSL** it may need
  `xdg-desktop-portal-gtk` installed and a WSLg session).
- Running a **project** (Run on a folder with `main.vbr`) shells out to the
  `vbr` binary's `runproject`. Put `vbr` on your `PATH`, or set `VBR_BIN` to its
  path (e.g. the repo's `target/debug/vbr`).
- If the build fails on `glib-2.0.pc` (or another `*.pc`), a `-dev` package
  didn't get pulled in transitively — the script now installs `libglib2.0-dev`
  and `libsoup-3.0-dev` explicitly. Verify with
  `pkg-config --exists glib-2.0 gtk+-3.0 webkit2gtk-4.1 libsoup-3.0`.

On Windows none of this applies — it uses WebView2 and native dialogs.

## What's verified, and where

```sh
cd vbr-ide-core && cargo test          # compiler integration — 13 tests:
                                       # transpile, run, project reading, ranges,
                                       # completion, hover, go-to-def, mapping
npm install && ./node_modules/.bin/tsc --noEmit   # the frontend typechecks
npm run build                          # the frontend bundles to dist/
```

All of the above pass. The Tauri **backend** (`src-tauri`) can only be compiled
where the WebKitGTK/WebView2 libraries are installed — so the Rust in
`src-tauri/src/main.rs` is written and reviewed but is first *compiled* by
`npm run tauri dev`/`build` on a machine with the deps (your Windows side, or a
Linux box after `scripts/setup-linux.sh`). The heavy lifting all lives in
`vbr-ide-core`, which is fully tested here.
