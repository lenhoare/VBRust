# Vinyl (`.vbr`) — compact context (console + GUI)

Console programs, desktop **Windows**, and the standard library. **Do not** write `Screen`, `Page`, `Sketch`, `Node2D`, `Gpu Draw`, or `On Key` — those are other surfaces.

Vinyl looks like VB. It compiles to Rust. Where VB habit and Rust collide, **Rust wins**: every value has a static type, arrays are zero-based, errors are values (not `On Error`).

A GUI is **not** VB6 forms. You do not poke controls. **State** is the truth, **View** is a picture of that state, **Events** are the only place state changes. Change a field; the view redraws.

---

## Build and run

From the Vinyl repo (builds `vbr` as needed):

```sh
cargo run -- run FILE.vbr              # no stdlib, no Window, no extra crates
cargo run -- runproject FILE.vbr       # Window, stdlib, Use, or a folder of .vbr files
cargo run -- runproject DIR
cargo run -- test FILE.vbr             # run Test blocks
cargo run -- emit FILE.vbr             # print the generated Rust
```

Once built, `target/debug/vbr` is the same CLI (`vbr run …`, `vbr runproject …`).

**Rule:** `run` is only for a self-contained console program (`Dim`, `If`, `Debug.Print`, maths, strings, `Vec`/`HashMap`). A **`Window` always needs `runproject`** (it pulls in Iced). Anything that names `FileSystem`, `Json`, `Http`, `DateTime`, `Regex`, `Database`, `Shell`, `DataFrame`, or `Use` also needs `runproject`. The compiler will say so if you forget.

`runproject` writes a Cargo project under `build/` and runs it. You never edit `Cargo.toml` or turn on features.

---

## Two program shapes

**Console** — `Function Main()` is the program:

```vb
Function Main()
    Debug.Print "hello"
End Function
```

**Window** — declare the window, then launch it from `Main`:

```vb
Window Counter
    Title "Counter"

    State
        Dim count As Long = 0
    End State

    View
        Column
            Text "Count: " & count
            Button "+"
                On Click Increment
            End Button
            Button "-"
                On Click Decrement
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

A program is **one** kind of app. Do not mix a `Window` with a `Screen` or a `Page`. `Function Main()` must call `<Name>.Run`.

---

## A program (the language)

Statements end at the newline. Comments are `'` to end of line. Keywords and names are case-insensitive.

```vb
Function Add(ByVal a As Long, ByVal b As Long) As Long
    Return a + b
End Function
```

`Function Name(…) As T` returns `T`. No `As` (or a `Sub`) returns nothing. Do **not** write `As Result<T>`. `Return expr` is success; bare `Return` exits early.

Parameters: `ByVal` copies a number / borrows a `String` (read-only) / borrows a struct or collection. `ByRef` is `&mut` — the caller is updated. Numbers and `String` default to `ByVal`. A `Vec` / `HashMap` / `Type` parameter **must** say `ByVal` or `ByRef`. Do not pass a literal to `ByRef`.

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
Dim x As Long = 0, y As Long = 0     ' each needs its own As
Dim nums As Vec<Long> = [10, 20, 30]
Dim empty As Vec<String> = []
Dim zeros As Vec<Long> = [0; 5]      ' five zeros
```

`Dim` always needs `As` (except tuple destructure `Dim a, b = pair`). Mutability is inferred — never write `mut`. A `Type` value must be fully constructed at `Dim`. The same `Dim` is legal in `Main`, in a `Type`, and in Window `State`.

Index with `xs[i]` — **zero-based**. `xs[i]` copies the element (strings/structs clone). Prefer `.Get(i)` when the index might be missing.

`Const Max As Long = 3` at module level. No mutable globals.

---

## Operators

`^` exponent. `* / Mod`. `+ -`. `&` concatenates (operands become text). `= <> < > <= >=`. `Not And Xor Or` are **logical and short-circuit** (not bitwise). `/` is floating division (`5 / 2` is `2.5`). Integer remainder: `n Mod 2`. Integer quotient: store in a `Long`, or `Int(a / b)`.

`=` is equality in an expression and assignment as a statement. Also `+= -= *= /=`.

`IIf(cond, a, b)` — both arms the same type.

---

## Control flow

```vb
If n > 0 Then
    Debug.Print "pos"
