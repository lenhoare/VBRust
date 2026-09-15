# Vinyl (`.vbr`) — compact context

Console programs plus the standard library. **Do not** write `Window`, `Screen`, `Page`, `Sketch`, `Node2D`, `Await`, `Every`, or `View` — those are other surfaces.

Vinyl looks like VB. It compiles to Rust. Where VB habit and Rust collide, **Rust wins**: every value has a static type, arrays are zero-based, errors are values (not `On Error`).

---

## Build and run

From the Vinyl repo (builds `vbr` as needed):

```sh
cargo run -- run FILE.vbr              # no stdlib, no extra crates
cargo run -- runproject FILE.vbr       # stdlib, Use, or a folder of .vbr files
cargo run -- runproject DIR            # DIR contains Function Main() (usually main.vbr)
cargo run -- test FILE.vbr             # run Test blocks
cargo run -- emit FILE.vbr             # print the generated Rust
```

Once built, `target/debug/vbr` is the same CLI (`vbr run …`, `vbr runproject …`).

**Rule:** `run` is only for self-contained programs (`Dim`, `If`, `Debug.Print`, maths, strings, `Vec`/`HashMap`). Anything that names `FileSystem`, `Json`, `Http`, `DateTime`, `Regex`, `Database`, `Shell`, `DataFrame`, or `Use` **must** use `runproject`. The compiler will say so if you forget.

`runproject` writes a Cargo project under `build/` and runs it. You never edit `Cargo.toml` or turn on features.

---

## A program

One entry point. Statements end at the newline. Comments are `'` to end of line. Keywords and names are case-insensitive.

```vb
Function Main()
    Debug.Print "hello"
End Function
```

```vb
Function Add(ByVal a As Long, ByVal b As Long) As Long
    Return a + b
End Function

Function Main()
    Debug.Print Add(3, 4)
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

`Dim` always needs `As` (except tuple destructure `Dim a, b = pair`). Mutability is inferred — never write `mut`. A `Type` value must be fully constructed at `Dim`.

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

Maths: `Abs`, `Int`, `Round`, `Sqr`, `Sin`/`Cos`/`Tan`/`Atn` (radians), `Exp`, `Log(x)` (natural log — parentheses), `Rnd()` (0 ≤ n < 1).

`Debug.Print expr` → stdout. `Log expr` → `vbr.log` (`Log.Debug` / `.Info` / `.Warn` / `.Error`). `Log(x)` with parens is still natural log. `Sleep 100` is milliseconds. `InputBox("name?")` reads a line (fails on EOF).

---

## Collections

```vb
Dim xs As Vec<Long> = [1, 2]
xs.Push(3)
Debug.Print xs.Len()          ' 3
Debug.Print xs.Is_Empty()
Debug.Print xs[0]             ' 1 — panics if out of range
Debug.Print xs.Get(9).Unwrap_Or(-1)
Debug.Print xs.First().Unwrap_Or(-1)
xs.Sort()
xs.Reverse()
xs.Clear()
Dim last As Option<Long> = xs.Pop()
```

```vb
Dim ages As HashMap<String, Long>
ages.Insert("Ada", 36)
Debug.Print ages["Ada"]                    ' panics if missing
If ages.Get("Bob") Is Some(a) Then
    Debug.Print a
End If
Debug.Print ages.Contains_Key("Ada")
ages.Remove("Ada")
```

Iterator chains on a `Vec` (console only): `nums.filter(|x| x > 2).map(|x| x * x).collect()` into a typed `Dim`. Closures are one expression.

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
    Debug.Print p.Name
    Dim s As Suit = Suit.Hearts
    If s = Suit.Hearts Then Debug.Print "red"
End Function
```

Methods: `Function Person.Greet() As String` … `Me.Name` … `End Function`. Payload enums: `Enum Shape` / `Circle(Double)` / `Empty` — build `Shape.Circle(2.0)`, read with `Match`.

---

## Errors

Failure is a `String`. A normal call **propagates**. The variable holds the success value. Unhandled in `Main` prints and exits 1.

```vb
Function Load(path As String) As String
    Return FileSystem.Read(path)       ' can fail — just write it
End Function

Function Main()
    Dim text As String = Load("a.txt") Handle err
        Debug.Print err
        Return
    End Handle
    Debug.Print text

    FileSystem.Delete("tmp") Handle err
        ' swallowed — falling through is allowed on the statement form
    End Handle

    If False Then RaiseError "nope"
End Function
```

