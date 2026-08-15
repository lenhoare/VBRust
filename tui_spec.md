# Bust TUI Specification

A `Screen` is a terminal (text) user interface, the counterpart to a `Window`
(the graphical GUI — see `gui_spec.md`). It compiles to a **ratatui** application.

Both backends share the same core: **State is the source of truth, the View is
derived from it, and Events change it** (The Elm Architecture). That half of the
language is renderer-agnostic — only the widgets and the runtime differ. So most
of what you know from the GUI carries over; this document covers what's specific
to the terminal.

---

## 1. Design goals

- **Same model as the GUI** — `State` / `View` / `Event`, so knowledge transfers.
- **Honest to the terminal** — a `Screen` is *not* a `Window`: input is
  keyboard-first, widgets are text, and the vocabularies differ. They coexist in
  one project rather than pretending to be one portable surface.
- **Clean, readable generated Rust** — the crossterm loop it emits is meant to be
  read and learned from (no hidden async machinery unless you ask for it).
- **Data-viz friendly** — first-class charts, because dashboards are a natural
  terminal use case.

---

## 2. Conceptual model

A `Screen` mirrors a `Window`:

```vb
Screen Counter
    Title "Counter"

    State
        Dim count As Integer = 0
    End State

    View
        Column
            Text "Count: " & count
            Text "(+/- to change, q to quit)"
        End Column
    End View

    On Key "+" Increment
    On Key "-" Decrement
    On Key "q" Quit

    Event Increment
        count += 1
    End Event

    Event Decrement
        count -= 1
    End Event
End Screen

Function Main()
    Counter.Run
End Function
```

### 2.1 State

Identical to a Window's `State` — primitives, enums, and `Vec<T>` collections
(fill in an event, iterate/plot in the view). A **selectable widget** (`List`,
`Table`) or an **input** join the *focus ring* automatically; lists/tables also
carry a hidden runtime cursor.

A field's initialiser may be **fallible** (`Dim db As Database =
Database.Open("ideas.db")`, or your own `Result`-returning function): the state
is then built *before* the terminal starts, and on failure the program prints
`could not start: <why>` and exits cleanly — see the GUI spec §2.1 for the full
rules (identical here; `examples/tui_ideas.vbr` shows a Database in State).
Native-only: a browser Screen gets a teaching error.

### 2.2 View

A tree of widgets (see §4), laid out with `Column`/`Row` and per-child size
constraints (§3). Unlike the GUI (widget properties), the terminal splits the
area into rectangles.

### 2.3 Events & input

Terminal input is keyboard-driven. There are three ways an event fires:

- **Keymap** — `On Key <key> <Event>` binds a key.
- **Timer** — `Every <ms> <Event>` fires on an interval (§6).
- **Focus** — the focused widget receives built-in keys (§5): a `List`/`Table`
  navigates with Up/Down and activates with Enter; an `Input` types; `Tabs`
  switches with Left/Right.

`Quit` is a built-in handler that exits: `On Key "q" Quit`. An optional string
after the handler is the **hotkey label** on the bottom status bar
(`On Key "q" Quit "quit"`). Omit it and the handler name is used.

> **Early-out in an event.** A `Screen` event lowers to a function that returns
> `std::io::Result<()>`, so a bare `Return` on its own is a type error (rustc:
> "`return;` in a function whose return type is not `()`"). To leave an event
> early, structure the logic with `If … Else` so it falls through, or guard the
> rest — `If Not won Then … End If` — rather than `If won Then Return`.

### 2.4 Status line

A Screen draws a bar along the bottom of the terminal (Turbo Vision style):
reversed key caps, then the label. Unthemed, it is cyan-on-black; a `Theme`
recolours it. `On Key` bindings appear automatically;
Tab / Enter / Up-Down / Left-Right are added when those built-ins apply. `Status <expr>`
puts live text on the left of the same bar:

```vb
Status "Count: " & count
On Key "+" Increment "inc"
On Key "-" Decrement "dec"
On Key "q" Quit "quit"
```

`Status` is Screen-only. A screen with no keys, no focusable widgets, no menu bar,
and no `Status` line has no bar (the view keeps the full height).

### 2.5 Theme

`Theme <Name>` under the Title restyles the Screen — the same names as a Window
(`Dracula`, `Nord`, …) plus **`NightOwl`** and **`JellyFish`**. It colours the
status bar, menu bar, outer border, text, chart series, gauges, and the file
prompt. Omit it and chrome stays cyan-on-black.

```vb
Screen Counter
    Title "Night Owl"
    Theme NightOwl
    …