ElseIf n = 0 Then
    Debug.Print "zero"
Else
    Debug.Print "neg"
End If

If n < 0 Then Return -n            ' single-line, no End If

For i = 1 To 10
    total = total + i
Next

For Each x In nums
    Debug.Print x
Next

Do While n > 0
    n = n - 1
Loop

Match n
    0 => Debug.Print "none"
    1 | 2 => Debug.Print "few"
    3..=9 => Debug.Print "some"
    _ => Debug.Print "many"
End Match
```

`Exit For`, `Exit Do`, `Exit Function`, `Continue`.

`Match` is Rust `match`. A bare name **binds**, it does not compare (`n =>` matches everything). Compare a variable with a guard: `v If v = y => …`. Patterns are Rust: `Some(x)`, `None`, `Ok(n)`, `Err(e)`, `Suit.Hearts`. Write bindings lowercase.

`If maybe Is Some(v) Then … End If` unpacks an `Option`.

---

## Strings

`"hello"`. A doubled quote is one quote: `"{""name"":""Ada""}"`. No backslash escapes. For JSON, SQL, or a long blob, use a `Text` block (verbatim; common indent stripped):

```vb
Dim body As String = Text
    {"name": "Ada", "age": 36}
End Text
```

`Text` is a block only when the rest of the line is blank.

| Call | Result |
|------|--------|
| `Len(s)` | character count |
| `Left(s, n)` / `Right(s, n)` / `Mid(s, start)` / `Mid(s, start, n)` | slices; **1-based** |
| `Trim` / `LCase` / `UCase` / `Replace(s, a, b)` | |
| `InStr(s, sub)` | `Option` — `Some(pos)` 1-based, or `None` |
| `Split(s)` / `Split(s, delim)` | `Vec<String>` |
| `Join(parts)` / `Join(parts, delim)` | `String` |
| `s.Contains(part)` / `.Starts_With` / `.Ends_With` | `Boolean` |
| `Val(s)` | `Double`, `0` if not a number (never fails) |
| `CDbl(s)` / `CLng(s)` / `CInt(s)` | strict parse — can fail |
| `Str(n)` / `CStr(x)` | to text |
| `Format(x, "{:.2}")` | Rust format string, one placeholder |

Maths: `Abs`, `Int`, `Ceiling`, `Round`, `Sqr` / `Sqrt`, `Sin`/`Cos`/`Tan`/`Atn` (radians), `Exp`, `Log(x)` (natural log — parentheses), `Rnd()` (0 ≤ n < 1).

`Debug.Print expr` → stdout (fine in a Window — it goes to the launching terminal). `Log expr` → `vbr.log` (`Log.Debug` / `.Info` / `.Warn` / `.Error`). `Log(x)` with parens is still natural log. `Sleep 100` is milliseconds — **illegal in an Event** (it would freeze the UI). `InputBox("name?")` is console-only.

---

## Collections

```vb
Dim xs As Vec<Long> = [1, 2]
xs.Push(3)
Debug.Print xs.Len()
Debug.Print xs.Get(9).Unwrap_Or(-1)
xs.Sort()
```

```vb
Dim ages As HashMap<String, Long>
ages.Insert("Ada", 36)
If ages.Get("Bob") Is Some(a) Then
    Debug.Print a
End If
```

Iterator chains (`nums.filter(|x| …).collect()`) are **console only**. In a Window Event or View, use `For Each`.

---

## `Type` and `Enum`

```vb
Type Person
    Name As String
    Age As Long
End Type

Enum Suit
    Hearts
    Spades
End Enum

Function Main()
    Dim p As Person = Person { Name: "Ada", Age: 36 }
    Dim s As Suit = Suit.Hearts
End Function
```

Methods: `Function Person.Greet() As String` … `Me.Name` … `End Function`. Payload enums: `Enum Shape` / `Circle(Double)` / `Empty` — build `Shape.Circle(2.0)`, read with `Match`.

---

## Errors

Failure is a `String`. A normal call **propagates**. The variable holds the success value. Unhandled in `Main` prints and exits 1. Unhandled in a Window Event ends that event; the window keeps running.

```vb
Function Load(path As String) As String
    Return FileSystem.Read(path)
