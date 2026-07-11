# VBR GUI Specification

Status: Draft V0.1  
Target backend: Iced  
Purpose: Define the first GUI model for VBR using a VB-like surface syntax and a modern state/message/view architecture.

---

## 1. Design Goals

The VBR GUI system should provide a simple, productive way to build desktop applications while preserving VBR's core philosophy:

- VB-like approachability.
- Rust-powered implementation.
- Clean, explicit program structure.
- Good compilation target for Iced.
- Easy generation by AI agents.
- Avoidance of fragile hidden GUI state.

The GUI system should feel familiar to a Visual Basic programmer, but it should not reproduce the old mutable-control-object model internally.

Instead, VBR GUI programs use:

```text
State
View
Events
```

The application state is the source of truth.  
The view is derived from the state.  
Events update the state.

---

## 2. Conceptual Model
cargo run -- run examples/hello.vbr
A VBR GUI window consists of:

```text
Window
    State
    View
    Events
End Window
```

### 2.1 State

The `State` block declares the persistent data belonging to a window.

Example:

```vb
State
    Dim count As Integer = 0
    Dim name As String = ""
    Dim enabled As Boolean = False
    Dim points As Vec<Point>       ' a collection — may start empty
End State
```

State fields are the authoritative data for the window. A field may be a
primitive, an enum, a `TextArea` (a multi-line editor), or a **`Vec<T>`
collection** (fill it in an event and iterate it in the view — the basis for
charts/plots). `Map` and fixed arrays are not yet supported as state fields.

Controls should not normally be mutated directly. Instead, controls display and update state.

**Fallible initialisers.** A field's initialiser may be a *fallible* call — a
stdlib constructor (`Database.Open`, `Json.Parse`, `DateTime.Parse`,
`FileSystem.Read`/`ReadLines`) or one of your own `Result`-returning functions:

```vb
State
    Dim db As Database = Database.Open("ideas.db")
End State
```

Construction then happens **before the window opens**, in a generated
`init() -> Result<State, String>`: on failure the program prints
`could not start: <why>` and exits — the app never launches with broken state.
Events use the ready value (`db` is `state.db`); passing it to one of your
functions borrows it (`&Database`). This is native-only — a browser `Page` or
`Screen` has no startup moment to fail in, and gets a teaching error.

More generally, an initialiser is **ordinary VBR** — it runs the same
resolution pass as a function-body `Dim`, so calling your own functions works
with full argument treatment (`Dim living As Long = CountLive(SeedGrid())`
borrows the ByVal `Vec` exactly as anywhere else).

**Multi-file projects.** A `Window` (like a `Screen`) joins a project: UI in
`main.vbr`, logic in sibling modules, called qualified from State
initialisers, events, and helper functions with the full cross-module
argument treatment (`projects_and_run_spec.md`). A sibling's `Public
Type`/`Enum` is used by its bare name — State can hold one, events can call
its methods. A *view* expression can't read a module constant directly (views
don't run the resolver) — mirror it into state or read it through a helper.

### 2.2 View

The `View` block declares the visible widget tree.

Example:

```vb
View
    Column
        Text "Counter"
        Text count

        Button "+"
            On Click Increment
        End Button
    End Column
End View
```

The view is a declarative description of the interface. It is regenerated when state changes.

The view may branch on state with **`Match`** *(BUILT — slice 2)* — the same
`Match` as the rest of the language, but each arm produces the widget(s) to show:

```vb
View
    Column
        TextInput "type here", name
            On Input Rename
        End TextInput
        Match name
            "" => Text "Type your name above."
            _  => Text "Hello, " & name & "!"
        End Match
    End Column
End View
```

Each arm yields one widget (or several wrapped in an implicit `Column`); a
`String` is matched as text, so `""` / literal patterns work.

`If … Then … [ElseIf …] [Else …] End If` *(BUILT — slice 5)* does the same by
condition rather than by pattern. `ElseIf`/`Else` are optional; an `If` with no
`Else` shows nothing when false:

```vb
View
    Column
        If level >= 10 Then
            Text "High!"
        Else
            Text "OK"
        End If
    End Column
