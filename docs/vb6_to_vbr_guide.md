# VBR for VB6 Programmers

*The quick on-ramp. If you already know VB6, this is the short list of what's
different — enough to be productive in an afternoon. For the full story see
`language_reference.md`; when the two disagree, the reference wins.*

VBR looks like VB and compiles to idiomatic Rust. You write the familiar syntax;
out comes real Rust, which is then built and run. The golden rule, whenever VB
habit and Rust reality collide: **Rust wins.** This guide is just the collisions.

---

## The five-minute mental shift

| VB6 habit | In VBR |
|-----------|--------|
| `Variant`, late binding | Gone. Every value has a static type you name. |
| `Dim x` with no type | `Dim` always carries `As` — the type is never guessed. |
| `Integer` is 16-bit | `Integer` is 32-bit (`i32`); `Long` is 64-bit. |
| `Select Case` | `Match` — no `Case` keyword, and a bare name **binds**, not compares. |
| `On Error GoTo` | Gone. A failure is a returned value (`Result`). |
| `ReDim`, `scores(i)` | Arrays are fixed; growable lists are `Vec`; index with `scores[i]`. |
| `New`, manual mutability | Values are made by their declaration; `mut` is inferred for you. |
| 1-based arrays | Zero-based, like the rest of the modern world. |
| `Declare Sub ... Lib "kernel32"` | The common ones (`Sleep`, dates, files) are built in. |

Everything else — `If/Then/Else`, `For/Next`, `Do/Loop`, `&` for concatenation,
`Function`/`Sub`, `.` for members — is what you already know.

---

## Types and `Dim`

The primitives are VB's names on Rust's machine types:

| VBR | Rust | | VBR | Rust |
|-----|------|-|-----|------|
| `Integer` | `i32` | | `Boolean` | `bool` |
| `Long` / `LongLong` | `i64` | | `Byte` | `u8` |
| `String` | `String` | | `Single` | `f32` |
| `Double` | `f64` | | | |

Three VB types are deliberately **absent**: `Variant` (a static language names the
type it means), `Currency` (use `Double`, or count integer cents in a `Long`), and
`Date` (a date needs a calendar — use `DateTime` from the standard library).

```vb
Dim count As Long                    ' declared, 0 by default
Dim name As String = "Ada"           ' declared and initialised
Dim x As Long = 0, y As Long = 0     ' several at once — each keeps its own As
```

Two quiet wins:

- **Mutability is inferred.** A variable you later assign becomes Rust's `let mut`
  automatically. You never write `mut`.
- **Names are case-insensitive**, as in VB (`total` and `Total` are one name).
  On the way to Rust each name is simply its lowercased self; constants are
  uppercased. You won't notice if you keep one spelling.

Constants live at the top level: `Const MaxRetries As Long = 3`.

The old trap `Dim a, b As Integer` — where `a` silently became a `Variant` — is
**refused**, with a nudge to give each variable its own `As`.

### Operators, with two gotchas