End Screen
```

`Theme Night_Owl` is the same name. An unknown name lists every built-in.

### 2.6 Menu bar

A Screen may declare a **menu bar** next to `View` — chrome, like `Title` and
`Status`, not a widget inside the Column. One-level dropdowns (no nested
submenus in v1):

```vb
Screen App
    Title "App"
    Menu
        Menu "File"
            Item "Beep" Beep
            Separator
            Item "Quit" Quit
        End Menu
        Menu "Help"
            Item "About" About
        End Menu
    End Menu

    View
        Text "F10 opens the menu"
    End View
End Screen
```

**F10** opens the first menu; **Alt+letter** opens the dropdown whose title
starts with that letter. Left/Right switch menus, Up/Down move (skipping
separators), Enter fires the item's Event (or built-in `Quit`), Esc / F10
closes. A letter while open matches the first letter of an item, or of another
menu title. While the menu is open it owns the keyboard (same steal as a Memo).

The bar is a Length(1) row above the titled view; the dropdown overlays the
body. Screen-only — a Window or a `Menu` inside `View` gets a teaching error.
In the browser the bar draws but isn't interactive yet (`tui-web-menu`).

Example: `examples/tui_menu.vbr`. In tide_design, **F4** switches the Menu page
(the view tree stays on View).

Event bodies are ordinary Bust — the same resolution pass a function body gets
(stdlib methods, string/numeric coercions, iterator chains, teaching
diagnostics), with the screen's state fields in scope — at any statement depth:
state fields inside `For`/`For Each`/`Do` bodies, `Match` arms, and `If`
branches all rewrite to `state.field` (`examples/tui_life.vbr`). This is shared
with the GUI backend (`src/surface.rs`); a `Screen` event and a `Window` event
lower identically. *(BUILT — 2026-07-04.)*

### 2.7 File dialogs

`GetOpenFilename()` / `GetSaveAsFilename()` (optional initial path) pop a path
prompt over the live Screen — the same Open / Save As box TIDE uses, as a
function, like VBA's `Application.GetOpenFilename`. Not a View widget.

```vb
Event OpenFile
    Dim path As String = GetOpenFilename()
    If path <> "" Then
        Match FileSystem.Read(path)
            Ok(text) => notes = text
            Err(e) => notes = "Could not read: " & e
        End Match
    End If
End Event
```

Tab completes (cycle matches; a unique directory then lists children; `../`
climbs). Enter on a folder browses in; Enter on a file returns it (Save As
returns even a name that doesn't exist yet). Esc cancels and returns `""`.
Call them from a Screen **event** only (they need the live terminal). Timers
and async pause while the prompt is open. Screen-only — a Window or a helper
`Function` / Screen `Sub` gets a teaching error. In the browser they return
`""` (`tui-web-file-dialog`).

Example: `examples/tui_file.vbr`.

**Multi-file projects.** A `Screen` joins a project like any other entry: put
the UI in `main.vbr` and the logic in sibling modules, and call them qualified
— from State initialisers (`Dim grid As Vec<Long> = Life.NewGrid()`), events
(`Life.SetCell(grid, x, y, 1)` → `crate::life::setcell(&mut state.grid, …)`),
and helper functions, all with the full cross-module argument treatment
(`projects_and_run_spec.md`). A sibling's `Public Type`/`Enum` is used by its
bare name — State can hold one (`Dim rule As Rule = Life.ClassicRule()`) and
events can call its methods. One limit: a *view* expression can't read
`Life.WIDTH` directly (views don't run the resolver) — mirror the value into
state or read it through a helper. Example: `examples/life_screen/`.

---

## 3. Layout

`Column` (vertical) and `Row` (horizontal) split their area. A **size line before
a child** constrains it along the container's main axis:

```vb
Column
    Length 1
    Text " header"
    Fill
    List items          ' takes the remaining space
    Length 1
    Text " footer"
