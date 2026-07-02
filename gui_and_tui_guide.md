# Building Interfaces: Windows and Screens

VBR builds two kinds of interactive program from the *same* set of ideas:

- a **`Window`** — a desktop application with buttons, sliders, and text boxes,
  drawn with [Iced];
- a **`Screen`** — a keyboard-driven terminal application with lists, tables, and
  charts, drawn with [ratatui].

This is a friendly tour. The terse, complete catalogues live in `gui_spec.md` and
`tui_spec.md`; here we just want to get the *shape* of things into your head. The
good news is that there is really only one shape to learn.

[Iced]: https://iced.rs
[ratatui]: https://ratatui.rs

---

## 1. One idea, two faces

If you built forms in VB6, you know the old rhythm: you drop controls on a form,
and each control's event handler reaches out and *pokes* other controls —
`Label1.Caption = CStr(n)`. The screen is the truth, and your code scrambles to
keep it consistent.

VBR turns that inside out. There is a single blob of **state** that is the truth,
a **view** that is *computed* from that state, and **events** that are the only
things allowed to change the state. When the state changes, the whole view is
redrawn from scratch. You never poke a widget; you change a number and describe
what the screen looks like for that number. (This pattern has a name — *The Elm
Architecture* — and it is why modern UIs are so much calmer than VB6 forms.)

Three parts, then: **State, View, Events.** Here is the whole of a counter, as a
desktop window:

```vb
Window Counter
    Title "Counter"

    State
        Dim count As Long = 0
    End State

    View
        Column
            Text "Counter"
            Text count
            Button "-"
                On Click Decrement
            End Button
            Button "+"
                On Click Increment
            End Button
        End Column
    End View

    Event Increment
        count += 1
    End Event

    Event Decrement
        count -= 1
    End Event
End Window

Function Main()
    Counter.Run
End Function
```

Read it top to bottom and the three parts announce themselves.

---

## 2. The three parts

**State** is a little block of `Dim`s — exactly the declarations you already know,
just gathered inside `State … End State`. It can hold primitives, enums, and
`Vec<T>` collections (a growing list you fill in an event and show in the view).
It is the *only* memory the program has; everything on screen is derived from it.

**View** is a tree of widgets wrapped in `View … End View`. Crucially, it is a
*picture of the state*, not a set of objects you keep and mutate. `Text count`
doesn't mean "make a label and remember it" — it means "wherever this sits, show
the current value of `count`." Change `count` and this text follows, for free,
because the whole view is recomputed. Widgets nest inside `Column` (stacked
vertically) and `Row` (side by side).

