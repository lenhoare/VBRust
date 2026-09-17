# Vinyl (`.vbr`) — compact context (console + TUI)

Console programs, terminal **Screens**, and the standard library. **Do not** write `Window`, `Page`, `Sketch`, `Node2D`, `Gpu Draw`, `TextInput`, or `On Click` as the way to drive a Screen — a Screen is keyboard-first.

Vinyl looks like VB. It compiles to Rust. Where VB habit and Rust collide, **Rust wins**: every value has a static type, arrays are zero-based, errors are values (not `On Error`).

A TUI is **not** VB6 forms. You do not poke controls. **State** is the truth, **View** is a picture of that state, **Events** are the only place state changes. Change a field; the view redraws. Input is keys, not the mouse.

**`Debug.Print` on a Screen is a compile error** — the Screen owns the terminal. Use `Log "msg"` (writes `vbr.log`). Watch it with `tail -f build/vbr.log`.

---

## Build and run

From the Vinyl repo (builds `vbr` as needed):

```sh
cargo run -- run FILE.vbr              # no stdlib, no Screen, no extra crates
cargo run -- runproject FILE.vbr       # Screen, stdlib, Use, or a folder of .vbr files
cargo run -- runproject DIR
cargo run -- test FILE.vbr             # run Test blocks
cargo run -- emit FILE.vbr             # print the generated Rust
```

Once built, `target/debug/vbr` is the same CLI (`vbr run …`, `vbr runproject …`).

**Rule:** `run` is only for a self-contained console program. A **`Screen` always needs `runproject`** (it pulls in ratatui). Anything that names `FileSystem`, `Json`, `Http`, `DateTime`, `Regex`, `Database`, `Shell`, `DataFrame`, or `Use` also needs `runproject`.

`runproject` writes a Cargo project under `build/` and runs it. You never edit `Cargo.toml`. Run a Screen in a real terminal (not piped).

---

## Two program shapes

**Console** — `Function Main()` is the program:

```vb
Function Main()
    Debug.Print "hello"
End Function
```

**Screen** — declare the screen, then launch it from `Main`:

```vb
Screen Counter
    Title "Counter"

    State
        Dim count As Integer = 0
    End State

    Status "Count: " & count
    On Key "+" Increment "inc"
    On Key "-" Decrement "dec"
    On Key "q" Quit "quit"

    View
        Column
            Text "Count: " & count
            Text "+/− to change, q to quit"
        End Column
    End View

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

A program is **one** kind of app. Do not mix a `Screen` with a `Window` or a `Page`. `Function Main()` must call `<Name>.Run`.

---

## A program (the language)

Statements end at the newline. Comments are `'` to end of line. Keywords and names are case-insensitive.

```vb
Function Add(ByVal a As Long, ByVal b As Long) As Long
    Return a + b
End Function
```

`Function Name(…) As T` returns `T`. No `As` (or a `Sub`) returns nothing. Do **not** write `As Result<T>`. `Return expr` is success; bare `Return` exits early.

Parameters: `ByVal` copies a number / borrows a `String` (read-only) / borrows a struct or collection. `ByRef` is `&mut`. Numbers and `String` default to `ByVal`. A `Vec` / `HashMap` / `Type` parameter **must** say `ByVal` or `ByRef`. Do not pass a literal to `ByRef`.

---

## Types and `Dim`

| Vinyl | Meaning |
|-------|---------|
| `Integer` | 32-bit int |
| `Long` / `LongLong` | 64-bit int |
| `Single` / `Double` | `f32` / `f64` |
| `Boolean` | `True` / `False` |
| `Byte` | 0–255 |
| `String` | owned text |
| `Vec<T>` | growable list |
| `HashMap<K, V>` | dictionary |
| `Option<T>` | `Some(x)` or `None` |
| `(T, U)` | tuple |

No `Variant`, `Currency`, `Date` (use `DateTime`), or `ReDim`.

```vb
Dim n As Long = 0
Dim name As String = "Ada"
Dim nums As Vec<Long> = [10, 20, 30]
Dim empty As Vec<String> = []
```

