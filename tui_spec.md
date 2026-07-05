# VBR TUI Specification

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

### 2.2 View

A tree of widgets (see §4), laid out with `Column`/`Row` and per-child size
constraints (§3). Unlike the GUI (widget properties), the terminal splits the
area into rectangles.

### 2.3 Events & input

Terminal input is keyboard-driven. There are three ways an event fires:

- **Keymap** — `On Key <key> <Event>` binds a key.
- **Timer** — `Every <ms> <Event>` fires on an interval (§6).
- **Focus** — the focused widget receives built-in keys (§5): a `List`/`Table`
  navigates with Up/Down and activates with Enter; an `Input` types.

`Quit` is a built-in handler that exits: `On Key "q" Quit`.

Event bodies are ordinary VBR — the same resolution pass a function body gets
(stdlib methods, string/numeric coercions, iterator chains, teaching
diagnostics), with the screen's state fields in scope. This is shared with the
GUI backend (`src/surface.rs`); a `Screen` event and a `Window` event lower
identically. *(BUILT — 2026-07-04.)*

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
defaults when unsized: containers/conditionals/scrollables/charts `Fill`, an
`Input` is 3 rows, `Text` is 1 row. A titled border frames the whole screen.

---

## 4. Widgets

### Text
`Text <expr>` — a line of text (`Paragraph`). Concatenate with `&`.

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

### Match / If in the view
Show different widgets by condition — identical to the GUI:

```vb
Match tab
    1 => Text "Overview"
    _ => Text "Settings"
End Match
```

---

## 5. Focus

`Input`, `List`, and `Table` are **focusable**. With more than one on screen,
**Tab** cycles focus, and the focused widget gets the relevant built-in keys:

- **Input** — printable keys type, Backspace deletes, Enter submits.
- **List/Table** — Up/Down move the selection, Enter selects.

Your own `On Key` bindings take precedence, so a globally-bound character key
can't also be typed into an input — with inputs, quit/act via `Esc` or a named
key.

Named keys for `On Key`: `Up`, `Down`, `Left`, `Right`, `Enter`, `Esc`, `Tab`,
`Space`, `Backspace`; otherwise a single character in quotes (`"q"`, `"+"`).

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
(infallible). V1 is one `Await` per event.

---

## 8. Running

`Function Main()` launches the screen with `<Screen>.Run`, just like a Window.
The generated `main` is a crossterm loop (`ratatui::init()` → draw → read key →
dispatch → `ratatui::restore()`); it takes over the terminal, so run it in a real
terminal (not piped), and it restores on exit. Adding a `Screen` pulls in
`ratatui` (crossterm comes with it); it builds far faster than the GUI's Iced.

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
`tui_list.vbr` / `tui_panels.vbr` (list + focus), `tui_table.vbr`,
`tui_input.vbr` (input + list), `tui_tabs.vbr` (Match/If), `tui_dashboard.vbr`
(Gauge/Sparkline/BarChart), `tui_chart.vbr` / `tui_multichart.vbr` (XY charts),
`tui_fetch.vbr` (async), `tui_monitor.vbr` (timers + async).