**Events** are handlers, written `Event Name … End Event`, and they are the only
place state may change. A button's `On Click Increment` says "when this is
clicked, run the `Increment` event." Inside, you just assign to state fields as if
they were ordinary variables — `count += 1` — and VBR arranges the redraw. An
event can take a parameter when the widget carries data (you'll see `On Change`
below hand a slider's new value to its handler).

That is the entire mental model. Everything else is *which widgets exist* and
*how they're arranged* — and that is where a Window and a Screen part ways.

---

## 3. Windows — the desktop GUI

A `Window` compiles to an Iced application: a real resizable window with themed,
mouse-driven controls.

### Widgets

Beyond `Text` and `Button`, a window has the controls you'd expect — text inputs,
checkboxes, togglers, sliders, progress bars, radio groups, and images. Each
event-producing control names the event it fires. A slider is typical:

```vb
Slider 0..=50, input
    On Change SetInput
End Slider
```

`0..=50` is the range and `input` is the state field it reflects; as you drag, it
fires `SetInput` with the new value:

```vb
Event SetInput(value As Integer)
    input = value
End Event
```

The full list of controls (with each one's event) is in `gui_spec.md`.

### Layout and sizing

`Column` and `Row` do the arranging. `Spacing` puts a gap between children and
`Padding` insets the whole group:

```vb
Column
    Spacing 12
    Padding 20
    Text "Count"
    Text count
End Column
```

By default each child takes just the room it needs. To take charge, put a **size
line before a child**: `Length 40` gives it exactly 40 pixels along the container's
main axis, and `Fill` lets it soak up whatever is left:

```vb
Column
    Length 40
    Text "Header — fixed 40px tall"
    Fill
    Text "Body fills the remaining space"
    Length 30
    Button "Footer button"
        On Click Bump
    End Button
End Column
```

### Themes

One line reskins everything. `Theme Dracula` under the `Title` restyles the whole
window, because Iced themes cascade to every widget — no per-control work:

```vb
Window Counter
    Title "Dracula Counter"
    Theme Dracula
    ...
```

---

## 4. Screens — the terminal TUI

A `Screen` compiles to a [ratatui] application: it takes over the terminal, draws
a text interface, and is driven by the **keyboard**. The State/View/Events core is
identical — so much of §2 carries straight over — but the vocabulary is honest to
a terminal.

### The keyboard is the mouse

There is no clicking, so a `Screen` binds keys to events with `On Key`:

```vb
On Key "+" Increment
On Key "-" Decrement
On Key "q" Quit
```

`Quit` is built in and exits. Named keys — `Up`, `Down`, `Enter`, `Esc`, `Tab`,
`Space`, `Backspace` — are written without quotes (`On Key Esc Quit`); a plain
character goes in quotes.

### Widgets, including data-viz

A screen has text, a single-line `Input` (fires `On Submit` on Enter), a
selectable `List` and `Table` (fire `On Select`), and — because dashboards are a
natural terminal job — first-class charts: `Gauge`, `Sparkline`, `BarChart`, and a
full XY `Chart`. A dashboard is just the usual `Column` of them:

```vb
View
    Column
        Spacing 1
        Gauge 0..=100, cpu
        Length 8
        Sparkline history
        Fill
        BarChart sales
    End Column
End View
```

Layout works as it does in a window — `Column`/`Row` with `Length`/`Fill` — plus
`Percent` and `Min`, since a terminal splits an area into rectangles rather than
laying out pixels.

### Focus

When more than one input/list/table is on screen, **Tab** moves focus between
them, and the focused widget gets the relevant keys: an `Input` takes typing and
Backspace; a `List`/`Table` moves its highlight with Up/Down and activates with
Enter. Here a note-taker wires an input to a list:

```vb
Input entry
    On Submit Add
End Input
...
List notes
    On Select Pick
End List
```

```vb
Event Add(text As String)      ' Enter in the box hands over the typed text
    notes.Push(text)
    entry = ""
End Event
```

### Timers

A terminal app often needs to tick along on its own. `Every <ms> <Event>` fires a
handler on an interval — a clock, an animation, a periodic refresh:

```vb
Every 1000 Tick          ' once a second
Every 5000 Poll          ' every five seconds

Event Tick
    seconds += 1
End Event
```

The full widget and key catalogue is in `tui_spec.md`.

---

## 5. Slow work without freezing — `Await`

Whether a window or a screen, one rule holds: never do slow work (a network
request, a heavy computation) directly in an event, or the interface locks up
until it finishes. The tool for this is **`Await`**, used inside an event. The
slow call runs off to one side, and when its result lands the interface updates:

```vb
Event Poll
    status = "refreshing…"
    Match Await Http.Get(url)
        Ok(body) => status = "ok, " & body.Len() & " bytes"
        Err(e)   => status = "error: " & e
    End Match
End Event
```

Notice the shape: set a "working…" state *now* (the view redraws immediately, so
the user sees it), then `Match` on the awaited result to fold the outcome back
into state. It reads like ordinary sequential code, but the interface stays live
throughout. This works for your own functions too, not just the stdlib — anything
that returns a value. (Combined with a timer, `Every 5000 Poll` gives you a
self-refreshing dashboard for almost nothing.)

---

## 6. Running it

Both kinds start the same way — call `.Run` on the app inside `Main`:

```vb
Function Main()
    Counter.Run       ' a Window opens a desktop window
    Notes.Run         ' a Screen takes over the terminal
End Function
```

A `Window` opens a window and returns when you close it. A `Screen` takes over the
terminal and restores it on exit, so run it in a real terminal, not a pipe.

---

## Which one?

Reach for a **`Window`** when you want a conventional desktop app — mouse, themes,
free-form layout. Reach for a **`Screen`** when you live in the terminal, want
something keyboard-fast, or are building a dashboard or tool that belongs next to
your other command-line work. They share a core and can live in the same project,
so the knowledge transfers completely — learn `State`/`View`/`Event` once, and you
can build either.