End View
```

### 2.3 Events

The `Event` blocks define how state changes in response to user actions.

```vb
Event Increment
    count += 1
End Event
```

An event may take **parameters** *(BUILT — slice 2)*, which carry data from the
widget (e.g. a `TextInput`'s new text). The event becomes a message variant
holding that data, and the body binds it:

```vb
Event Rename(value As String)   ' → Message::Rename(String)
    name = value
End Event
```

Widgets do not directly call arbitrary procedures. They emit events/messages, and the window handles them.

An event (or the view) **may call your own top-level `Function`/`Sub`s**
*(BUILT — slice 10)* — they're emitted alongside the window, so helper logic
(validation, formatting, computation) lives in a procedure rather than being
inlined. (V1: call sites in events aren't resolved, so pass values rather than
`ByRef`/struct args; helpers should avoid stdlib/HashMap for now — imports aren't
unified across the GUI yet.)

#### Async events — `Await` *(BUILT — slice 4)*

Slow work (an HTTP request, later a timer) must not run on the UI thread or the
window freezes. `Await` runs it off-thread and resumes the handler when it
finishes:

```vb
Event Fetch
    status = "loading…"
    Match Await Http.Get(url)
        Ok(body) => status = "got " & body.len() & " bytes"
        Err(e)   => status = "error: " & e
    End Match
End Event
```

Under the hood the event is **split in two**: a kick-off (runs the code before the
`Await`, then hands Iced a `Task` that does the work) and an auto-generated
continuation message (`FetchDone(result)`) that runs the code after it. The
blocking stdlib call runs via `tokio::task::spawn_blocking`, and state used in
the awaited call is snapshotted (cloned) into the task.

Rules:
- **Fallible** async (`Http.Get` returns a `Result`) **must** be handled with
  `Match Await …` — a GUI must not crash on a failed request. **Infallible**
  async can use `Dim x As T = Await …`.
- **One `Await` per event** (V1). Multiple/looped awaits are a future state machine.
- `Await` works on a **known stdlib call** (`Http.Get`) **or one of your own
  functions** (its return type is known, so the result message is generated;
  it runs off-thread via `spawn_blocking`).
- Calling a blocking stdlib call in an event **without** `Await` is a hard error
  (✘ "would freeze the window — use `Await`"), so the trap is caught at compile time.

---

## 3. Backend Mapping

The VBR GUI model is intended to compile naturally to Iced.

A VBR window maps approximately to:

```rust
struct WindowState {
    // VBR State fields
}

enum Message {
    // VBR Events and generated control messages
}

fn update(state: &mut WindowState, message: Message) {
    // VBR Event bodies
}

fn view(state: &WindowState) -> Element<Message> {
    // VBR View tree
}
```

The compiler may generate additional internal messages for bound controls such as text boxes, sliders, toggles, and choosers.

---

## 4. V1 Control Set

The V1 GUI library should include the following controls.

**Naming principle:** controls take their **Iced names**, not VB history —
`TextInput` (not `TextBox`), `Checkbox`, `Slider`, etc. VBR is a stepping stone
to Rust, so the names you learn here are the ones you'll meet in real Iced code.
(Where this spec still shows older VB-flavoured names for unbuilt controls, those
will be renamed to their Iced equivalent when built.)

### 4.1 Display Controls

#### Text

Displays static text or a value from state.

```vb
Text "Hello"
Text count
Text "Name: " & name
```

Maps to Iced `text`.

---

#### Image  *(BUILT — slice 11)*

Displays an image from a file path (a literal, or a `String` state field).

```vb
Image "assets/logo.png"
Image profilePicture
```

Maps to `iced::widget::image(...)` (a `String` field is cloned to own the path).
Using `Image` auto-adds Iced's **`image`** feature to the project (so PNG/JPEG
decode). A display-only window (no events) still compiles warning-free.

---

### 4.2 Input Controls

#### TextInput  *(BUILT — slice 2)*

Single-line text input bound to a `String` state field. The placeholder comes
first, then the bound field; an `On Input` clause names the event fired on each
keystroke, which receives the new text as a payload.

```vb
TextInput "Enter your name", name
    On Input Rename