`Handle` is postfix on **one** call. Do not nest fallible calls inside it. Do not write `On Error`, `Try`, `?`, `.Unwrap()`, `Return Ok(…)`, or `Return Err(…)`.

`Raw F()` gives `Result<T>` as data: `Match r` / `Ok(n)` / `Err(e)`. Ordinary functions still declare `As Long`, not `As Result<Long>`.

`Option` is absence, not failure. Never auto-propagate `Option`.

---

## Standard library

Namespaced calls: `FileSystem.Read("a.txt")`. No import. **Always `runproject`.**

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

`Http.Get(url)`. `Http.Post(url, body, headers)` where `headers` is `HashMap<String, String>` (empty map is fine).

### Database (SQLite)

No server. `Dim db As Database = Database.Open("app.db")`.

`db.Execute(sql, params)` → rows changed (`Long`). `db.Query(sql, params)` → `Vec<Json>`. `params` is a list filling `?` — `["Ada"]` or `[]`. `db.Last_Insert_Id()`.

```vb
db.Execute("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT)", [])
db.Execute("INSERT INTO t (name) VALUES (?)", ["Ada"])
Dim rows As Vec<Json> = db.Query("SELECT name FROM t", [])
Debug.Print rows[0].Get_String("name")
```

### Shell

`Shell.Run("echo hi")` → stdout `String` (waits). `Dim p As Process = Shell.Start("sleep 1")` then `p.Is_Running()`, `p.Wait()` → exit code, `p.Kill()`.

### DataFrame

`Dim df As DataFrame = DataFrame.Read_Csv("sales.csv")`. Methods return a **new** table — assign them.

Bare names inside `With_Column` / `Filter` are **columns**, applied row-wise: `df = df.With_Column("total", price * qty)` then `df = df.Filter(total > 100)`. `And` / `Or` / `Not` / `IIf` work in a formula. A name that is also a local variable is that variable, not a column. Spaces in a column name: `` `Unit Price` ``.

`Group_By("band").Agg(Sum(qty), Mean(price), Count())`. `Column("name")` → `Vec`. `Sort("age")`. `Join(other, "id")`. `df.Print()` (or `Debug.Print df`).

---

## Tests

Ignored by `run` / `runproject`. Run with `vbr test`.

```vb
Test "adds"
    Assert Add(2, 3) = 5
    Assert Add(0, 0) <> 1
End Test
```

`Assert a = b` / `a <> b` show both sides on failure. Anything else is a boolean assert. Put extra tests in `foo.test.vbr` beside `foo.vbr`.

---

## Several files

A folder of `.vbr` files. The file with `Function Main()` is the entry (usually `main.vbr`). Other files are modules named by the **lowercased filename** (`Life.vbr` → `Life.CountLive(grid)`).

`Public Function` / `Public Type` / `Public Const` are visible to other files. Bare / `Private` stay local. **Functions are qualified** (`Utils.DoThing()`). **Types are not** — a `Public Type Person` is just `Person` everywhere.

---

## Do not

- `Window` / `Screen` / `Page` / `On Click` / `Await` / `Every` (out of scope here)
- `Dim x` with no `As`; `Dim a, b As Long` (each needs `As`)
- `Variant`, `ReDim`, `On Error`, `Select Case`, `Option Explicit`, `New`
- `Date` — use `DateTime`
- `As Result<T>`, `?`, `.Unwrap()`, `Try`/`Catch`
- 1-based list indexes (`InStr`/`Mid` are the 1-based string exceptions)
- Nested fallible calls in one `Handle`
- `Log(x)` when you meant the log **verb** (`Log "msg"`)

---

## Tiny examples

```vb
Function Main()
    Dim nums As Vec<Long> = [3, 1, 2]
    nums.Sort()
    Dim total As Long = 0
    For Each n In nums
        total = total + n
    Next
    Debug.Print "sum = " & total
End Function
```

```vb
Function Main()
    FileSystem.Write("notes.txt", "hello")
    Dim text As String = FileSystem.Read("notes.txt") Handle err
        Debug.Print "read failed: " & err
        Return
    End Handle
    Debug.Print text
End Function
```

```vb
Function Main()
    Dim doc As Json = Json.Object()
    doc.Set_String("name", "Ada")
    doc.Set_Int("age", 36)
    Debug.Print doc.To_String()
End Function
```
