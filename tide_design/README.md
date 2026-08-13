# tide_design

Structural **Screen** designer for VBR — a sibling to [TIDE](../tide/).

> Don't draw the screen. Build its structure quickly and see it continuously.

| Pane | Role |
|------|------|
| **Palette** | View page: Column, Row, Frame, Tabs, Tab, Text, Space, Button, Checkbox, Radio, Input, Memo, List, Table, Gauge, Sparkline, BarChart, Chart. Menu page: Menu, Item, Separator |
| **Tree** | View structure, or the menu bar |
| **Preview** | Static Ratatui preview (highlight follows selection; menu bar draws when present) |

Two pages, now on the **View** menu next to File: **Screen** (the widget tree) and
**Menu** (Screen chrome). **F4** still toggles. The menu bar is not a palette
widget on the Screen page — it lives next to `View` in the emitted Screen.

No pixel dragging. No Window/GUI forms. Interactive “test mode” is a later slice.

## Run

```bash
cd tide_design
cargo run
cargo run -- templates/notes.vbt   # open a Screen template
```

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
| `Ctrl+N` | New design |
| `Ctrl+O` | Open `.vbt` template |
| `Ctrl+S` | Save (`.vbr` program, or `.vbt` if that's the current file) |
| `Tab` | Cycle focus Palette → Tree → Preview |
| `Ctrl+Q` | Quit (also File → Quit) |

## Output

**Save** / **Save as…** emit a complete `Screen … End Screen` plus `Function Main` — a `.vbr` you can open in TIDE or `vbr run`.

**Save as template…** writes a `.vbt`: the same Screen syntax, but structure only (Title, Menu, View). No `State`, no `Event` bodies, no `On Key`, no `Main`. **Open template** loads that back into the tree.

A `.vbr` a human has been editing is refused — the designer will not try to pick the View out of mixed logic. Start from a template, or from blank.

Example: `templates/notes.vbt`.

## Later

- Interactive preview (widgets actually focus / type)
- Starter kits (Master/Detail, …) and “save selection as component”
- Hand-off “Open in TIDE”