End TextInput

Event Rename(value As String)
    name = value
End Event
```

Maps to Iced `text_input("Enter your name", &state.name).on_input(Message::Rename)`.

The binding is **explicit** by design: the event makes the message-passing
mechanism visible (the point of a teaching tool) and gives you a place to
validate or transform the input rather than only store it. (A future
auto-binding shorthand could synthesise the trivial "just store it" event, but
the explicit form is the foundation.)

---

#### TextArea  *(BUILT — slice 7)*

A multi-line text editor (Iced `text_editor`). Unlike `TextInput`, it's
**stateful** — the bound field is declared `As TextArea` and holds an editor
buffer (`text_editor::Content`), not a `String`:

```vb
' State: Dim notes As TextArea = "initial text"
TextArea notes
```

Maps to `text_editor(&state.notes).on_action(Message::NotesEdited)`. The edit
handler is **generated automatically** (`state.notes.perform(action)`) — you
don't write it. Read the typed text with **`.Text()`**: `notes.Text()` →
`state.notes.text()` (e.g. `notes.Text().len()`).

`TextArea` is deliberately separate from `TextInput`: the backend widget is
stateful and behaves differently enough to be its own control.

---

#### Button

A push button that emits an event when clicked.

```vb
Button "Save"
    On Click SaveClicked
End Button
```

Example event:

```vb
Event SaveClicked
    SaveFile()
End Event
```

Maps to Iced `button`.

The compiler may later allow shorthand syntax:

```vb
Button "Save"
    SaveFile()
End Button
```

This should be treated as syntactic sugar for an automatically generated event.

---

#### Checkbox  *(BUILT — slice 3)*

Boolean control bound to a `Boolean` state field. Like `TextInput`, the binding
is explicit: an `On Toggle` clause names the event fired when it's ticked, which
receives the new `bool`.

```vb
Checkbox "Remember me", remember_me
    On Toggle SetRemember
End Checkbox

Event SetRemember(value As Boolean)
    remember_me = value
End Event
```

Maps to `checkbox("Remember me", state.remember_me).on_toggle(Message::SetRemember)`.

---

#### Slider  *(BUILT — slice 3)*

A draggable value over an inclusive range `min..=max`, bound to a numeric state
field. Iced sliders always report movement, so `On Change` is **required**.

```vb
Slider 0..=100, volume
    On Change SetVolume
End Slider

Event SetVolume(value As Integer)
    volume = value
End Event
```

Maps to `slider(0..=100, state.volume, Message::SetVolume)`.

The bound field must be a type Iced can convert to `f64` — `Integer`, `Single`,
`Double`, or `Byte`. A `Long` (i64) is rejected with a friendly error.

---

#### Toggler  *(BUILT — slice 6)*

On/off switch bound to a `Boolean` state field — like `Checkbox`, but a switch.
`On Toggle` fires with the new `bool`.

```vb
Toggler "Advanced mode", advanced_mode
    On Toggle SetAdvanced
End Toggler

Event SetAdvanced(value As Boolean)
    advanced_mode = value
End Event
```

Maps to `toggler(state.advanced_mode).label("Advanced mode").on_toggle(Message::SetAdvanced)`.
Binding to a non-`Boolean` field is a friendly error. `Checkbox` and `Toggler`
are both boolean controls — pick by the UI intent.

---

#### Radio  *(BUILT — slice 6)*

Selects one value from a small fixed set. Each `Radio` offers one option;
`On Select` fires with the chosen value. The bound field holds the selection.

```vb
Enum Size
    Small
    Medium
    Large
End Enum
' …State: Dim choice As Size = Size.Small …

Radio "Small", choice, Size.Small
    On Select Pick
End Radio
Radio "Medium", choice, Size.Medium
    On Select Pick
End Radio

Event Pick(value As Size)
    choice = value