`Dim` always needs `As` (except tuple destructure `Dim a, b = pair`). Mutability is inferred. The same `Dim` is legal in `Main`, in a `Type`, and in Screen `State`.

Index with `xs[i]` — **zero-based**. Prefer `.Get(i)` when the index might be missing.

`Const Max As Long = 3` at module level. No mutable globals.

---

## Operators

`^` exponent. `* / Mod`. `+ -`. `&` concatenates. `= <> < > <= >=`. `Not And Xor Or` are **logical and short-circuit**. `/` is floating division (`5 / 2` is `2.5`). Remainder: `n Mod 2`. Integer quotient: store in a `Long`, or `Int(a / b)`.

`=` is equality in an expression and assignment as a statement. Also `+= -= *= /=`. `IIf(cond, a, b)` — both arms the same type.

---

## Control flow

```vb
If n > 0 Then
    Log "pos"
ElseIf n = 0 Then
    Log "zero"
Else
    Log "neg"
End If

For i = 1 To 10
    total = total + i
Next

For Each x In nums
    Log x
Next

Match n
    0 => Log "none"
    1 | 2 => Log "few"
    _ => Log "many"
End Match
```

`Exit For`, `Exit Do`, `Exit Function`, `Continue`.

`Match` is Rust `match`. A bare name **binds**, it does not compare. Write bindings lowercase. `If maybe Is Some(v) Then … End If` unpacks an `Option`.

---

## Strings

`"hello"`. A doubled quote is one quote: `"{""name"":""Ada""}"`. No backslash escapes. Long JSON/SQL: a `Text` block (`Text` on its own line, `End Text`).

| Call | Result |
|------|--------|
| `Len(s)` | character count |
| `Left` / `Right` / `Mid` | slices; **1-based** |
| `Trim` / `LCase` / `UCase` / `Replace(s, a, b)` | |
| `InStr(s, sub)` | `Option` — `Some(pos)` 1-based, or `None` |
| `Split` / `Join` | `Vec<String>` / `String` |
| `s.Contains` / `.Starts_With` / `.Ends_With` | `Boolean` |
| `Val(s)` | `Double`, `0` if not a number |
| `CDbl` / `CLng` / `CInt` | strict parse — can fail |
| `Str(n)` / `CStr(x)` | to text |
| `Format(x, "{:.2}")` | Rust format string |

Maths: `Abs`, `Int`, `Ceiling`, `Round`, `Sqr` / `Sqrt`, `Sin`/`Cos`/`Tan`/`Atn` (radians), `Exp`, `Log(x)` (natural log — parentheses), `Rnd()`.

**On a Screen:** `Log expr` (and `Log.Debug` / `.Info` / `.Warn` / `.Error`) — not `Debug.Print`. `Log(x)` with parens is still natural log. `Sleep` in an Event is illegal — use `Every`.

---

## Collections

```vb
Dim xs As Vec<Long> = [1, 2]
xs.Push(3)
Log xs.Len()
xs.Sort()
```

```vb
Dim ages As HashMap<String, Long>
ages.Insert("Ada", 36)
If ages.Get("Bob") Is Some(a) Then
    Log a
End If
```

Iterator chains (`nums.filter(|x| …).collect()`) are **console only**. In a Screen Event or View, use `For Each`.

---

## `Type` and `Enum`

```vb
Type Person
    Name As String
    Age As Integer
End Type

Enum Size
    Small
    Medium
    Large
End Enum
```

Construct: `Person { Name: "Ada", Age: 36 }`. Variants: `Size.Small`. Payload enums: `Shape.Circle(2.0)`, read with `Match`.

---

## Errors

Failure is a `String`. A normal call **propagates**. Unhandled in `Main` prints and exits 1. Unhandled in a Screen Event ends that event; the screen keeps running.

```vb
Function Load(path As String) As String
    Return FileSystem.Read(path)
End Function
```

`Handle` is postfix on **one** call. No `On Error`, `Try`, `?`, `.Unwrap()`, `Return Ok` / `Return Err`. `Raw F()` gives `Result<T>` for `Match` / `Ok` / `Err`. `Option` is absence, not failure.

