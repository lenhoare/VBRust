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
End State
```

State fields are the authoritative data for the window.

Controls should not normally be mutated directly. Instead, controls display and update state.

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
- V1 awaits **known stdlib calls** (`Http.Get`); awaiting arbitrary user
  functions is a future addition.

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

#### Image

Displays an image from a file path, resource, or image handle.

```vb
Image "assets/logo.png"
Image profilePicture
```

Maps to Iced `image`.

V1 should support at least PNG and JPEG if supported by the backend configuration.

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

#### TextArea

Multiline text input bound to a string or text-buffer state field.

```vb
TextArea notes
TextArea notes Placeholder "Notes"
```

Maps to Iced `text_editor`.

`TextArea` is deliberately separate from `TextBox`. Although it feels like a multiline textbox, the backend widget and behaviour are different enough that it should be a distinct VBR control.

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

#### Toggle

Boolean switch control bound to a Boolean state field.

```vb
Toggle "Advanced mode", advancedMode
```

Maps to Iced `toggler`.

Used for on/off settings.

Although `CheckBox` and `Toggle` are both Boolean controls, both should exist because they express different UI intent.

---

#### RadioButton

Selects one value from a small fixed set.

```vb
RadioButton "Small", size, "Small"
RadioButton "Medium", size, "Medium"
RadioButton "Large", size, "Large"
```

Maps to Iced `radio`.

The bound state field should hold the selected value.

Radio buttons are recommended when all options should be visible at once.

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

#### ProgressBar

Displays progress through a task.

```vb
ProgressBar progress
ProgressBar progress From 0.0 To 1.0
ProgressBar percent From 0 To 100
```

Maps to Iced `progress_bar`.

Useful for file operations, compilation, downloads, AI inference, and long-running Rust tasks.

---

### 4.4 Drawing Controls

#### Canvas

An interactive drawing surface.

```vb
Canvas DrawingArea
    On Draw
        Circle 100, 100, 50
        Line 0, 0, mouseX, mouseY
    End Draw
End Canvas
```

Maps to Iced `canvas`.

Canvas is included in V1 because it gives VBR an immediate path to:

- simple graphics
- plots
- simulations
- visual demos
- educational examples
- games
- robot or sensor dashboards
- custom controls

Canvas should initially support a small 2D drawing API.

Recommended V1 drawing primitives:

```text
Line
Rectangle
Circle
Ellipse
Text
Image
Clear
```

Recommended V1 drawing properties:

```text
StrokeColor
FillColor
StrokeWidth
FontSize
```

Canvas may emit mouse events:

```vb
Canvas DrawingArea
    On MouseDown CanvasMouseDown
    On MouseMove CanvasMouseMove
    On MouseUp CanvasMouseUp
    On Draw DrawCanvas
End Canvas
```

Example:

```vb
Event CanvasMouseDown(x As Float, y As Float)
    lastX = x
    lastY = y
End Event
```

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

### 5.5 Space

Adds empty space.

```vb
Space Height 20
Space Width 10
```

Maps to Iced `space`.

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

V1 should support a small set of common layout properties.

Recommended properties:

```text
Width
Height
Padding
Spacing
Align
Center
```

Possible values:

```text
Fill
Shrink
Pixels(n)
```

Suggested syntax:

```vb
Column
    Spacing 10
    Padding 20

    Text "Settings"

    TextBox name
End Column
```

Container example:

```vb
Container
    Width Fill
    Height Fill
    Padding 20
    Center

    Text "Hello"
End Container
```

Absolute positioning is deliberately excluded from V1.

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

## 13. Example: Canvas

```vb
Window CanvasDemo

    Title = "Canvas Demo"

    State
        Dim mouseX As Float = 0
        Dim mouseY As Float = 0
        Dim isDrawing As Boolean = False
    End State

    View
        Column
            Text "Move the mouse over the canvas"

            Canvas DrawingArea
                Width Fill
                Height 300

                On Draw DrawCanvas
                On MouseMove CanvasMouseMove
                On MouseDown CanvasMouseDown
                On MouseUp CanvasMouseUp
            End Canvas
        End Column
    End View

    Event DrawCanvas
        Clear "White"
        Circle mouseX, mouseY, 20
        Text "Mouse: " & mouseX & ", " & mouseY, 10, 10
    End Event

    Event CanvasMouseMove(x As Float, y As Float)
        mouseX = x
        mouseY = y
    End Event

    Event CanvasMouseDown(x As Float, y As Float)
        isDrawing = True
    End Event

    Event CanvasMouseUp(x As Float, y As Float)
        isDrawing = False
    End Event

End Window
```

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