End Event
```

Maps to `radio("Small", Size::Small, Some(state.choice), Message::Pick)`. Iced's
`radio` needs the value to be **`Copy + Eq`**, so the bound field must be an
**enum** (the natural fit) or an **integer** — `String` (not `Copy`) and float
(not `Eq`) values are rejected with a friendly error. This is why enums and Radio
came together.

---

#### Slider

Numeric input over a range.

```vb
Slider volume From 0 To 100
Slider opacity From 0.0 To 1.0 Step 0.01
```

Maps to Iced `slider`.

The bound state field may be integer or floating-point, depending on the declared state type and range.

---

#### Chooser

Dropdown selection from a list.

```vb
Chooser mode From ["Simple", "Advanced", "Expert"]
Chooser selectedName From names
```

Maps to Iced `pick_list`.

A searchable chooser may later map to Iced `combo_box`, but `Chooser` V1 should mean a simple dropdown selection.

V1 does not include a VB-style permanently open listbox, but we should add it to the next version.

---

### 4.3 Feedback Controls

#### ProgressBar  *(BUILT — slice 6)*

A read-only gauge showing a numeric state field over an inclusive range. No
events (display only), so it's a single line — `ProgressBar min..=max, field`.

```vb
ProgressBar 0..=100, level
```

Maps to `progress_bar((0 as f32)..=(100 as f32), state.level as f32)` (Iced
progress bars are `f32`, so the bounds and value are cast). Binding to a
non-numeric field is a friendly error.

Useful for file operations, compilation, downloads, AI inference, and long-running Rust tasks.

---

### 4.4 Drawing Controls

#### Canvas  *(BUILT — slice 12; drawing only)*

A 2-D drawing surface — the closest thing to a VB6 `PictureBox`. It maps to
Iced's `canvas::Program`. A canvas is defined **at the top level** with a `Draw`
block, and **placed in a view** with `Canvas <Name>`:

```vb
Window Sketch
    State
        Dim radius As Integer = 40
    End State
    View
        Column
            Slider 10..=120, radius
                On Change Resize
            End Slider
            Canvas Face Width 300 Height 220
        End Column
    End View
    Event Resize(value As Integer)
        radius = value
    End Event
End Window

Canvas Face
    Draw
        DrawGrid()
        Fill Circle(150, 110, radius), Color.Navy
        Stroke Circle(150, 110, radius), Color.White, 2
        Text "radius = " & radius, 10, 16, Color.Black
    End Draw
End Canvas
```

**The mental model (important).** Iced draws *from state*, not paint-on-demand.
The `Draw` block runs on every repaint and describes the whole picture as a
function of the current state — you never poke pixels from an event. To change
what's drawn, change the **state** (e.g. in an event); the canvas repaints
itself. This is the one real shift from VB6's immediate `Picture1.Line …`.

**Reads state.** The `Draw` block may reference the hosting window's state
fields directly (`radius` above); those fields are snapshotted into the canvas
each frame.

**Data-driven drawing (charts/plots).** State fields may be **`Vec` collections**
(`Dim bars As Vec<Bar>`, which may start empty), so a canvas can plot a dataset.
Fill the `Vec` in an event (`bars = MakeBars(seed)` / `bars.Push(...)`) and iterate
it in `Draw` with `For Each`; change the data and the chart repaints. See
`examples/plot.vbr`:

```vb
State
    Dim bars As Vec<Bar> = MakeBars(3)
End State
...
Canvas Plot
    Draw
        For Each b In bars
            Fill Rect(b.x, 180 - b.h, 18, b.h), Color.Navy
        Next b
    End Draw
End Canvas
```

> Note: canvas bodies don't run the resolver, so iterate with **`For Each`**
> (which borrows correctly) rather than iterator adapters (`.map`/`.filter`)
> or `.Count()`-style index loops. Compute heavy data in an *event* (off-thread
> with `Await` if slow) and store it in state; keep `Draw` to rendering.

**Drawing verbs** (valid in a `Draw` block or a paint function):

```text
Fill   <shape>, <color>
Stroke <shape>, <color>[, <width>]     ' width default 1
Text   <string>, <x>, <y>[, <color>]   ' color default Black
```

**Shapes:**

```text
Circle(cx, cy, radius)
Rect(x, y, width, height)
Line(x1, y1, x2, y2)                    ' Stroke only — a Line has no area
```

**Colors:** a named `Color.Red` (Black, White, Red, Green, Blue, Gray, Yellow,
Orange, Purple, Navy, Cyan, Magenta) or an explicit `Color(r, g, b)` (0–255).

**Paint functions.** You can factor drawing into ordinary functions the `Draw`
block calls — a function that draws (or calls one that does) automatically
receives the `frame`, so it can only be called from a `Draw` block or another
paint function:

```vb
Function DrawGrid()
    For x = 0 To 300 Step 30
        Stroke Line(x, 0, x, 220), Color.Gray, 1
    Next x
