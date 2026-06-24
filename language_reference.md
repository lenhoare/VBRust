# The VBR Programming Language

*A reference for programmers who already know Visual Basic.*

VBR is a small language. You write something that looks like VB; out comes
idiomatic Rust, which is then compiled and run. The point is not to hide Rust but
to lead you into it: the syntax is familiar, the semantics are Rust's, and where
the two disagree, **Rust wins**. You will meet ownership, static types, and
exhaustive matching — not as obstacles, but a few at a time, in a setting you
already understand.

This is the discursive guide. Its terse companion, `language_spec.md`, is the
normative reference; when in doubt, that document is the law. Throughout, examples
are shown as a pair — the VBR on the left of the arrow, the Rust it becomes on the
right — because the second half is the whole point.

```
Debug.Print "hello, world"     →     println!("{}", "hello, world");
```

---

## 1. A Tutorial Introduction

The only way to learn a language is to write in it, and the first program is
always the same:

```vb
Function Main()
    Debug.Print "hello, world"
End Function
```

`Function Main()` is the entry point; it becomes Rust's `fn main()`. `Debug.Print`
writes a line to the terminal. Save this as `hello.vbr` and run it:

```
vbr run hello.vbr
```

`run` transpiles the file, hands it to the Rust compiler, and executes the
result, all in one step. The generated Rust is no secret:

```rust
fn main() {
    println!("{}", "hello, world");
}
```

A larger program shows the shape of things. This one sums the numbers from 1 to
100:

```vb
Function Main()
    Dim total As Long = 0
    For i = 1 To 100
        total += i
    Next
    Debug.Print "the total is " & total
End Function
```

Several things are worth noting already. A variable is introduced with `Dim` and
must be given a type with `As`. The loop variable `i` is declared by the `For`
itself. `total += i` is compound assignment — the familiar `+=`, `-=`, `*=`, `/=`;
write `total = total + i` if you prefer, but you needn't. The `&` operator joins
strings, converting `total` to text as it goes. And `total` was declared without
`mut`, yet we change it — VBR notices and makes the binding mutable for you. The
Rust is what you would have written by hand:

```rust
fn main() {
    let mut total: i64 = 0;
    for i in 1..=100 {
        total += i;
    }
    println!("{}", format!("{}{}", "the total is ", total));
}
```

When a program needs the standard library or an outside crate, the single-file
`run` is not enough — those must be linked by Cargo, not the bare compiler. For
those, `vbr runproject` builds a small, visible Cargo project in a `build/`
directory beside your sources and runs it. We return to projects in §10.

---

## 2. Types, Declarations, and Operators

### Types

VBR's primitive types are VB's names mapped to Rust's machine types:

| VBR | Rust | | VBR | Rust |
|-----|------|-|-----|------|
| `Integer` | `i32` | | `Boolean` | `bool` |
| `Long` | `i64` | | `Byte` | `u8` |
| `LongLong` | `i64` | | `Single` | `f32` |
| `String` | `String` | | `Double` | `f64` |

The sizes are chosen the way a Rust programmer expects, not the way VB's history
dictates: `Integer` is the everyday 32-bit integer, not a 16-bit relic, and
`Long` is 64 bits. `LongLong` survives as a synonym for `Long`.

Some VB types are deliberately absent. `Currency` has no Rust counterpart (use
`Double`, or count integer cents in a `Long`). `Variant` cannot exist in a
statically typed language — name the type you mean. `Date` is gone too: a date
with no calendar behind it is just a number wearing a costume, so VBR sends you to
the `DateTime` type in the standard library (§10).

### Declarations

```vb
Dim count As Long              ' declared, zero by default
Dim name As String = "Ada"     ' declared and initialised
```

A single `Dim` always carries `As`; the type is never guessed. **Mutability is
inferred**: a binding that is later assigned or modified is emitted `let mut`,
otherwise plain `let`. You never write `mut` yourself.

Identifiers are **case-insensitive**, as in VB — `total` and `Total` are the same
name — because each is recoined in Rust's house style on the way out: variables,
parameters, and functions become `snake_case`, constants `SCREAMING_SNAKE_CASE`,
and type names are left as written (PascalCase by convention). Keep one spelling
per name and you will never notice.

Constants are declared at the top level and are immutable:

```vb
Const MaxRetries As Long = 3     →     const MAX_RETRIES: i64 = 3;
```