End Function

Function Main()
    Dim text As String = Load("a.txt") Handle err
        Debug.Print err
        Return
    End Handle
    Debug.Print text
End Function
```

`Handle` is postfix on **one** call. Do not nest fallible calls inside it. Do not write `On Error`, `Try`, `?`, `.Unwrap()`, `Return Ok(…)`, or `Return Err(…)`.

`Raw F()` gives `Result<T>` as data: `Match r` / `Ok(n)` / `Err(e)`. Ordinary functions still declare `As Long`, not `As Result<Long>`.

`Option` is absence, not failure. Never auto-propagate `Option`.

---

## Window — State, View, Events

```
Window <Name>
    Title "<title>"
    Theme Dracula          ' optional

    State
        Dim <field> As <Type> = <init>
    End State

    View
        Column
            …
        End Column
    End View

    Sub Helper(…)          ' optional, inside the Window
        …
    End Sub

    Event <Name>
        …
    End Event
End Window

Function Main()
    <Name>.Run
End Function
```

**State** — ordinary `Dim`s. Primitives, `String`, `Vec`, `HashMap`, `Option`, tuples, your `Type`s. Later fields may read earlier ones. A fallible init (`Database.Open`) runs before the window opens; failure prints `could not start:` and exits.

**View** — a tree. Never mutate widgets. Read state fields by name (`count`, not `state.count`). `If` / `Match` in the View pick which widgets show.

**Events** — the only writers. Assign to fields as locals: `count += 1`. A widget names the event: `On Click Increment`. Payload events take the new value:

```vb
Event Rename(value As String)
    name = value
End Event
```

Bare `Return` leaves an Event early. `Return <value>` is a Function's job, not an Event's.

**Helper `Sub`** inside the Window reads fields directly. Events call it by name. Two events sharing work belong here — do not try to call one Event from another.

```vb
Sub TryMove(ByVal cell As Long)
    If board[cell] <> "" Then Return
    board[cell] = "X"
End Sub

Event Cell1
    TryMove(0)
End Event
```

`Theme Dracula` (also `Nord`, `NightOwl`, `JellyFish`, `Light`, `Dark`, `CatppuccinMocha`, …) restyles the whole window.

### Layout

`Column` stacks vertically; `Row` is side by side. Nest them. Inside either:

- `Spacing 12` — gap between children
- `Padding 20` — inset
- `Length 40` — next child is 40px on the main axis (height in a Column, width in a Row)
- `Fill` — next child takes leftover space

`Scrollable` … `End Scrollable` for overflow. `Rule Horizontal` / `Rule Vertical` is a separator. `Frame "Title"` … `End Frame` is a bordered box.

### Everyday widgets

| Widget | Bind / payload | Event |
|--------|----------------|-------|
| `Text expr` | display | — |
| `Button "label"` … `On Click E` … `End Button` | | `Event E` |
| `TextInput "hint", field` … `On Input E` … `End TextInput` | `String` | `Event E(value As String)` — assign `field = value` |
| `Checkbox "label", field` … `On Toggle E` … `End Checkbox` | `Boolean` | `Event E(value As Boolean)` |
| `Toggler "label", field` … `On Toggle E` … `End Toggler` | `Boolean` | same |
| `Slider 0..=100, field` … `On Change E` … `End Slider` | `Integer` / `Single` / `Double` / `Byte` — **not `Long`** | `Event E(value As Integer)` |
| `ProgressBar 0..=100, field` | numeric | — |
| `Image "logo.png"` | path | — |

`On Submit E` on a `TextInput` fires on Enter (no extra payload). `Secure` hides typing. `Enabled expr` on a `Button` disables it when false.

```vb
TextInput "Enter your name", name
    On Input Rename
End TextInput

Checkbox "Remember me", remember
    On Toggle SetRemember
End Checkbox

Slider 0..=100, volume
    On Change SetVolume