End Function
```

Placing a `Canvas` auto-adds Iced's **`canvas`** feature to the project.

**Deferred:** interaction (mouse/keyboard → messages), gradients, per-shape
transforms, `Clear`/caching. Those are the `Program::update` half of Iced's
canvas — a different event model — and are the next step if canvases grow up.

---

## 5. Layout Controls

Layout is not optional. VBR GUI should avoid absolute positioning in V1.

The V1 layout controls are:

```text
Row
Column
Container
Scrollable
Space
Rule
```

### 5.1 Column

Vertical layout.

```vb
Column
    Text "Title"
    TextBox name
    Button "OK"
        On Click OkClicked
    End Button
End Column
```

Maps to Iced `column`.

---

### 5.2 Row

Horizontal layout.

```vb
Row
    Button "Back"
        On Click Back
    End Button

    Button "Next"
        On Click Next
    End Button
End Row
```

Maps to Iced `row`.

---

### 5.3 Container

Wraps a single child and applies layout properties such as padding, width, height, and alignment.

```vb
Container
    Padding 20
    Width Fill

    Text "Hello"
End Container
```

Maps to Iced `container`.

---

### 5.4 Scrollable

Allows child content to scroll.

```vb
Scrollable
    Column
        For Each item In items
            Text item
        Next
    End Column
End Scrollable
```

Maps to Iced `scrollable`.

---

### 5.5 Space  *(BUILT — slice 8)*

Adds a one-off blank gap.

```vb
Space Height 20
Space Width 10
```

Maps to `iced::widget::Space::with_height(20)` / `::with_width(10)`.

---

### 5.6 Rule

Displays a horizontal or vertical separator.

```vb
Rule Horizontal
Rule Vertical
```

Maps to Iced `rule`.

---

## 6. Common Layout Properties

**`Spacing` and `Padding` are BUILT (slice 8).** Inside a `Column`/`Row`, a
`Spacing N` line puts a uniform `N`-pixel gap between every child, and `Padding N`
insets the whole container:

```vb
Column
    Spacing 12
    Padding 20
    Text "Settings"
    Button "OK"
        On Click Ok
    End Button
End Column
```

→ `column![…].spacing(12).padding(20)`.

### Child sizing  *(BUILT — 2026-07-03)*

A **size line before a child** sizes that child along the container's **main
axis** — height in a `Column`, width in a `Row` — the same syntax the TUI uses:

```vb
Column
    Length 40
    Text "Header — fixed 40px tall"
    Fill
    Text "Body fills the remaining space"
    Length 30
    Button "Footer"
        On Click Save
    End Button
End Column
```

- **`Length N`** → a fixed `N` pixels (`iced::Length::Fixed`).
- **`Fill`** / **`Fill N`** → fill the leftover space, weighted by `N`
  (`iced::Length::Fill` / `FillPortion(N)`).

Each sized child is wrapped in an Iced `container` with the length applied on the
main axis. `Percent`/`Min` are **Screen (TUI) only** — the GUI reports a friendly
error and asks for `Length`/`Fill`.

**Not yet built:** cross-axis sizing (e.g. "fill the column's width"), container
`Align`/`Center` alignment, and `Shrink`. Absolute positioning is deliberately
excluded.

---

## 7. Binding Rules

Controls may bind directly to state fields.

Examples:

```vb
TextBox name
CheckBox "Enabled", enabled
Slider volume From 0 To 100
Chooser mode From modes
```

The compiler generates the necessary backend messages to update the state.

A bound control must be bound to a compatible state field.

Examples:

```text
TextBox     -> String
TextArea    -> String or text buffer
CheckBox    -> Boolean
Toggle      -> Boolean
Slider      -> Integer or Float
Chooser     -> value matching option type
RadioButton -> value matching option type
ProgressBar -> Integer or Float
```

Invalid bindings should be compile-time errors where possible.

---

## 8. Events

Events are named handlers within a window.

```vb
Event SaveClicked
    SaveFile()