End Column
```

Size constraints:

| Line        | ratatui `Constraint` | Meaning                          |
|-------------|----------------------|----------------------------------|
| `Length N`  | `Length(N)`          | exactly N rows/cols              |
| `Percent N` | `Percentage(N)`      | N% of the container              |
| `Fill` / `Fill N` | `Fill(N)`      | share leftover space, weight N   |
| `Min N`     | `Min(N)`             | at least N                       |

`Spacing N` (gap between children) and `Padding N` (margin) also apply. Sensible
defaults when unsized: containers, conditionals, scrollables, charts, and `Tabs`
`Fill`; an `Input` is 3 rows, `Text` is 1 row. A titled border frames the whole
screen. Nested **`Frame`** widgets add titled panels inside the view (see §4).

---

## 4. Widgets

### Text
`Text <expr>` — a line of text (`Paragraph`). Concatenate with `&`.

### Frame  *(titled panel)*
`Frame ["title"]` … `End Frame` — a bordered box (ratatui `Block`) wrapping
children. The title is optional (`Frame` alone is just a border). Several
children stack as a Column; `Spacing` / `Padding` / size lines work as in
`Column`/`Row`.

```vb
Frame "Customers"
    List people
        On Select Open
    End List
End Frame
```

### Space  *(gap)*
`Space Height N` / `Space Width N` — a blank gap of N rows or columns.

### Button  *(push)*
`Button "label"` … `End Button` — a `[ label ]` control. **Enter** or **Space**
fires `On Click` (optional). Same syntax as a Window button.

```vb
Button "Save"
    On Click Saved
End Button
```

### Checkbox  *(boolean)*
`Checkbox "label", field` … `End Checkbox` — `[x]` / `[ ]` bound to a `Boolean`.
Enter/Space toggles the field, then fires `On Toggle` (optional) with the new
value — so the same `Event Toggled(value As Boolean)` / `field = value` body
works on a Window and a Screen.

### Radio  *(one of a set)*
`Radio "label", field, option` … `End Radio` — `(*)` / `( )`. Each Radio offers
one option; the bound field (an enum or integer) holds the selection. Enter/Space
assigns `option` into the field, then fires `On Select` (required, as in a Window).

```vb
Radio "Small", choice, Size.Small
    On Select Pick
End Radio
```

### Input  *(text entry)*
`Input <field>` bound to a `String` state field, with optional `On Submit`:

```vb
Input query
    On Submit Search
End Input
```
The focused input receives typed characters and Backspace; Enter fires
`On Submit`, which gets the typed text as a parameter
(`Event Search(text As String)`).

### Memo  *(multi-line edit)*
`Memo <field>` bound to a `String` — a multi-line editor (tui-textarea). Enter
inserts a newline; arrows move the caret; Tab leaves when anything else is
focusable. Quit with `Esc` (a `"q"` binding would steal the letter). Screen-only
— a Window uses `TextArea` (a different backend buffer).

```vb
Memo notes
End Memo
```

The typed text *is* the string (`notes`, `notes.Len()`). Hidden editor state
holds the caret, like a List's hidden cursor. Example: `examples/tui_memo.vbr`.

### List  *(selectable)*
`List <field>` over a `Vec<String>`, optional `On Select`:

```vb
List fruits
    On Select Chosen        ' Event Chosen(item As String)
End List
```
Up/Down move the highlight; Enter fires `On Select` with the **selected item**.

### Table  *(selectable, columns from a struct)*
`Table <field>` over a `Vec<Struct>` — one column per struct field, field names as
the header. `On Select` receives the **selected row** (the struct):

```vb
Table people
    On Select Show          ' Event Show(who As Person)
End Table
```

### Charts  *(display-only)*
- **`Gauge min..=max, field`** — a progress gauge over a numeric field.
- **`Sparkline field`** — a compact trend line over a `Vec` of numbers.
- **`BarChart field`** — bars over a `Vec<Struct>`; first `String` field labels
  each bar, first numeric field is its height.
- **`Chart …`** — an X/Y line or scatter chart over `Vec<Struct>` series (first
  two numeric fields = x, y). One or more series, each its own colour + legend:

  ```vb
  Chart prices, average          ' quick comma form (auto axes)

  Chart                          ' block form
      Series linear
      Series quad
      XAxis 0..=10               ' explicit bounds (else auto)
      YAxis 0..=100
      Scatter                    ' points instead of a line
  End Chart
  ```

### Tabs  *(tab bar + pages)*
`Tabs <field>` … `Tab <title>` … `End Tab` … `End Tabs` — a tab bar (ratatui
`Tabs`) plus one page per `Tab`. `field` is an **`Integer` index, 0-based**
(first tab = 0). The bar is **focusable**: **Left/Right** cycle (wrapping),
**Enter** advances, and digit keys **1–9** jump (1 = first tab). Optional
`On Change <Event>` fires with the new index.

```vb
Tabs tab
    Tab "Overview"
        Text "Welcome"
    End Tab
    Tab "Details"
        List items
            On Select Pick
        End List
    End Tab