End Slider
```

Also: `Chooser field From options` (`options` is a `Vec` of the field's type, `On Select`). `List field` over `Vec<String>`, `On Select` gets the `String`. `Table field` over `Vec<Struct>` (columns from the struct). `Tabs tab` … `Tab "Desk"` … `End Tab` … `End Tabs` — `tab` is an `Integer`, 0 = first tab.

`Color` / `BackColor` / `Border` on `Button`, `Frame`, or `Text` (`Color.Navy` or `Color(r, g, b)`). A painted `Text` needs `End Text`.

### `Await` — do not freeze the window

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

`FileSystem.Read`, `db.Query`, `DataFrame.Read_Csv` — **not** Await-stdlib forms. Put them in a **Function** and `Match Await Load(path)` from the Event. Calling that Function (or those calls) from an Event without `Await` is a compile error.

A helper `Sub` may hold the `Await` if the Event **ends** with a call to it. A module `Function` in a Window program must **not** contain `Await` — keep it synchronous; the Event awaits the whole Function.

`DateTime.Now()` in an Event is fine (it is not I/O). `Sleep` in an Event is an error.

### File dialogs

`GetOpenFilename()` / `GetSaveAsFilename()` / `GetFolderName()` — OS dialogs, optional starting path. Return `""` if cancelled. Call from an **Event or helper Sub**, then `FileSystem.Read` / `Write` the path (those reads still go through `Await` + a Function). Not a View widget.

```vb
Event Pick
    Dim picked As String = GetOpenFilename()
    If picked = "" Then Return
    path = picked
End Event
```

---

## Standard library

Namespaced calls: `FileSystem.Read("a.txt")`. No import. **Always `runproject`.** In a Window Event, wrap slow calls as above.

### FileSystem

| Call | |
|------|--|
| `Read(path)` / `Read_Lines(path)` | whole file `String` / `Vec<String>` |
| `Write(path, text)` / `Append(path, text)` | replace / append (creates) |
| `Exists(path)` | `Boolean` — does not fail |
| `Copy(src, dest)` / `Delete(path)` | |
| `List(path)` | names in a folder; directories end with `/` |
| `Join(a, b)` / `Parent(path)` / `Name(path)` | path math, `/` |

### Json

`Json.Parse(text)` → `Json`. `Json.Object()` / `Json.Array()` to build.

Read (can fail if missing/wrong type): `Get_String`, `Get_Int`, `Get_Float`, `Get_Bool`, `Get_Array`, `Get` (nested object). `Has_Key(k)` is `Boolean`.

Build: `Set_String` / `Set_Int` / `Set_Bool` / `Set` (nested Json). `Push` on an array. `To_String` / `To_Pretty`. A value that is itself a string/number: `As_String` / `As_Int` / `As_Float` / `As_Bool`.

Inner quotes in a Vinyl string are doubled, or use a `Text` block.

### DateTime

`DateTime.Now()`. `DateTime.Parse(text, "%Y-%m-%d")` can fail.

On a value: `Format("%Y-%m-%d %H:%M")`, `Year` / `Month` / `Day`, `Add_Days(n)` / `Add_Hours` / `Add_Minutes`, `Diff_Days(other)` / `Diff_Hours`. Patterns are strftime (`%Y %m %d %H %M %S`).

### Regex

Pattern **first**, then text. Can fail if the pattern is invalid. In a Vinyl string, `\\d` is a digit class.

`Is_Match(pat, text)` → `Boolean`. `Find_All(pat, text)` → `Vec<String>`. `Replace` / `Replace_All(pat, text, repl)`. `Captures(pat, text)` → groups of the first match as `Vec<String>`.

### Http

Blocking, one-shot, HTTPS works. Visible type is the body `String`.

`Http.Get(url)`. `Http.Post(url, body, headers)` where `headers` is `HashMap<String, String>` (empty map is fine). **In a Window Event: `Match Await Http.Get(url)`.** In console `Main`, just call it.

### Database (SQLite)

No server. `Dim db As Database = Database.Open("app.db")`.

`db.Execute(sql, params)` → rows changed (`Long`). `db.Query(sql, params)` → `Vec<Json>`. `params` is a list filling `?` — `["Ada"]` or `[]`. `db.Last_Insert_Id()`.

From a Window Event, wrap Query/Execute in a Function and `Match Await Load(…)`.

### Shell

`Shell.Run("echo hi")` → stdout `String` (waits). `Dim p As Process = Shell.Start("sleep 1")` then `p.Is_Running()`, `p.Wait()` → exit code, `p.Kill()`. In an Event, `Await Shell.Run(…)`.

### DataFrame

`Dim df As DataFrame = DataFrame.Read_Csv("sales.csv")`. Methods return a **new** table — assign them.

Bare names inside `With_Column` / `Filter` are **columns**: `df = df.With_Column("total", price * qty)` then `df = df.Filter(total > 100)`. `Group_By("band").Agg(Sum(qty), Mean(price), Count())`. `Column("name")` → `Vec`. `Sort("age")`. `Join(other, "id")`. `df.Print()`.

---

## Tests

Ignored by `run` / `runproject`. Run with `vbr test`.

```vb
Test "adds two longs"
    Assert Add(2, 3) = 5
    Assert Add(0, 0) <> 1