End Event
```

Events may receive parameters.

```vb
Event CanvasMouseDown(x As Float, y As Float)
    lastX = x
    lastY = y
End Event
```

Events may update window state.

Events may call normal VBR procedures.

Events may call inline Rust if allowed elsewhere in the language.

An event body is ordinary VBR — it runs the same resolution pass as a function
body, with the window's state fields and the event's parameters in scope. So
stdlib methods (`now.AddDays(30)`), string/numeric coercions, iterator chains
(`nums.Iter().Sum()`), and the usual teaching diagnostics all work inside an
event exactly as they do in a function. *(BUILT — 2026-07-04.)*

---

## 9. Generated Messages

Internally, every event maps to a backend message.

Example VBR:

```vb
Button "Increment"
    On Click Increment
End Button

Event Increment
    count += 1
End Event
```

Conceptual Rust:

```rust
enum Message {
    Increment,
}

fn update(state: &mut AppState, message: Message) {
    match message {
        Message::Increment => {
            state.count += 1;
        }
    }
}
```

For bound controls, messages may be generated automatically.

Example VBR:

```vb
TextBox name
```

Conceptual Rust:

```rust
enum Message {
    NameChanged(String),
}
```

The generated update logic sets:

```rust
state.name = value;
```

---

## 10. Window Syntax

A window may set a built-in **`Theme`** *(BUILT — slice 9)* — one of Iced's
~20 palettes (`Dark`, `Light`, `Dracula`, `Nord`, `GruvboxDark`,
`CatppuccinMocha`, `TokyoNight`, …). It restyles the **whole** window — Iced
themes cascade to every widget, so there's no per-control styling:

```vb
Window Counter
    Title "Counter"
    Theme Dracula
    …
End Window
```

→ `iced::application(…, update, view).theme(|_| iced::Theme::Dracula).run()`. An
unknown name is a friendly error listing the built-ins. (Loading a *custom*
theme from a document — a small palette of colours — is a future low-touch
addition; per-widget styling is intentionally out of scope.)

A complete window has this structure:

```vb
Window MainWindow

    Title = "Example"

    State
        Dim name As String = ""
        Dim enabled As Boolean = False
    End State

    View
        Column
            Text "Example"

            TextBox name Placeholder "Name"

            CheckBox "Enabled", enabled

            Button "Run"
                On Click RunClicked
            End Button
        End Column
    End View

    Event RunClicked
        Debug.Print "Running for " & name
    End Event

End Window
```

The project entry point may specify which window to run.

Possible syntax:

```vb
Run Window MainWindow
```

or:

```vb
Application
    MainWindow = MainWindow
End Application
```

The exact application-level syntax is deferred.

---

## 11. Example: Counter

```vb
Window CounterWindow

    Title = "Counter"

    State
        Dim count As Integer = 0
    End State

    View
        Column
            Spacing 10
            Padding 20

            Text "Counter"
            Text count

            Row
                Spacing 10

                Button "-"
                    On Click Decrement
                End Button

                Button "+"
                    On Click Increment
                End Button
            End Row
        End Column
    End View

    Event Increment
        count += 1
    End Event

    Event Decrement
        count -= 1
    End Event

End Window
```

---

## 12. Example: Settings Form

```vb
Window SettingsWindow

    Title = "Settings"

    State
        Dim userName As String = ""
        Dim notes As String = ""
        Dim rememberMe As Boolean = False
        Dim advancedMode As Boolean = False
        Dim volume As Integer = 50
        Dim mode As String = "Simple"
        Dim progress As Float = 0.0
    End State

    View
        Column
            Padding 20
            Spacing 12

            Text "Settings"

            TextBox userName Placeholder "User name"

            TextArea notes Placeholder "Notes"

            CheckBox "Remember me", rememberMe

            Toggle "Advanced mode", advancedMode

            Slider volume From 0 To 100

            Chooser mode From ["Simple", "Advanced", "Expert"]

            ProgressBar progress From 0.0 To 1.0

            Button "Run"
                On Click RunClicked
            End Button
        End Column
    End View

    Event RunClicked
        progress = 0.0
        Debug.Print "Running"
    End Event