---

## Screen — State, View, keys, Events

```
Screen <Name>
    Title "<title>"
    Theme NightOwl             ' optional; omit = cyan-on-black

    State
        Dim <field> As <Type> = <init>
    End State

    Status <expr>              ' left side of the bottom bar
    On Key "<key>" <Event> ["label"]
    On Key "q" Quit "quit"     ' Quit is built-in — no Event needed
    Every 100 Tick             ' optional timer, milliseconds

    View
        Column
            …
        End Column
    End View

    Sub Helper(…)              ' optional, inside the Screen
        …
    End Sub

    Event <Name>
        …
    End Event
End Screen

Function Main()
    <Name>.Run
End Function
```

**State** — ordinary `Dim`s. Later fields may read earlier ones. A fallible init (`Database.Open`) runs before the terminal starts; failure prints `could not start:` and exits.

**View** — a tree. Never mutate widgets. Read fields by name (`count`). `If` / `Match` in the View pick which widgets show.

**Keys** — `On Key "+" Increment "inc"`. The optional string is the hotkey caption on the bar. Named keys: `Esc`, `Enter`, `Tab`, `Up`, `Down`, `Left`, `Right`, `F10`. Character keys are quoted (`"q"`, `"+"`, `"r"`).

**`Quit`** is built-in. `On Key "q" Quit` exits. Do not write `Event Quit`.

**Events** — the only writers. Assign to fields as locals. Payload events take the widget's value (`Event Add(text As String)`). Bare `Return` leaves early. `Return <value>` is a Function's job.

**Helper `Sub`** inside the Screen reads fields directly. Events call it by name. Do not call one Event from another.

**`Every <ms> <Event>`** fires that Event on an interval (animation, polling). Timers pause while a file prompt is open.

**`Status expr`** — live text on the left of the bottom bar. Key bindings appear on the right automatically.

**Menu** (optional, next to View — not inside it):

```vb
Menu
    Menu "File"
        Item "Open" OpenFile
        Separator
        Item "Quit" Quit
    End Menu
End Menu
```

F10 opens the first menu; Enter fires the item's Event (or `Quit`).

`Theme NightOwl` (also `Dracula`, `Nord`, `JellyFish`, `Light`, `Dark`, …) restyles chrome, text, and charts.

### Layout

`Column` stacks vertically; `Row` is side by side. A size line **before** a child is along the main axis:

| Line | Meaning |
|------|---------|
| `Length N` | exactly N rows (Column) or columns (Row) |
| `Percent N` | N% of the container |
| `Fill` / `Fill N` | leftover space, weight N |
| `Min N` | at least N |
| `Spacing N` / `Padding N` | gap / inset |

Defaults: `Text` is 1 row, `Input` is 3 rows, `List`/`Table`/`Chart`/`Tabs` Fill. `Space Height N` / `Space Width N` is a blank gap. `Frame "Title"` … `End Frame` is a bordered panel.

### Everyday widgets

A Screen is **not** a Window. Use `Input` (not `TextInput`), `Memo` (not `TextArea`). No `Slider` — use `Gauge`.

| Widget | Bind | How it fires |
|--------|------|----------------|
| `Text expr` | display | — |
| `Input field` … `On Submit E` … `End Input` | `String` | focused: type + Backspace; Enter → `Event E(text As String)` |
| `Memo field` … `End Memo` | `String` | multi-line; Enter is newline; quit with `Esc` (not `"q"`, which would steal `q`) |
| `List field` … `On Select E` … `End List` | `Vec<String>` | Up/Down; Enter → `Event E(item As String)` |
| `Table field` … `On Select E` … `End Table` | `Vec<Struct>` | Up/Down; Enter → `Event E(row As Struct)` |
| `Button "label"` … `On Click E` … `End Button` | | Tab to focus; Enter/Space |
| `Checkbox "label", field` … `On Toggle E` … `End Checkbox` | `Boolean` | Enter/Space → `Event E(value As Boolean)` |
| `Radio "label", field, option` … `On Select E` … `End Radio` | enum / integer | Enter/Space → `Event E(value As T)` |
| `Gauge 0..=100, field` | numeric | display |
| `Sparkline field` | `Vec` of numbers | display |
| `Chart series` | `Vec<Struct>` with numeric x, y | display |
| `Tabs tab` … `Tab "A"` … `End Tab` … `End Tabs` | `Integer`, 0 = first | Left/Right |