`+ - * / ^ Mod` all work; `^` is exponentiation, `Mod` is remainder. `&`
concatenates. Comparisons are `= <> < > <= >=`. Logic is words — `And Or Not Xor`
— and they are **logical and short-circuiting** (Rust's `&& || ! ^`), *not*
bitwise. There is no `\` integer division. For bitwise ops or anything else Rust
has, drop into an inline `Rust` block (below).

Where VB silently converted numbers, VBR inserts a visible `as` cast — assign a
`Long` into a `Double` and you'll see `as f64` appear. That's a teaching moment,
not a wart.

---

## Strings, `ByVal`, and your first taste of ownership

A `String` **owns** its text; a borrowed view of one is a `&str`. This is where
`ByVal`/`ByRef` stops being a style choice and becomes how Rust borrows.

```vb
Function Shout(ByVal text As String) As String   ' text arrives as &str (a borrow)
Function AddTo(ByRef total As Long, ByVal amount As Long)  ' total is &mut — writes flow back
```

- A parameter with **no keyword defaults to `ByVal`**, which for a `String` is a
  *read-only borrow*. Read it freely; you just can't reassign it.
- Reach for **`ByRef`** only when you actually need to change the caller's value.
  VBR inserts the `&mut` at the call site and marks the caller's variable mutable.

Trying to mutate a `ByVal` string is a friendly error that names the fix ("declare
it `ByRef`"). That nagging is the whole point — it's Rust's ownership, introduced
one message at a time.

---

## Control flow

`If / ElseIf / Else / End If` is exactly what you expect. The two things worth
your attention:

### `Match` replaces `Select Case`

No `Case` keyword. Every arm is `pattern => body`:

```vb
Match n
    0 => Debug.Print "zero"
    1 | 2 | 3 => Debug.Print "small"
    4..=10 => Debug.Print "medium"
    _ => Debug.Print "large"
End Match
```

The patterns are **real Rust** — `|` for alternatives, `..=` for ranges, `_` for
the wildcard. A `Match` must be exhaustive, but that's *rustc's* job: miss a case
and it names exactly what you left out.

**The one sharp difference from `Select Case`:** a bare name doesn't compare, it
**binds** (matches everything and names it). `Case y` in VB asked "equal to `y`?";
in a Rust pattern `y` catches all. To compare against a variable, use a guard:

```vb
Match n
    v If v < 0 => Debug.Print "negative"
    0         => Debug.Print "zero"
    _         => Debug.Print "ordinary"
End Match
```

(Keep pattern bindings lowercase — the pattern is raw Rust, the body is VBR, and
lowercase makes the two halves line up.)

### Loops

```vb
For i = 1 To 5          ' Rust makes its own i — drop any separate "Dim i"
Next                    ' and the counter is gone after Next

For Each name In names  ' borrows each element
    Debug.Print name
Next

Do While total < 100 ... Loop        ' test first
Do ... Loop Until done               ' test after
```

`Exit For/Do/Function` and `Continue` do the obvious. And the pause every module
used to `Declare`: `Sleep 500` (milliseconds) is built in.

---

## Functions and `Sub`

```vb
Function Square(ByVal n As Long) As Long
    Return n * n
End Function
```

A `Sub` is just a `Function` with no `As` (no return) — VBR accepts it as familiar
sugar and reminds you they're the same thing. `Public` makes a function visible to
other modules in a project (`pub fn`); without it, it's private to its file.

---

## Your own types: `Type` and `Enum`

A `Type` is an *and* (a name **and** an age) — it becomes a `struct`:

```vb
Type Person
    Public name As String
    Public age As Long
End Type

Dim p As Person = Person { name: "Ada", age: 36 }   ' built complete, all at once
```

Methods carry the type name; `Me` is the receiver. VBR works out `&self` vs
`&mut self` by watching whether you assign to a field:

```vb
Function Person.Greet() As String
    Return "I am " & Me.name
End Function
```

An `Enum` is an *or* — exactly **one of** a set. The plain form is a named set of
choices (`Suit.Hearts`); the powerful form lets a variant **carry data**, giving
you Rust's real superpower, the sum type:

```vb
Enum Shape
    Circle(Double)
    Rectangle(Double, Double)
    Empty
End Enum
```

You build one by calling the variant — `Shape.Circle(2.0)` — and the *only* way to
read the payload back is to `Match`, which unpacks it. The compiler guarantees
every `Match` handles every variant.

---

## Errors are values, not jumps

There is no `On Error GoTo`. Rust has no exceptions; **a failure is an ordinary
returned value.** A function that can fail says so in its type:

```vb
Function Divide(ByVal a As Long, ByVal b As Long) As Result<Long>
    If b = 0 Then Return Err("cannot divide by zero")
    Return Ok(a / b)
End Function
```

`As Option<T>` (with `Some`/`None`) is the same idea for "a value, or nothing."
You then do one of three things with the box you get back:

```vb
' 1. HANDLE it — examine both outcomes:
Match Divide(10, 2)
    Ok(value)   => Debug.Print "got " & value
    Err(reason) => Debug.Print "failed: " & reason
End Match

' 2. PROPAGATE it with ? — "not my job to handle":
Dim q As Long = Divide(a, b)?      ' returns the Err from THIS function on failure

' 3. UNWRAP it (training wheels — crashes on failure):
Dim v As Long = Divide(10, 2).Unwrap()
```

`?` is only legal where the enclosing function itself returns `Result`/`Option` —
VBR tells you plainly if you forget.

This shows up in conversions too. `Val(" 42x ")` is the forgiving one (a `Double`,
`0` for junk, never fails). The strict `CDbl` / `CLng` / `CInt` return a `Result` —
in VB they raised a runtime error; here that error is a value you catch.

---

## Arrays, `Vec`, and iterators

```vb
Dim scores(10) As Long             ' fixed, stack, zero-based; index scores[i]
Dim nums As Vec<Long>              ' growable list
nums.push(10)
Dim names As Vec<String> = ["alice", "bob"]    ' inline list literal
Dim ages As HashMap<String, Long>
ages.insert("Ada", 36)
```

No `ReDim` — a list that grows is a `Vec`. Index with **brackets** (`scores[i]`),
not `scores(i)`; `scores.get(i)` is the checked, optional read. Reading a `String`
out of a list clones it for you (Rust won't let you leave a hole).

Instead of writing loops to transform data, chain iterator adapters with closures
written `|x| expr`:

```vb
Dim big As Vec<Long> = nums.filter(|x| x > 2).map(|x| x * 2).collect()
```

`filter`, `map`, `collect`; terminal `sum`, `count`, `any`, `all`. (These assume
`Copy` elements — numbers, in practice; richer cases go in an inline `Rust` block.)

---

## The escape hatch: inline Rust (and Python)

VBR covers a friendly slice of Rust. For *everything else*, splice in a block of
the real thing. A `Rust … End Rust` block is a **Rust expression**: your VBR
variables are already in scope (by their lowercased names), and the block's value
is its **last line written with no semicolon**.

```vb
Dim big As Long = Rust
    let mut total = 0;
    for i in 1..=100 { total += i; }
    total
End Rust
```

This is "inline assembly" for VBR — the door to Rust operators, traits, ranges and
crates VBR doesn't surface. Declare a crate with `Use rand 0.8`; the trait and
generic complexity stays sealed inside the block, and only a plain value comes
back. A `Dim` with **no `As`** holds an *opaque handle* (an iterator, a client) you
can pass back into later blocks.

There's a second door — `Python … End Python` — that *runs* real CPython (via
pyo3) so you can reach numpy, pandas and friends. Same rule: the last line is the
value, extracted into the type you annotate.

```vb
Dim mean As Double = Python
    import numpy as np
    np.array([1, 2, 3, 4]).mean()
End Python
```

---

## The mirror: embedding VBR *inside* Rust

If you have a Rust file and want to write a chunk of it in VBR, do the reverse.
Write VBR inside a `/* vbr … */` block comment, then run `vbr embed <file.rs>`:

```rust
fn main() {
    let limit = 5;
    /* vbr
        Dim total As Long = 0
        For i = 1 To limit
            total = total + square(i)      ' calls the Rust fn below
        Next
    */
    println!("sum of squares 1..={} = {}", limit, total);
}

fn square(n: i64) -> i64 { n * n }
```

`vbr embed` transpiles the block and writes the Rust into a managed
`// vbr:gen … // vbr:gen-end` region right after it (re-run any time; it's
idempotent). Because embedding resolves at build time, the VBR and the Rust share
one scope — a VBR loop can read Rust variables (`limit`) and call Rust functions
(`square`) with no ceremony; rustc checks the seam. In VS Code the ▶ button (or
**Ctrl+Alt+R**) expands and runs such a file in one click.