### Operators

Arithmetic is ordinary: `+  -  *  /  ^  Mod`. The caret is exponentiation
(lowered to Rust's `powi`/`powf`), and `Mod` is the remainder operator, which
becomes Rust's `%`:

```vb
Debug.Print 17 Mod 5           →     17 % 5        ' 2
```

`Mod` binds as tightly as `*` and `/` — Rust's rule, not VB's separate rung — but
since output is parenthesised where it matters, `a + b Mod c` groups as
`a + (b Mod c)` either way.

The ampersand `&` concatenates, formatting each side to a string. Comparison uses
`=  <>  <  >  <=  >=`; note that `=` is equality in an expression and assignment
as a statement — position decides.

Logical operators are words: `And`, `Or`, `Not`, `Xor`. They are **logical and
short-circuiting**, exactly like Rust's `&&`, `||`, `!`, `^`, and they bind
*looser* than comparison, so the obvious thing works without parentheses:

```vb
If age >= 18 And member Then     →     if age >= 18 && member {
```

There is no bitwise overload of `And`/`Or`, and no integer-division `\`. For those,
or any other Rust operator, drop into an inline `Rust` block (§9).

Where VB would quietly convert one number to another, VBR inserts an explicit Rust
`as` cast — the conversion VB hides, made visible. Assign a `Long` to a `Double`
and you will see `as f64` appear; this is a teaching moment, not a wart.

---

## 3. Control Flow

### If

```vb
If score >= 90 Then
    grade = "A"
ElseIf score >= 80 Then
    grade = "B"
Else
    grade = "C"
End If
```

This is the `if / else if / else` you expect. The conditions are ordinary boolean
expressions.

### Select Case

`Select Case` becomes a Rust `match`, and this is where a familiar construct hides
a sharp Rust truth. The simple forms are exactly what you'd guess:

```vb
Select Case n
    Case 0
        Debug.Print "zero"
    Case 1, 2, 3
        Debug.Print "small"
    Case 4 To 10
        Debug.Print "medium"
    Case Else
        Debug.Print "large"
End Select
```

Literals and constants compare as you expect, and `4 To 10` is an inclusive range.
A `Select` must be exhaustive — `match` covers every value — so either provide
`Case Else` (or `Case _`, the wildcard) or arms that already cover everything.

The trap is comparing against a **variable**. In VB, `Case y` means "is the
subject equal to the value in `y`?" In a Rust `match`, a bare name in a pattern is
not a comparison at all — it *binds*, matching everything. The two readings are
opposite, so VBR refuses the ambiguous case outright:

```vb
Select Case x
    Case y          ' ✘ rejected: y is a variable
```

> ✘ `Case y` can't compare against a variable — Rust's match would treat `y` as a
> catch-all that matches everything. To compare, use a guard: `Case v If v = y`.

The guard is the honest tool. `Case v If condition` binds the subject to `v` and
keeps the arm only when the condition holds:

```vb
Select Case n
    Case v If v < 0
        Debug.Print "negative"
    Case 0
        Debug.Print "zero"
    Case v If v > 100
        Debug.Print "huge"
    Case _
        Debug.Print "ordinary"
End Select
```

`Select Case` over `Ok`/`Err` and `Some`/`None` is special and covered in §8.

### Loops

The counting loop:

```vb
For i = 1 To 5            →     for i in 1..=5 {
Next                            }
```

A `Step` clause changes the stride. `For Each` walks a collection, borrowing each
element:

```vb
For Each name In names           →     for name in &names {
    Debug.Print name                   ...
Next                                   }
```

The `Do` loop comes in the usual flavours — the condition may sit on the `Do`
(tested first) or the `Loop` (tested after), but not both:

```vb
Do While total < 100             →     while total < 100 { ... }
    ...
Loop

Do                               →     loop { ...; if done { break; } }
    ...
Loop Until done
```

`Exit Do`, `Exit For`, and `Exit Function` break out early; `Continue` skips to the
next turn of the loop.

---

## 4. Functions

A function takes parameters and may return a value:

```vb
Function Square(ByVal n As Long) As Long
    Return n * n
End Function
```

The return type follows `As`. A function with no `As` returns nothing — that is
the whole meaning of a `Sub`, which VBR accepts as familiar sugar:

```vb
Sub Greet(ByVal name As String)        →     fn greet(name: &str) {
    Debug.Print "Hello, " & name              println!(...);
End Sub                                       }
```

(VBR will gently remind you that a `Sub` is just a returnless `Function`; both
become a plain Rust `fn`.) A trailing `Return value` is lowered to Rust's bare
tail expression, so the generated code reads as a Rust programmer would write it.

### How arguments are passed

The choice of `ByVal` or `ByRef` is the choice of how Rust borrows, and this is
the first real encounter with ownership.

`ByVal` passes a copy of a fixed-size value. For a `String` — which has no fixed
size — `ByVal` passes a borrowed slice `&str` instead of moving the whole string,
because moving it would consume the caller's copy:

```vb
Function Shout(ByVal text As String) As String     →     fn shout(text: &str) -> String {
```

`ByRef` passes a mutable reference, `&mut T`; changes the function makes are seen
by the caller:

```vb
Function AddTo(ByRef total As Long, ByVal amount As Long)
    total = total + amount
End Function
```

VBR inserts the `&mut` at the call site for you, and marks the caller's variable
mutable. Passing a literal where a `ByRef` is expected is an error — there is
nothing for the reference to point at.

You needn't write `ByVal` for the common case. A parameter with no keyword
defaults to `ByVal`, so a string you only read is plain:

```vb
Function Loudly(message As String) As String     →     fn loudly(message: &str) -> String {
    Return message & "!"
End Function
```

Because that default is a *read-only* borrow, trying to change such a parameter is
an error that names the fix:

```vb
Function Append(s As String)
    s = s & " more"     ' ✘ s is read-only (ByVal) — declare it ByRef to change it
End Function
```

So the rule is pleasant in practice: read a string parameter freely with no
ceremony; reach for `ByRef` only at the moment you actually need to change the
caller's string. (Struct and collection parameters, being larger and rarer, still
ask you to say `ByVal` or `ByRef` outright.)

### Visibility

A function is private to its file unless marked `Public`, which makes it callable
from other modules in a project (§10) and emits `pub fn`.

---

## 5. Strings and Ownership

Here is the heart of Rust, met gently. A `String` owns its characters; a `&str`
borrows a view of someone else's. VBR makes every `String` an owned, heap value,
so the rules are uniform:

```vb
Dim greeting As String = "Hello"       →     let greeting: String = "Hello".to_string();
```

Concatenation builds a fresh owned string; it never quietly shares storage:

```vb
Dim full As String = greeting & ", World"
```

Now suppose you want a second name for the same string without paying for a copy.
That is a **borrow**, and VBR spells it `Set`:

```vb
Set view = greeting            →     let view = &greeting;
```

This is `Set` doing more than VB ever asked of it. In VB, `Set` assigned object
references; in VBR it means "make `view` point at this value rather than copy it,"
and it works on *any* variable, not just objects. It is meaningful for owned types
like `String` and structs, where a borrow saves a move or a clone; on a small
`Copy` number it is legal but pointless.

When you genuinely want an independent copy, ask for one:

```vb
Dim copy As String = greeting.Clone()
```

The contrast between `Set view = greeting` (borrow — no copy) and
`greeting.Clone()` (a new owned string) is the contrast between a reference and
ownership, the single most important idea in Rust. VBR shows it to you in VB
clothing.

When the rules are broken — using a value after it has been moved, say — the error
explains the Rust reason and the VB-shaped fix, rather than leaving you to decode a
borrow-checker message cold.

---

## 6. Arrays, Collections, and Iterators

A fixed array has a size known at compile time and lives on the stack:

```vb
Dim scores(10) As Long         →     let scores: [i64; 10] = [0; 10];
```

The size is the element count, and indices run `0` to `N-1` — zero-based, as Rust
and modern sense demand. Index with brackets; the VB-style `scores(i)` is rejected
in favour of `scores[i]`, with `scores.get(i)` available for a checked, optional
read. There is no `ReDim`: a list that grows is a `Vec`, not a resized array.

A `Vec` is a growable list:

```vb
Dim nums As Vec<Long>          →     let mut nums: Vec<i64> = Vec::new();
nums.push(10)
nums.push(20)
```

(VB's `New` is unnecessary here and earns a gentle warning if you write it; the
value is created by the declaration itself.) A `HashMap` maps keys to values:

```vb
Dim ages As HashMap<String, Long>
ages.insert("Ada", 36)
```

You walk either with `For Each`, which borrows the elements.

### Iterators

For transforming a collection without writing a loop, VBR offers Rust's iterator
adapters, driven by closures written `|x| expr`:

```vb
Dim big As Vec<Long> = nums.filter(|x| x > 2).map(|x| x * 2).collect()
```

becomes

```rust
let big: Vec<i64> = nums.iter().copied().filter(|&x| x > 2).map(|x| x * 2).collect();
```

`filter` keeps elements, `map` transforms them, and `collect` gathers the result —
the type of which is taken from the `As Vec<Long>` annotation, so no turbofish is
needed. Terminal operations like `sum`, `count`, `any`, and `all` end a chain with
a single value. These adapters currently assume `Copy` element types — numbers, in
practice; richer element types are a job for an inline `Rust` block.

---

## 7. Structures and Methods

A `Type` gathers related data into one value, becoming a Rust `struct`:

```vb
Type Person
    Public name As String
    Public age As Long
End Type
```

A struct must be built complete; you cannot declare it empty and fill it in later:

```vb
Dim p As Person = Person { name: "Ada", age: 36 }
```

Fields are read and written with a dot. Methods are functions whose name carries
the type, and inside them `Me` is the receiver:

```vb
Function Person.Greet() As String
    Return "I am " & Me.name
End Function

Function Person.HaveBirthday()
    Me.age = Me.age + 1
End Function
```

These become an `impl` block. A method that only reads takes `&self`; one that
assigns to a field of `Me` — like `HaveBirthday` — takes `&mut self`, and VBR works
out which by watching what the method does. Call them with a dot:

```vb
Debug.Print alice.Greet()
alice.HaveBirthday()
```

Because `HaveBirthday` borrows `alice` mutably, `alice` is made a `let mut` for
you. Calls qualified by the type name — `Person.Something(...)` — become Rust's
associated-function syntax `Person::something(...)`.

---

## 8. Errors as Values

VB signalled failure by jumping: `On Error GoTo`. Rust has no jumps and no
exceptions. **A failure is an ordinary value.** A function that may fail says so in
its type, returning a `Result`:

```vb
Function Divide(ByVal a As Long, ByVal b As Long) As Result<Long>
    If b = 0 Then
        Return Err("cannot divide by zero")
    End If
    Return Ok(a / b)
End Function
```

The caller receives a box that is *either* `Ok(value)` *or* `Err(reason)` — not a
bare number — and the compiler will not let it be ignored. `As Option<T>`, with
`Some` and `None`, is the same idea for "a value, or nothing."

There are three things you may do with such a box.

**Handle it**, examining both outcomes with `Select Case`:

```vb
Select Case Divide(10, 2)
    Case Ok(value)
        Debug.Print "got " & value
    Case Err(message)
        Debug.Print "failed: " & message
End Select
```

(These arms are exhaustive on their own — `Ok` and `Err` cover every case — so no
`Case Else` is required.)

**Propagate it** with the `?` operator, when handling the failure is not this
function's job:

```vb
Function DoubleQuotient(ByVal a As Long, ByVal b As Long) As Result<Long>
    Dim q As Long = Divide(a, b)?
    Return Ok(q * 2)
End Function
```

The single `?` means: if the call failed, return that error from
*this* function immediately; if it succeeded, unwrap the value and carry on. It is
shorthand for a whole `Select Case … Return Err` dance. Because it returns an error
from the enclosing function, `?` is only legal where that function itself returns
`Result` or `Option` — VBR tells you so plainly if you forget.

**Unwrap it** with `.Unwrap()`, which yields the value or crashes on failure. It is
allowed, and flagged as training wheels; real code should handle or propagate.

The rule of thumb: `?` when the failure belongs to someone above you; `Select Case`
at the level that knows how to recover or report. `On Error` is rejected with a
nudge toward all of the above.

---

## 9. Inline Rust

VBR covers a friendly slice of Rust. For everything else there is an escape hatch:
a block of real Rust, spliced in where you write it.

```vb
Dim big As Long = Rust
    let mut total = 0;
    for i in 1..=100 {
        total += i;
    }
    total
End Rust
```

A `Rust … End Rust` block is a Rust *expression*. Your VBR variables are already
in scope inside it by their snake-cased names — no passing required — and the
block's value is its last line written **without** a semicolon, exactly as a Rust
block returns its tail expression. The declared type (`As Long` here) says what the
block must produce, and that value flows back into VBR. For several results, return
a tuple and destructure it.

This is the place for the Rust operators, traits, and library calls that VBR does
not surface directly. It is "inline assembly" for VBR: a deliberate, visible door
into the lower level, used in small doses.

### Opaque handles

Sometimes you want to hold a Rust value that VBR has no type for — an iterator, a
network client, a parser — and reuse it across several blocks. Declare it with no
`As`, and VBR will hold it as an **opaque handle**:

```vb
Dim client = Rust reqwest::blocking::Client::new() End Rust

Dim body As String = Rust
    client.get("https://example.com").send().unwrap().text().unwrap()
End Rust
```

Rust infers the handle's type; VBR keeps it but cannot interpret it. The one thing
you may do with a handle is hand it back into another `Rust` block — you cannot
print it, compare it, or assign it, because VBR does not know what it is. It lives
for the duration of its function, and state held inside it persists from one block
to the next. That is how a connection or an iterator survives across calls without
a global and without a wrapper.

---

## 10. Projects, Modules, and the Standard Library

A single file is a program. A **folder of files is a project**, and `vbr
runproject` builds the lot.

### Modules

The file with `Function Main()` is the entry point and becomes the crate root. Every
other `.vbr` file is a module, named after the file (`Geometry.vbr` → module
`geometry`). Items a module wants to share are marked `Public`; calls across
modules are qualified by the module name:

```vb
' in main.vbr
Debug.Print Geometry.Area(r)       →     crate::geometry::area(r)
```

A module need not be VBR. A `.rs` file dropped in the same folder is included
**verbatim** as a hand-written Rust module, called with the same qualified syntax.
This is the in-project "wrapper": when you have some gnarly or stateful Rust — a
session object, a custom parser — you keep it in a small `.rs` file of your own,
not a published crate. A project quietly accumulates these until, one day, it is
simply a Rust project. That is the intended destination.

### External crates

To use a crate from the wider ecosystem, declare it with `Use`:

```vb
Use rand 0.8
```

This adds the dependency to the generated `Cargo.toml`. The crate is then *used*
from an inline `Rust` block (which brings in its own traits) or from a `.rs`
module:

```vb
Use rand 0.8

Function Main()
    Dim roll As Long = Rust
        use rand::Rng;
        rand::thread_rng().gen_range(1..=6)
    End Rust
    Debug.Print "you rolled a " & roll
End Function
```

Anything using `Use` or the standard library needs the project build; the
single-file `run` will say so and point you to `runproject`.

### The standard library

`vbr_stdlib` is a curated set of conveniences, imported automatically when you
mention them. Calls are namespaced, the dot becoming Rust's path separator:

```vb
Dim text As String = FileSystem.Read("notes.txt")?
```

`FileSystem`, `Regex`, and `Http` are stateless namespaces of functions; `DateTime`
and `Json` are value types you hold and use — the polished, pre-built version of
the `.rs`-helper idea, done once for the common cases:

```vb
Dim now As DateTime = DateTime.Now()
Dim later As DateTime = now.AddDays(30)
Debug.Print later.Format("%Y-%m-%d")
```

`Http` does simple, blocking, one-shot requests:

```vb
Dim body As String = Http.Get("https://example.com")?
```

For a *stateful* HTTP client — a reused connection, cookies, auth across many
calls — this isn't the tool; that's the case for an opaque handle or a `.rs`
module holding a `reqwest::Client`. The stdlib keeps the easy case easy.

Each namespace that pulls a real crate (`Json`, `DateTime`, `Regex`, `Http`) sits
behind a Cargo feature, and the project build enables only the ones you use — so a
program that just reads a file compiles nothing extra. `FileSystem` is always
there.

### Running and seeing

| command | what it does |
|---------|--------------|
| `vbr run file.vbr` | compile one file with `rustc` and run it |
| `vbr runproject [dir]` | build the visible `build/` Cargo project and run it |
| `vbr build [dir]` | generate the project without running |
| `vbr transpile file.vbr` | write the generated Rust to a file |
| `vbr emit file.vbr` | print the generated Rust |

The `build/` directory is generated, visible, and yours to explore. Nothing is
hidden. You can ignore it while you are comfortable, read it when you are curious,
run `cargo` in it yourself when you are ready, and keep it when the day comes that
you no longer need the VB on top. That day is the whole purpose of the language.
