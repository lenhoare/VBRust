# VBR — Android editor

A phone-sized editor that **runs** VBR. The desktop IDE (`vbr-ide`) shells out to
`rustc`; a phone has no Rust toolchain, so this app takes the **C target**
(`language_spec.md` §14, `targets_spec.md` §4) and compiles it in-process with
**TinyCC**.

The VBR compiler itself still runs on the device — as a prebuilt native library,
not as `rustc`. User programs never touch the Rust toolchain.

The editor uses **unscii** (public domain VGA-style terminal glyphs) so the
chrome reads as a DOS text mode rather than a modern UI font.

```
.vbr  →  libvbr (Rust .so)  →  C pane (what `vbr c` would emit)
                         ↘  Main / Screen host (interpreter)  →  stdout / WebView
```

TinyCC is still linked and proven on the **host** (`cargo test`). On the phone,
`tcc_relocate` hangs: Android will not give an ordinary app executable JIT
pages (`execmem`). F9 therefore runs `Function Main()` in the same AST host
as `Screen`.

---

## 1. What it is

A phone-sized **TIDE**: Turbo Pascal chrome (blue / yellow / cyan), not a
remake of the desktop Monaco IDE.

- **Edit** `.vbr` with VBR highlighting (keywords, `' comments`, strings).
- **File** menu: New, Open file, Open project (folder with `main.vbr` or
  several `.vbr`), Units, Examples, Save / Save As, Quit. Phone storage uses
  the system picker (SAF); app-private `programs/` is always available.
- **Edit** menu: Undo/Redo, Cut/Copy/Paste, Find, Replace.
- **Run** menu: Compile (fills Watch), Run (F9), toggle the generated **C
  pane** (F4) — Turbo Debugger–style strip at the bottom, ~42% height,
  syntax-coloured, scrolls with the cursor via a VBR↔C line map.
- **Watch** window when diagnostics exist; tap / Enter jumps to the line.
- **Help**: keys and About.
- **Run** `Function Main()`: the same in-process interpreter as `Screen`
  captures `Debug.Print`. (TinyCC in-process JIT hangs on Android — no RWX
  memory — so the phone does not call `tcc_relocate`. F4 still shows C.)
- **Run a `Screen`**: F9 opens a tap-driven Turbo Vision surface with the same
  controls as the desktop TUI (Button, Checkbox, Radio, Input, List, Tabs,
  Gauge, menus, status keys) instead of Tab / Space / Enter.

Sideload / Android Studio. Not a Play Store package yet.

---

## 2. What runs (v1)

**Yes — the core language** on the C target: scalars, strings, `Dim`, arithmetic
(including `^` / maths builtins), `If`/`For`/`Do`/`Match`, `Type`/`Enum`,
`Vec`/`HashMap`, iterators, `Option`/`Result`/`?`, `Const`.

**Yes — `Screen` (the desktop TUI)** as a tap host: same State / View / Events
model, Turbo Pascal chrome. Click a Button instead of Tab+Enter; status-bar
key chips stand in for `On Key`. File → Examples → `tui_counter` / `tui_controls`.

**Not yet**, each with a teaching message rather than a mysterious linker error:

| Kind | Why |
|------|-----|
| `Window` / `Page` | GUI / web surfaces. `Screen` is the phone TUI. |
| `Http` / `Database` / `Json` | Need libcurl / sqlite / cJSON at link time. |
| `DataFrame` | No C lowering. |
| `FileSystem` / `DateTime` / `Regex` / `Shell` | Need POSIX headers whose layouts must match Bionic; later slice. |
| Inline `Rust` / `Python`, `Use` crates | No toolchain on the phone. |
| `InputBox` | The C backend doesn't lower it yet; a prompt UI is a later slice. |
| Screen `Table` / `Chart` / `GetOpenFilename` | Host paints a placeholder; List / Gauge / Sparkline work. |

A program that only `Debug.Print`s is the happy path.

---

## 3. Architecture

Three pieces, same split as `vbr-ide`:

| Piece | Role |
|-------|------|
| **`vbr-android/native`** | Rust cdylib. Wraps `vbr::compile` / `compile_c` / `complete`, plus an in-process **Main / Screen** interpreter. Links TinyCC for **host** tests. JNI for the app. |
| **TinyCC** | `third_party/tinycc` (fetched, not vendored). Host: `tcc -run` proves the C pipeline. Android does not call it — `tcc_relocate` hangs (no `execmem`). |
| **`vbr-android/app`** | Kotlin `WebView` shell around a bundled editor (`assets/index.html`). `JavascriptInterface` calls into the .so. Programs persist under the app's `filesDir`. |

The generated C is the same single self-contained `.c` `vbr c` would write; F4
shows it. F9 on the phone interprets `Main()` / `Screen` rather than JITing.

---

## 4. Tooling

```
vbr-android/scripts/fetch-tcc.sh   # clone TinyCC, host libtcc for tests
cd vbr-android/native && cargo test
```

The Android APK is an Android Studio project (`vbr-android/`). It needs the SDK
**and** NDK (to cross-compile the Rust .so + libtcc for `arm64-v8a` / `x86_64`).
See `vbr-android/README.md`.
