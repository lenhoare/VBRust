# tide_design

Structural **Screen** designer for Bust — a sibling to [TIDE](../tide/).

> Don't draw the screen. Build its structure quickly and see it continuously.

| Pane | Role |
|------|------|
| **Palette** | View page: Column, Row, Frame, Tabs, Tab, Text, Space, Button, Checkbox, Radio, Input, Memo, List, Table, Gauge, Sparkline, BarChart, Chart. Menu page: Menu, Item, Separator |
| **Tree** | View structure, or the menu bar |
| **Preview** | Static Ratatui preview (highlight follows selection; menu bar draws when present) |

Two pages, now on the **View** menu next to File: **Screen** (the widget tree) and
**Menu** (Screen chrome). **F4** still toggles. The menu bar is not a palette
widget on the Screen page — it lives next to `View` in the emitted Screen.

No pixel dragging. No Window/GUI forms. **Run → Test** (F9) launches the Screen through Bust.

## Run

```bash
cd tide_design
cargo run
cargo run -- templates/notes.vbt   # open a Screen template
```

**File → Open** starts in `templates/` with a blank name — Tab cycles the `.vbt` files (filenames only). **Save as template…** is the same folder with a suggested name. **Save as…** (`.vbr`) uses the current directory.

## Templates

Twenty starter Screens — structure only, no event bodies. Open one, rearrange, then Save as a `.vbr` when you want logic.

| File | Pattern |
|------|---------|
| `notes.vbt` | Scratch memo (Notepad / Turbo editor) |
| `login.vbt` | Username, password, OK / Cancel |
| `settings.vbt` | Tabbed checkboxes and radios |
| `master_detail.vbt` | List + detail fields (Access / FileMaker) |
| `dashboard.vbt` | Gauges, sparkline, bar chart |
| `file_browser.vbt` | Two-pane commander (Norton / Midnight) |
| `search.vbt` | Query box + results list |
| `wizard.vbt` | Stepped tabs: Welcome → Details → Confirm |
| `crud.vbt` | Table + New / Edit / Delete |
| `log_viewer.vbt` | Filter + level radios + log memo |
| `chat.vbt` | Transcript + compose line |
| `mail.vbt` | Folders \| messages \| body (Pine) |
| `calendar.vbt` | Agenda list + event detail |
| `todo.vbt` | Add line + task list |
| `inspector.vbt` | Labelled property rows (VB6 Properties) |
| `diff.vbt` | Two memos side by side |
| `repl.vbt` | Output memo + command line |
| `menu_app.vbt` | Menu-heavy workbench (Turbo Pascal) |
| `chart_report.vbt` | Chart above a data table |
| `status_board.vbt` | Gauges, sparkline, alerts list |

## Keys

| Key | Action |
|-----|--------|
| `↑↓` | Move selection in the tree |
| `F2` / click palette | Add component under selection |
| `Enter` | Properties (Tab to Size, ←→ cycle) |
| `Del` | Remove node (not the root Column) |
| `Alt+↑↓` | Reorder among siblings |
| `Alt+←` | Move out one level |
| `Alt+→` | Nest inside preceding container |
| `F4` | Switch View → Screen / Menu |
| `F10` | File menu |
| `F9` | Run → Test (interactive Screen via `vbr runproject`) |
| `Ctrl+N` | New design |
| `Ctrl+O` | Open `.vbt` template |
| `Ctrl+S` | Save (`.vbr` program, or `.vbt` if that's the current file) |
| `Tab` | Cycle focus Palette → Tree → Preview |
| `Ctrl+Q` | Quit (also File → Quit) |

## Output

**Run → Test** (F9) writes a temp `main.vbr` and runs `vbr runproject` — the real Screen takes over the terminal (Tab between controls, type, **q** to quit). Put `vbr` on PATH, or set `VBR_BIN`. First Test compiles ratatui; later ones reuse the cache.

**Save** / **Save as…** emit a complete `Screen … End Screen` plus `Function Main` — a `.vbr` you can open in TIDE or `vbr runproject`.

**Save as template…** writes a `.vbt`: the same Screen syntax, but structure only (Title, Menu, View). No `State`, no `Event` bodies, no `On Key`, no `Main`. **Open template** loads that back into the tree.

A `.vbr` a human has been editing is refused — the designer will not try to pick the View out of mixed logic. Start from a template, or from blank.

## Later

- “Save selection as component”
- Hand-off “Open in TIDE”