End Test
```

`Assert a = b` / `a <> b` show both sides on failure. Anything else is a boolean assert. Put extra tests in `foo.test.vbr` beside `foo.vbr`.

**Do not name a `Test` the same as a `Function`.** `Test "IsPrime"` lowers to `fn isprime()` and shadows `Function IsPrime` inside the test.

---

## Several files

A folder of `.vbr` files. The file with `Function Main()` is the entry (usually `main.vbr`). Other files are modules named by the **lowercased filename** (`Life.vbr` → `Life.CountLive(grid)`).

`Public Function` / `Public Type` / `Public Const` are visible to other files. Bare / `Private` stay local. **Functions are qualified** (`Utils.DoThing()`). **Types are not** — a `Public Type Person` is just `Person` everywhere.

---

## Do not

- `Screen` / `Page` / `Sketch` / `On Key` / `Gpu Draw` (out of scope here)
- Mix a `Window` with another surface
- Poke widgets (`Label1.Caption = …`) — assign State, the View follows
- `Sleep` or blocking I/O in an Event without `Await`
- `Await` inside a module `Function` in a Window program
- Two `Await`s, or `Await` nested in `If`/`For`
- `Return 1` from an Event (bare `Return` only)
- `Slider` bound to a `Long`
- `Dim x` with no `As`; `Dim a, b As Long` (each needs `As`)
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
Window Counter
    Title "Counter"
    Theme NightOwl

    State
        Dim count As Long = 0
    End State

    View
        Column
            Spacing 8
            Padding 16
            Text "Count: " & count
            Row
                Button "+"
                    On Click Increment
                End Button
                Button "-"
                    On Click Decrement
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

Function Main()
    Counter.Run
End Function
```

```vb
Window Greeter
    Title "Greeter"

    State
        Dim name As String = ""
        Dim hello As String = "type a name"
    End State

    View
        Column
            Padding 16
            TextInput "name", name
                On Input SetName
            End TextInput
            Button "Greet"
                On Click Greet
                Enabled name <> ""
            End Button
            Text hello
        End Column
    End View

    Event SetName(value As String)
        name = value
    End Event

    Event Greet
        hello = "Hello, " & name
    End Event
End Window

Function Main()
    Greeter.Run
End Function
```

```vb
Window Fetcher
    Title "Fetcher"

    State
        Dim url As String = "https://example.com"
        Dim status As String = "idle"
    End State

    View
        Column
            TextInput "url", url
                On Input SetUrl
            End TextInput
            Button "Fetch"
                On Click Fetch
            End Button
            Text status
        End Column
    End View

    Event SetUrl(value As String)
        url = value
    End Event

    Event Fetch
        status = "loading…"
        Match Await Http.Get(url)
            Ok(body) => status = "got " & Len(body) & " bytes"
            Err(e) => status = "error: " & e
        End Match
    End Event
End Window

Function Main()
    Fetcher.Run
End Function
```

```vb
Function Main()
    Dim nums As Vec<Long> = [3, 1, 2]
    nums.Sort()
    Debug.Print "sum path still works in console"
End Function
```