End Tabs
```

Layout is a one-row bar plus a Fill body (several children in a pane stack as a
Column). Size lines work inside a `Tab` the same as in `Column`. `Tab` is only
valid inside `Tabs`. Screen-only — a Window gets a teaching error.

A **focusable** widget (`List`/`Input`/`Table`/…) may live inside a pane; its
selection/typing state is wired up even when that pane is hidden. Example:
`examples/tui_tabs.vbr`, `examples/tui_list_tabs.vbr`.

### Match / If in the view
Show different widgets by condition — identical to the GUI:

```vb
Match mode
    1 => Text "Overview"
    _ => Text "Settings"
End Match
```

A **focusable** widget may also live inside a `Match` arm or an `If` branch.
`Tabs` is the usual way to switch pages; `Match` remains for arbitrary
conditions.

---

## 5. Focus

`Input`, `List`, `Table`, `Button`, `Checkbox`, `Radio`, `Tabs`, and `Memo` are **focusable**.
With more than one on screen, **Tab** cycles focus, and the focused widget gets
the relevant built-in keys:

- **Input** — printable keys type, Backspace deletes, Enter submits.
- **Memo** — printable keys type, Enter is a newline, arrows move the caret.
- **List/Table** — Up/Down move the selection, Enter selects.
- **Button / Checkbox / Radio** — Enter or Space activates (click / toggle / pick).
- **Tabs** — Left/Right cycle (wrap), Enter advances, 1–9 jump to a pane.

Your own `On Key` bindings take precedence, so a globally-bound character key
can't also be typed into an input or memo — quit/act via `Esc` or a named
key.

Named keys for `On Key`: `Up`, `Down`, `Left`, `Right`, `Enter`, `Esc`, `Tab`,
`Space`, `Backspace`, `F1`–`F12`; otherwise a single character in quotes (`"q"`, `"+"`).

---

## 6. Timers — `Every`

`Every <ms> <Event>` fires a handler on an interval. Combined with `Await`
(§7), this gives periodic background polling for free:

```vb
Every 1000 Tick          ' a clock / animation
Every 5000 Refresh       ' Refresh may Await Http.Get(...) → live dashboard

Event Tick
    seconds += 1
End Event
```

A screen with a timer keeps ticking (it doesn't block waiting for a keystroke).

---

## 7. Async — `Await`

Slow work (HTTP, heavy compute) must not block the loop or the whole screen
freezes. `Await` in an event runs the work on a background thread and updates
state when it lands — the same `Await` as the GUI:

```vb
Event Fetch
    status = "loading…"
    Match Await Http.Get(url)
        Ok(_)  => status = "done"
        Err(e) => status = "error: " & e
    End Match