**Focus ring:** Tab cycles `Input` / `Memo` / `List` / `Table` / `Button` / `Checkbox` / `Radio`. The focused widget eats its built-in keys. Put a `Fill` on the list/table so it gets the leftover space.

```vb
Input entry
    On Submit Add
End Input

List notes
    On Select Pick
End List

Event Add(text As String)
    notes.Push(text)
    entry = ""
End Event
```

### `Await` — do not freeze the terminal

Slow work in an Event must use `Await`. One `Await` per event, as a **top-level** `Match` or `Dim` — not inside `If`/`For`. To skip it: `If busy Then Return` *before* the `Await`.

```vb
Event Fetch
    status = "loading…"
    Match Await Http.Get(url)
        Ok(body) => status = "got " & Len(body) & " bytes"
        Err(e) => status = "error: " & e
    End Match
End Event
```

`Http.Get` / `Http.Post` / `Shell.Run` — `Await` them directly.

`FileSystem.Read`, `db.Query`, `DataFrame.Read_Csv` — put them in a **Function** and `Match Await Load(path)` from the Event (or a helper Sub that the Event **ends** with). Calling them from an Event without `Await` is a compile error.

A module `Function` in a Screen program must **not** contain `Await`. `DateTime.Now()` in an Event is fine.

### File dialogs

`GetOpenFilename()` / `GetSaveAsFilename()` / `GetFolderName()` — path prompt over the live Screen (Tab completes, Enter chooses, Esc → `""`). Call from an **Event or helper Sub**. Then `Await` a Function that `FileSystem.Read` / `Write`s.

```vb
Event OpenFile
    Dim picked As String = GetOpenFilename(path)
    If picked = "" Then Return
    Match Await ReadFile(picked)
        Ok(text) => notes = text
        Err(e) => notes = "Could not read: " & e
    End Match
End Event

Function ReadFile(ByVal picked As String) As String
    Return FileSystem.Read(picked)
End Function
```

---

## Standard library

Namespaced calls: `FileSystem.Read("a.txt")`. No import. **Always `runproject`.** In a Screen Event, wrap slow calls as above.

### FileSystem

`Read` / `Read_Lines` / `Write` / `Append` / `Exists` / `Copy` / `Delete` / `List` / `Join` / `Parent` / `Name`. `Exists` does not fail. `List` directory names end with `/`.

### Json

`Json.Parse` / `Json.Object` / `Json.Array`. Read: `Get_String`, `Get_Int`, `Get_Float`, `Get_Bool`, `Get_Array`, `Get`, `Has_Key`. Build: `Set_String` / `Set_Int` / `Set_Bool` / `Set` / `Push`. `To_String` / `To_Pretty`. `As_String` / `As_Int` / `As_Float` / `As_Bool`. Doubled quotes or a `Text` block.

### DateTime

`DateTime.Now()`. `Parse(text, "%Y-%m-%d")`. `Format`, `Year` / `Month` / `Day`, `Add_Days` / `Add_Hours` / `Add_Minutes`, `Diff_Days` / `Diff_Hours`. strftime patterns.

### Regex

Pattern **first**. `Is_Match` / `Find_All` / `Replace` / `Replace_All` / `Captures`. In a Vinyl string, `\\d` is a digit class.

### Http

`Http.Get(url)`. `Http.Post(url, body, headers)` — `headers` is `HashMap<String, String>`. **In a Screen Event: `Match Await Http.Get(url)`.** In console `Main`, just call it.

### Database (SQLite)

`Database.Open("app.db")`. `Execute(sql, params)` / `Query(sql, params)` → `Vec<Json>`. Params fill `?` — `["Ada"]` or `[]`. `Last_Insert_Id`. From an Event, wrap in a Function and `Match Await`.

