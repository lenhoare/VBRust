# TIDE

A Turbo Pascal–inspired TUI IDE for VBR, built on a reusable terminal code
editor — the “Monaco for TUIs.”

| Crate | Role |
|-------|------|
| **`tide-editor`** | Reusable ratatui editor (document / view / decorations / highlight hooks) |
| **`tide`** | Thin VBR IDE shell: blue menus, edit, save, compile-and-run |

Design north star is classic **Turbo Pascal**, not a remake of the desktop
`vbr-ide`. The existing Tauri IDE, VS Code extension, and LSP are left alone.

## Requirements

- Rust stable
- A terminal that supports ANSI colour
- The `vbr` CLI on `PATH` (or set `VBR_BIN`) to Run programs

## Build & run

```bash
cd tide
cargo run -p tide                 # empty buffer
cargo run -p tide -- ../examples/life_screen   # open a project folder
cargo run -p tide -- path/to.vbr               # open a file (auto-detects project)
cargo run -p tide-editor --example minimal -- path/to.txt
```

## Keys (IDE)

| Key | Action |
|-----|--------|
| `F10` | Menu bar |
| `F1` | Help |
| `F9` / `Ctrl+R` | Compile then run via `vbr` (blocked if front-end errors) |
| `Alt+F9` | Compile only — fill the Watch window |
| `F4` | Toggle generated Rust pane (TD-style, read-only + copy) |
| `Tab` | Cycle focus Editor ↔ Rust ↔ Watch |
| `Enter` (in Watch) | Jump to the selected diagnostic |
| `Ctrl+P` | Open project folder (Tab completes dirs) |
| `Ctrl+U` | Units list (switch `.vbr` files) |
| `Ctrl+F` | Find |
| `F3` / `Shift+F3` | Find next / previous |
| `Ctrl+H` | Replace (Enter = replace+next, Ctrl+A = all) |
| `Ctrl+S` | Save |
| `Ctrl+O` | Open (Tab completes paths) |
| `Ctrl+N` | New |
| `Ctrl+Q` | Quit |
| `Ctrl+Z` / `Ctrl+Y` | Undo / redo |
| `Ctrl+C` / `Ctrl+X` / `Ctrl+V` | Copy / cut / paste (in-app + OSC 52 to host) |
| Mouse | Menus, drag-select in editor, click a Watch line to jump |

## Later (end of the list)

Not blocking day-to-day use — park these until the TP core feels finished:

- Packaging / getting-started for distributing `tide`

Screen layout work lives in the sibling app **[`tide_design/`](../tide_design/)** (structural TUI Screen designer).

## Layout

```
tide/
  editor/   # tide-editor library
  app/      # tide binary (VBR customer)
```