End Window
```

---

## 13. Example: Canvas  *(BUILT — see `examples/canvas.vbr`)*

A slider resizes a circle drawn on a canvas over a grid drawn by a paint
function. Drawing is state-driven: the event changes `radius`, and the canvas
repaints from it.

```vb
Window Sketch
    Title "Canvas"

    State
        Dim radius As Integer = 40
    End State

    View
        Column
            Spacing 10
            Padding 10
            Text "Drag the slider to resize the circle"
            Slider 10..=120, radius
                On Change Resize
            End Slider
            Canvas Face Width 300 Height 220
        End Column
    End View

    Event Resize(value As Integer)
        radius = value
    End Event
End Window

Canvas Face
    Draw
        DrawGrid()
        Fill Circle(150, 110, radius), Color.Navy
        Stroke Circle(150, 110, radius), Color.White, 2
        Text "radius = " & radius, 10, 16, Color.Black
    End Draw
End Canvas

Function DrawGrid()
    For x = 0 To 300 Step 30
        Stroke Line(x, 0, x, 220), Color.Gray, 1
    Next x
    For y = 0 To 220 Step 30
        Stroke Line(0, y, 300, y), Color.Gray, 1
    Next y
End Function

Function Main()
    Sketch.Run
End Function
```

> **Note.** Mouse/keyboard *interaction* (the `On MouseMove` / `On Draw` event
> style once sketched here) is **deferred** — see §4.4. V1 canvases are
> drawing-only and repaint from state.

---

## 14. Deferred Features

The following are deliberately not part of V1:

```text
Absolute positioning
VB-style ListBox
TreeView
DataGrid/Table
Menus
Toolbars
Tabs
Docking layouts
MDI
Native OS widget fidelity
Drag-and-drop form designer
Advanced styling/themes
Accessibility annotations beyond backend defaults
```

Some of these may be added later. They should not block V1.

---

## 15. Future Features

Potential V1.5 or V2 controls:

```text
List
Markdown
Svg
Combobox
MenuBar
Toolbar
Tooltip
Table
Tabs
VerticalSlider
FilePicker
ColorPicker
DatePicker

Way down the line, maybe
CustomControl
```

Some of these map to existing Iced widgets or common extension patterns, but they are not required for the first usable VBR GUI layer.

---

## 16. Implementation Notes

Maybe compiler should lower GUI files into an intermediate GUI representation before generating Rust/Iced code.

Possible internal representation to discuss.

```text
GuiModule
    Windows
        Window
            StateFields
            ViewTree
            Events
            Bindings
```

This avoids coupling the VBR syntax directly to Iced and leaves open the possibility of future backends.

The Iced backend should be the first supported backend.

> *(BUILT — 2026-07-04.)* The State/View/Events machinery is now a shared core
> (`src/surface.rs`): the program prologue, state maps, event-body resolution
> and lowering, `Await` splitting, and blocking-call checks are one
> implementation used by both the GUI (Iced) and TUI (ratatui) emitters, which
> remain view renderers plus a runtime shell. A future backend (e.g. a web
> `Page`) would be a third renderer over the same core, not a third copy.

Inline Rust may be supported inside events...

---

## 17. Summary

VBR GUI V1 should provide:

```text
Text
TextBox
TextArea
Button
Image
CheckBox
Toggle
RadioButton
Slider
Chooser
ProgressBar
Canvas

Row
Column
Container
Scrollable
Space
Rule
```

The central rule is:

```text
State is truth.
View displays state.
Events change state.
```

This gives VBR a GUI system that feels approachable like classic Visual Basic, but compiles cleanly to a modern Rust/Iced architecture.