### Shell

`Shell.Run(cmd)` / `Shell.Start(cmd)` → `Process` with `.Is_Running` / `.Wait` / `.Kill`. In an Event, `Await Shell.Run(…)`.

### DataFrame

`DataFrame.Read_Csv`. `With_Column` / `Filter` (bare names are columns). `Group_By("k").Agg(Sum(qty), Mean(price), Count())`. `Column` / `Sort` / `Join`. `df.Print()`.

---

## Tests

Ignored by `run` / `runproject`. Run with `vbr test`.

```vb
Test "adds two longs"
    Assert Add(2, 3) = 5
End Test
```

**Do not name a `Test` the same as a `Function`.** `Test "IsPrime"` shadows `Function IsPrime`.

---

## Several files

A folder of `.vbr` files. The file with `Function Main()` is the entry. Other files are modules named by the **lowercased filename** (`Life.vbr` → `Life.CountLive(grid)`).

`Public Function` / `Public Type` / `Public Const` cross files. **Functions are qualified.** **Types are not.** A view expression cannot read `Life.WIDTH` directly — put it in State.

---

## Do not

- `Window` / `Page` / `Sketch` / `TextInput` / `TextArea` / `Slider` / `On Click` as the main input model
- Mix a `Screen` with another surface
- **`Debug.Print` on a Screen** — use `Log`
- Poke widgets — assign State
- `Sleep` or blocking I/O in an Event without `Await`
- `Await` inside a module `Function` in a Screen program
- Two `Await`s, or `Await` nested in `If`/`For`
- `Return 1` from an Event (bare `Return` only)
- `Event Quit` — `On Key "q" Quit` is built-in
- `On Key "q"` while a `Memo` needs the letter `q` — use `Esc`
- `Dim x` with no `As`; `Dim a, b As Long`
- `Variant`, `ReDim`, `On Error`, `Select Case`, `Option Explicit`, `New`
- `Date` — use `DateTime`
- `As Result<T>`, `?`, `.Unwrap()`, `Try`/`Catch`
- 1-based list indexes (`InStr`/`Mid` are the 1-based string exceptions)
- Nested fallible calls in one `Handle`
- `Log(x)` when you meant the log **verb** (`Log "msg"`)
- A `Test` named after a `Function`

---

## Tiny examples

```vb
Screen Counter
    Title "Counter"
    Theme NightOwl

    State
        Dim count As Integer = 0
    End State

    Status "Count: " & count
    On Key "+" Increment "inc"
    On Key "-" Decrement "dec"
    On Key "q" Quit "quit"

    View
        Column
            Text "Count: " & count
            Text "+/− to change, q to quit"
        End Column
    End View

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

```vb
Screen Notes
    Title "Notes"

    State
        Dim entry As String = ""
        Dim notes As Vec<String>
        Dim status As String = "type a note, Enter to add"
    End State

    View
        Column
            Length 3
            Input entry
                On Submit Add
            End Input
            Fill
            List notes
                On Select Pick
            End List
            Length 1
            Text status
        End Column
    End View

    On Key Esc Quit

    Event Add(text As String)
        notes.Push(text)
        entry = ""
        status = "added"
    End Event

    Event Pick(item As String)
        status = "selected: " & item
    End Event
End Screen

Function Main()
    Notes.Run
End Function
```

```vb
Screen Fetcher
    Title "Fetch"

    State
        Dim url As String = "https://example.com"
        Dim status As String = "press r to fetch, q to quit"
    End State

    View
        Column
            Text "URL: " & url
            Text status
        End Column
    End View

    On Key "r" Fetch
    On Key "q" Quit

    Event Fetch
        status = "loading…"
        Match Await Http.Get(url)
            Ok(body) => status = "got " & Len(body) & " bytes"
            Err(e) => status = "error: " & e
        End Match
    End Event
End Screen

Function Main()
    Fetcher.Run
End Function
```

```vb
Function Main()
    Dim nums As Vec<Long> = [3, 1, 2]
    nums.Sort()
    Debug.Print "console still uses Debug.Print"
End Function
```