---

## And you should be aware…

These exist; you don't need them to start, but it's good to know the ceiling is
high. Each has its own spec in `docs/`.

- **Whole apps, same language.** A `Window` block builds a desktop GUI (over Iced),
  a `Screen` block a terminal TUI (over ratatui), and a `Page` block a web app
  (over Yew) — all sharing one `State` / `View` / `Events` core. There's even a
  visual form designer in the IDE.
- **Games.** A `Node2D` / `Node3D` block compiles to a Godot 4 extension —
  `vbr rungodot` and you're moving sprites.
- **Other backends.** The same core-language file can transpile to **Python**
  (`vbr py`) or **C** (`vbr c`), not just Rust — handy for teaching or for dropping
  VBR logic into an existing codebase.
- **A real standard library.** Namespaced calls for `FileSystem`, `DateTime`,
  `Shell`, `Regex`, `Json`, `Database` (SQLite), `Http`, and a native
  Excel-style `DataFrame` (over polars) — pulled in only when you use them.
- **Projects.** A folder is a project: multiple `.vbr` modules, `Public` across
  files, `Use` for crates, data files copied into the build.
- **Tests and logging.** `vbr test` runs `Test`/`Assert` blocks that read like a
  spec; `Log <expr>` writes a timestamped line even inside a GUI/TUI.
- **Editor support.** A VS Code extension gives colours, completion, hover,
  go-to-def, live error squiggles, and a side pane showing the Rust your VBR
  becomes as you type — the transpiler's whole point, made visible.

---

## Running it

```
vbr run hello.vbr          # transpile → rustc → execute, in one step
vbr run myfolder/          # a folder is a project
vbr emit hello.vbr         # just print the generated Rust
vbr py hello.vbr           # transpile to Python instead
vbr test hello.vbr         # run the Test blocks
```

The generated Rust is never a secret — reading it beside your VBR is the fastest
way to actually *learn* Rust, which, when you're ready, is the real destination.

---

## One-screen cheat sheet

| VB6 | VBR |
|-----|-----|
| `Select Case x` / `Case 1` | `Match x` / `1 => …` (no `Case`; bare name binds) |
| `On Error GoTo` | `As Result<T>` + `Match` / `?` / `.Unwrap()` |
| `ReDim arr(n)` | `Dim v As Vec<T>` … `v.push(x)` |
| `arr(i)` | `arr[i]` (or `arr.get(i)`) |
| `Dim a, b As Integer` | `Dim a As Integer, b As Integer` |
| `Set o = New Thing` | `Dim o As Thing = Thing { … }` |
| `Currency` / `Variant` / `Date` | `Double` / (name the type) / `DateTime` |
| `Declare Sub Sleep …` | `Sleep 500` (built in) |
| `x And y` (bitwise) | inline `Rust` block |
| `MyName` mutability | inferred — never write `mut` |
| An `.frm` form | a `Window` / `Screen` / `Page` block |