End Event
```

The generated loop stays synchronous and readable: a `std::sync::mpsc` channel
delivers the result, the loop polls input briefly (so it keeps ticking) and
drains results with `try_recv`. No `tokio`/async-`main`. A blocking stdlib call
used **without** `Await` is a friendly error ("would freeze the UI, use `Await`").

Forms: `Match Await …` (fallible, e.g. `Http.Get`) and `Dim x = Await …`
(infallible). One `Await` per event, and it must be a **top-level** statement —
not nested inside an `If`/`For`/`Match`. This is deliberate: a top-level `Await`
lowers to a plain "kick off the work, resume in the continuation" pair with no
hidden state machine, keeping the generated loop readable. To guard the call, put
the check *before* the `Await` (`If busy Then Return`, or set a flag first), or
move the guard into the awaited helper (return early). Nesting an `Await` earns a
teaching error that points at these options — Bust keeps async simple on purpose;
reach for real Rust when you need more.

---

## 8. Running

`Function Main()` launches the screen with `<Screen>.Run`, just like a Window.
The generated `main` is a crossterm loop (`ratatui::init()` → draw → read key →
dispatch → `ratatui::restore()`); it takes over the terminal, so run it in a real
terminal (not piped), and it restores on exit. Adding a `Screen` pulls in
`ratatui` (crossterm comes with it); it builds far faster than the GUI's Iced.

### 8.0 Diagnosing a running screen — `Log`, not `Debug.Print`

A `Screen` owns the terminal, so `Debug.Print` scribbles over the UI — Bust warns
and sends you to **`Log`**. `Log "message"` (composes with `&` like
`Debug.Print`) appends a timestamped line to `build/vbr.log`; open a second
terminal and `tail -f build/vbr.log` to watch the app think while it runs. `Log`
works in any event or helper. `vbr run` prints the log path at startup. See
`language_spec.md` §Logging.

### 8.1 Running in the browser — `vbr runweb`

The **same `Screen` file** also runs in a browser:

```sh
vbr runweb examples/tui_counter.vbr    # serve it (trunk, like a Page)
vbr build --web examples/tui_counter.vbr   # just generate the project
```

This swaps the shell, not the program: it compiles to WebAssembly against
**Ratzilla** (pinned 0.3, on ratatui 0.30), which draws real ratatui widgets
into the DOM — same State struct, same `view`, same event bodies, byte for
byte. Only `fn main` differs: the state lives in an `Rc<RefCell<_>>` shared by
an `on_key_event` handler (dispatching the same keymap) and a `draw_web`
render loop. The one-time setup is the web toolchain from `web_spec.md` §4
(the wasm target + trunk).

The focusable widgets (`Input`, `List`, `Table`) work in the browser with the
same built-in navigation as the terminal (§5): Tab cycles focus, arrows move
the selection, Enter submits/selects, typing and Backspace edit the focused
input — the identical dispatch, wired into the browser's key handler.

`Every` timers (§6) run on browser interval timers (gloo-timers), each
executing the same handler body against the shared state; the render loop
picks the change up automatically. `examples/tui_pulse.vbr` — a timer-driven
Gauge + Sparkline animation — runs identically in both shells.

`Await` (§7) works too, on the browser's own machinery: the event splits
exactly as in the terminal, but the awaited `Http.Get` runs on the browser's
`fetch` (the same generated `http_get` wrapper a `Page` uses — see
`web_spec.md` §5, including the CORS note), and the continuation runs in a
spawned future (`spawn_local`) that re-borrows the state when the result
lands — no channel, no thread. `tui_monitor.vbr` — timers + async refresh —
is the full demo: `vbr runproject` for the terminal, `vbr runweb` for a URL.

Web differences, each said out loud rather than silently diverged:

- A `Quit` binding (key or timer) is dropped (a page can't quit itself —
  close the tab); a note says so.
- The stdlib beyond `Await Http.Get` is a teaching error (it doesn't compile
  to WebAssembly), as is `Await` on your own functions (no browser threads).
  The terminal version of the same file runs both today.

*(BUILT — 2026-07-06, complete: the shell, keymap + sync events, the full
widget set including focus/Input/List/Table, `Every` timers, and async
`Await Http.Get` — the widget lowering compiles unchanged against ratatui
0.30 on wasm.)*

---

## 9. Deferred

- **True streaming / progress from inside one task** — emitting repeated/partial
  updates from a single long computation (progress bars, tailing) needs an
  emit-from-work mechanism + cancellation. Timers cover interval *polling*; this
  is the other half.
- **Cross-widget richer layout** the GUI doesn't have either (e.g. absolute).
- **A shared View subset** unified with the GUI, once it's clear what converges.

---

## Examples

`examples/tui_counter.vbr` (keymap), `tui_layout.vbr` (dashboard layout),
`tui_list.vbr` / `tui_panels.vbr` (list + focus), `tui_frame.vbr` (titled panels),
`tui_table.vbr`,
`tui_input.vbr` (input + list), `tui_memo.vbr` (multi-line edit), `tui_menu.vbr` (menu bar), `tui_controls.vbr` (Button / Checkbox / Radio),
`tui_tabs.vbr` (Tabs widget), `tui_list_tabs.vbr` (lists in panes), `tui_dashboard.vbr`
(Gauge/Sparkline/BarChart), `tui_chart.vbr` / `tui_multichart.vbr` (XY charts),
`tui_fetch.vbr` (async), `tui_monitor.vbr` (timers + async), `tui_pulse.vbr`
(timer-driven animation, terminal + browser), `tui_nightowl.vbr` / `tui_jellyfish.vbr`
(`Theme` on a Screen).
