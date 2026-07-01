# VBR — Language Specification

Concise, normative reference for the VBR language as transpiled by this
implementation. Not a tutorial. Where a construct is unsupported it is listed in
§12, with the diagnostic the compiler emits.

**Prime directive:** VBA-flavoured syntax in, idiomatic Rust out. Where VBA
semantics and Rust semantics conflict, **Rust wins** — VBR exposes Rust's rules
(ownership, static typing, exhaustive matching) rather than hiding them.

---

## 1. Source & lexical structure

- Source files use the `.vbr` extension; UTF-8.
- **Statements** are newline-terminated. There is no statement separator.
- **Keywords are case-insensitive** (`Dim`, `dim`, `DIM`).
- **Identifiers are effectively case-insensitive** (as in VB). Each is re-cased
  for Rust idiom — procedures, variables, parameters, and fields → `snake_case`;
  `Const` names → `SCREAMING_SNAKE_CASE`; `Type` (struct) names kept as written
  (expected PascalCase) — so names differing only in case collapse to the same
  identifier (`Total` and `total` both become `total`). Use one consistent
  spelling per name. A rename emits a one-time `ℹ`/`⚠` note.
- **Comments:** `'` to end of line. Emitted as `//` in the output.
- **String literals:** `"…"`. A doubled quote `""` inside a literal denotes one
  `"` (VBA escaping). No backslash escapes.
- **Numeric literals:** integer (`42`, `-7`) and float (`3.14`). A float literal
  in a numeric expression is tagged `f64` in output to avoid f32/f64 ambiguity.
- **Booleans:** `True`, `False`.

---

## 2. Types

Primitive VBR types map to Rust as follows:

| VBR        | Rust     | Notes                                            |
|------------|----------|--------------------------------------------------|
| `Integer`  | `i32`    | Rust's default integer (not VBA's 16-bit).       |
| `Long`     | `i64`    |                                                  |
| `LongLong` | `i64`    | Same as `Long`; kept for familiarity.            |
| `Single`   | `f32`    |                                                  |
| `Double`   | `f64`    |                                                  |
| `Boolean`  | `bool`   |                                                  |
| `Byte`     | `u8`     |                                                  |
| `String`   | `String` | Owned, unknown size — ownership rules apply.     |

`Date` is **not** a built-in type (it would be a number with no calendar
semantics) — use `DateTime` from the standard library (§10).

Integer sizes are chosen **Rust-first**: `Integer` is the `i32` a Rust
programmer expects, not VBA's legacy 16-bit. Bare integer literals are `i32`;
loop counters are `i32` but adapt to their use (Rust infers a wider type from
context, e.g. when added to a `Long`).

Compound / declared types:

- **Tuple:** `(T1, T2, …)` → `(T1, T2, …)`.
- **Vec:** `Vec<T>` → `Vec<T>`. Element `T` may be a primitive or a named type.
- **Map:** `HashMap<K, V>` → `std::collections::HashMap<K, V>`.
- **Fixed array:** `[T; N]` (see §3).
- **Named type:** a user `Type` (§7) or stdlib type (§10), used by name.

`Currency` and `Variant` are rejected (§12). Where VB would coerce one numeric
type to another silently, VBR inserts an **explicit Rust `as` cast** (e.g. a
`Long` assigned into a `Double` becomes `… as f64`) — the conversion VB hides,
made visible. Bare numeric literals adapt to their context instead (a `5` in a
float slot is emitted `5.0`).

---

## 3. Declarations

### Variables — `Dim`
```
Dim name As Type                 ' default-initialised
Dim name As Type = expr          ' with initialiser
Dim a, b = tupleExpr             ' tuple destructuring (inferred)
```
- A single `Dim` **requires** `As Type`. Only the tuple-destructure form is
  type-inferred.
- Mutability is **inferred**: a variable is emitted `let mut` iff it is later
  assigned or mutated, otherwise `let`.
- A struct must be fully initialised at its `Dim` (no declare-then-fill).

### Arrays
```
Dim x(10) As Long          ' fixed: [i32; 10], zero-initialised
Dim g(3, 4) As Long        ' fixed 2-D: [[i32; 4]; 3]
Dim v() As Long            ' growable: Vec<i32>, starts empty
Dim v() As Long = expr     ' growable with initialiser (e.g. .collect())
```
Indexing is `x(i)` in source → `x[i]` in output; **zero-based**. `ReDim` is
rejected — use a `Vec`.

### Collections
```
Dim v As Vec<T>
Dim m As HashMap<K, V>
```
`Vec<T>` and `HashMap<K, V>` are the built-in collections. A `New` keyword (VB
habit) is accepted but **warns** — Rust creates the value from the declaration
itself, so `New` is redundant.

### Constants — `Const`
```
Const Name As Type = literalExpr
[Public | Private] Const …
```
Module-level constants are permitted. Mutable module-level globals are **not**
(§12).

---

## 4. Procedures

```
[Public | Private] Function Name(params) As RetType
    …
    Return expr
End Function
```
- `Function … As T` returns `T`. `RetType` may be a primitive, named type,
  tuple, `Result<T>`, or `Option<T>`. A `Function` with **no** `As` returns
  nothing (`()`).
- `Sub Name(params) … End Sub` is accepted as **sugar** for a no-return
  `Function` — both become a Rust `fn` — and emits a one-time note. A `Sub` may
  not declare `As T` (it returns nothing).
- `Return expr` yields a value; bare `Return` exits early. The transpiler lowers
  a trailing `Return` to a Rust tail expression where possible.
- Procedure names are emitted `snake_case`.
- **`Public`** makes a function visible to other modules (emitted `pub fn`); bare
  or `Private` functions are file-local (§13). The same applies to `Type` and
  `Const`.
- **Entry point:** `Function Main()` → `fn main()`.

### Parameters
```
[ByVal | ByRef] name As Type
```
- `ByVal` passes by value: a fixed-size type is copied; a `String` is a read-only
  borrow `&str`; a struct/collection is an immutable borrow `&T`.
- `ByRef` passes a mutable reference `&mut T`; the caller's binding is updated.
- **Default mode:** fixed-size types and `String` default to `ByVal` if no mode
  is given. Struct and collection parameters require an explicit `ByVal`/`ByRef`.
- **Writing to a `ByVal String` parameter is rejected** (it is read-only) — use
  `ByRef` to modify the caller's string. Passing a literal to a `ByRef` parameter
  is also rejected.

---

## 5. Expressions & operators

Operators, tightest binding first to last:

| Level | Operators            | Meaning                                  |
|-------|----------------------|------------------------------------------|
| 1     | `^`                  | exponentiation (`.powf`/`.powi`)         |
| 2     | unary `-`            | negation                                 |
| 3     | `*`  `/`  `Mod`      | multiply, divide, remainder (`%`)        |
| 4     | `+`  `-`             | add, subtract                            |
| 5     | `&`                  | string concatenation                     |
| 6     | `=` `<>` `<` `>` `<=` `>=` | comparison                          |
| 7     | `Not`                | logical negation → `!` (unary)           |
| 8     | `And`                | logical and → `&&` (short-circuit)       |
| 9     | `Xor`                | logical xor → `^` (on `bool`)            |
| 10    | `Or`                 | logical or → `\|\|` (short-circuit)      |

- `=` is equality in expression position and assignment in statement position.
- `&` concatenates; operands are formatted to `String`.
- Logical operators are **looser than comparison** and **short-circuit**, exactly
  as in Rust — `a > 0 And b < 10` → `a > 0 && b < 10`. They are logical only
  (operands are `bool`); there is no bitwise `And`/`Or` overload.
- `Mod` is the remainder operator → Rust's `%` (Rust rules: integer remainder for
  ints, float remainder for floats), at the same precedence as `*`/`/` (Rust's,
  not VB's own rung). Integer-divide (`\`) is not provided. For bitwise or other
  Rust operators, use an inline `Rust` block (§9).

Other expression forms: literals, identifiers, `Me` (→ `self`), calls
`f(a, b)`, method/field access `recv.method(...)` / `recv.field`, indexing
`a(i)`, tuple index, and inline Rust (§9).

### Rust methods

Method calls pass straight through to Rust, so the full Rust API is available
alongside the familiar VB functions — use whichever reads better:

```vb
Dim name As String = "  Ada  "
Debug.Print UCase(name)          ' VB function — call form Trim(s), Left(s, n), …
Debug.Print name.trim()          ' Rust method — call form s.trim(), s.replace(a, b), …
Debug.Print name.trim().to_uppercase()   ' methods chain
```

- Method names **lower to idiomatic Rust**: both `.Trim()` (muscle memory) and
  `.trim()` emit Rust's `.trim()`. Free functions (`Trim(s)`) are the VB world;
  methods (`s.trim()`) are the Rust world — they never clash.
- A curated set of common methods is **type-aware**, so the same coercions VB
  functions get apply: assigning `s.trim()` (a `&str`) to a `String` inserts
  `.to_string()`; `.len()` is a `usize` in comparisons; `.contains(...)` is a
  `bool`. Known returns include `trim*` (`&str`); `to_uppercase`/`to_lowercase`/
  `replace`/`repeat`/`to_string` (`String`); `len`/`count`/`capacity` (`usize`);
  `is_empty`/`contains`/`starts_with`/`ends_with` (`bool`).
- **Number** methods carry the receiver's own type: `n.abs()`/`min`/`max`/
  `clamp`/`pow`/`signum` are the receiver's type, the float family (`sqrt`,
  `floor`, `ceil`, `round`, `powf`, `ln`, the trig functions, …) is the float
  type, and `is_nan`/`is_finite`/`is_power_of_two` are `bool`. So `Dim k As Long
  = n.abs()` keeps `k` a `Long`.
- **Vec/collection** methods pass through too. The `Option`-returning accessors
  (`first`, `last`, `get`) pair with `Match` over `Some`/`None`; `len` is a
  `usize`; `join` builds a `String`. `vec.contains(x)` takes `&T`, so the borrow
  (and owning a string element) is inserted for you — unlike `str.contains`,
  which is left as-is.
- A **mutating** method (`push_str`, `push`, `sort`, …) makes its receiver `mut`
  for you.
- Methods outside the curated set still pass through verbatim; they simply skip
  the auto-coercion, and `rustc` is the backstop if a type doesn't line up.

---

## 6. Statements & control flow

### Assignment

```
target = expr            ' plain assignment (Ident or place: a.field, a(i))
target += expr           ' compound assignment; also -=  *=  /=
```
Compound assignment is numeric and lowers to Rust's `+=`/`-=`/`*=`/`/=` (a
convenience beyond VB6; there is no `&=`, since `&` formats rather than appends).

### Borrowing — `Set`

`Set` binds a name as a **borrow** of another value (Rust `&` / `&mut`) instead
of copying it:

```
Set name = value         ' shared borrow   → let name = &value;
Set Mut name = value     ' mutable borrow  → let name = &mut value;
```

Unlike VB — where `Set` is for object references only — **VBR's `Set` works on
any variable**. It is the one explicit way to say "point at this, don't copy it."

`Set` is meaningful for **owned / non-`Copy`** types (`String`, structs, `Vec`,
`HashMap`), where a borrow avoids a move or a `.clone()`. On `Copy` primitives
(`Long`, `Boolean`, …) a borrow is legal but pointless — Rust copies them freely.
A note explains the borrow.

```
Dim greeting As String = "Hello"
Set view = greeting                  ' borrow → let view = &greeting;  (no copy)
Dim copy As String = greeting.clone()  ' owned copy → greeting.clone();
Debug.Print view                     ' greeting is still usable; view just points at it
```
The borrow (`view`) and the clone (`copy`) make Rust's central distinction —
**reference vs. ownership** — visible, using familiar VB syntax.

### Conditional
```
If cond Then
    …
ElseIf cond Then
    …
Else
    …
End If

If cond Then stmt                  ' single-line form, no End If
If cond Then stmt Else stmt
```
The single-line form takes one statement per branch (e.g. `If x < 0 Then Return -x`).

### Match
```
Match subject
    100 => Debug.Print "exact"      ' a literal
    1 | 2 | 3 => DoThing()          ' alternation
    4..=10 => DoOther()             ' inclusive range
    Ok(n) => Use(n)                 ' destructure / bind
    n If n < 0 => Handle(n)         ' a binding with an If guard
    _ =>                            ' wildcard, multi-statement body:
        Log("default")
        Reset()
End Match
```
- Lowered straight to a Rust `match`. Every arm is **`pattern => body`** — there
  is no `Case` keyword. The body is one statement on the same line, or an indented
  block on the following lines (running until the next arm or `End Match`).
- **Patterns are real Rust**, passed through verbatim: literals, `|` alternation,
  `..=` / `..` ranges, constructor/struct/tuple destructuring (`Ok(n)`, `Some(x)`,
  `Point { x, y }`, `(a, b)`), bindings, and `_`. A guard is `<pattern> If <cond>`.
- A bare name **binds** (it does not compare) — `n => …` matches everything and
  names it `n`. To compare against a variable, use a guard: `v If v = y => …`.
- **Exhaustiveness is rustc's job**: there is no forced catch-all. For a `Result`,
  covering `Ok`/`Err` is complete; a missing case is a compile error that names
  exactly what you left out.
- Name bindings should be written snake_case (the Rust convention) — that's how
  the body refers to them.

### Loops
```
For i = lo To hi [Step s]      ' numeric range
Next

For Each x In collection
Next

Do … Loop
Do While cond … Loop           ' pre-test
Do … Loop Until cond           ' post-test
```
A condition may sit on the `Do` **or** the `Loop`, not both.

### Loop control
`Exit Do`, `Exit For`, `Exit Function`, `Continue`.

### Output / input
`Debug.Print expr` → `println!`. `MsgBox` / `InputBox` are lowered to terminal
output and prompted input (no GUI), as built-ins — not part of the stdlib crate.

---

## 7. User-defined types (`Type`)

```
[Public | Private] Type Name
    field As Type
    …
End Type
```
- Emitted as a Rust `struct`. Construct with all fields:
  `Dim p As Person = Person { name: "…", age: 30 }`.
- **Methods** are declared `Function TypeName.MethodName(params) …` and become
  `impl` methods; inside the body, **`Me`** is the receiver (`→ self`). Instance
  calls use `recv.method(...)`; a method that mutates `Me` takes `&mut self`.
  Associated/static calls use `Type.method(...)` → `Type::method(...)`.

### Enums (`Enum`)

```
[Public | Private] Enum Name
    Variant
    …
End Enum
```
- A simple (C-like) enum: a named set of unit variants. Emitted as a Rust
  `#[derive(Debug, Clone, Copy, PartialEq, Eq)] enum`. Variant names keep their
  PascalCase (not snake-cased), as in Rust.
- Reference a variant with a dot — `Suit.Hearts` → `Suit::Hearts` (a *name path*,
  not a value access; see §1). The same dot-to-`::` applies in `Match` patterns:
  `Match s / Suit.Hearts => …`.
- Values are `Copy` and compare with `=` (`If s = Suit.Spades Then …`), pair with
  `Match`, and work as a `Dim`/parameter/return type (`Dim s As Suit = …`).

A variant may carry a **tuple payload**, making it a Rust **sum type** — the same
shape as `Option`/`Result`, but your own:

```
Enum Shape
    Circle(Double)
    Rectangle(Double, Double)
    Empty
End Enum
```
- Build one with `Shape.Circle(2.0)`; read the data back by **matching** (the only
  way — like Rust): `Match s / Shape.Circle(r) => … / Shape.Rectangle(w, h) => …`.
- Derives are computed from the payloads: `Debug`/`Clone`/`PartialEq` always,
  `Copy` unless a `String` payload, `Eq` unless a float payload.
- V1: payloads are **primitives or `String`**, and enums are **non-recursive**
  (a variant can't hold its own enum). Richer payloads (structs, `Vec`, nested
  enums) and recursion (auto-`Box`) come later.

---

## 8. Error model

VBR has no `On Error` — no exceptions, no jumps. **Failure is a value.** A
fallible function declares `As Result<T, E>` and returns `Ok(v)` on success or
`Err(e)` on failure. The caller receives that `Result` — a value that is
*either* `Ok` or `Err`, not a bare `T` — and **cannot silently ignore it**.
`As Option<T>` (`Some`/`None`) is the same idea for "a value, or nothing."

`Result<T, E>` is a real generic type — the error is a **typed value**, exactly
as in Rust. `E` can be a `String`, or your own enum (`Result<User, ParseError>`),
etc. **`Result<T>` is shorthand for `Result<T, String>`** — the easy path for
beginners, no lie about Rust. `Err("…")` coerces the string literal to `String`
when `E` is `String`; with a typed `E` you construct the variant: `Err(ParseError.TooLong)`.
A fallible action that returns *nothing* on success uses `Result<()>` with
`Return Ok(())` (`()` is the unit value).

> `?` note: Rust's `?` converts the error via `From`, which VBR can't generate.
> So `?` works when the inner error type **matches** the enclosing function's `E`
> (or both are `String`). Mixing different typed errors with `?` isn't supported
> yet — handle those with `Match`.

A `Result` at the call site must be **handled**, **propagated**, or
**unwrapped**:

- **Handle it** — `Match` over `Ok`/`Err` (or `Some`/`None`):
  ```
  Match Divide(10, 2)
      Ok(value) => Use(value)       ' success — value is the T
      Err(message) => Report(message) ' failure — message is the error
  End Match
  ```
- **Propagate it — `?`** — `Dim x As Long = MightFail()?` means: *if it's `Err`,
  return that error from the current function immediately; if it's `Ok`, take the
  value and continue.* It is shorthand for the "unwrap on success / early-return
  on failure" pattern, and is the idiomatic way to pass a failure up to whoever
  called you. **`?` is only valid inside a function that itself returns `Result`**
  (the propagated error needs somewhere to go).
- **Unwrap it — `.Unwrap()`** — returns the value, or panics (crashes) on `Err`.
  Allowed, but flagged as training wheels; avoid in real code.

Rule of thumb: use `?` when handling the failure isn't *this* function's job
(push it up); use `Match` at the level that knows how to recover or report.

Silently discarding a returned `Result` (calling it as a bare statement) is
rejected. `On Error …` is also rejected, with guidance toward `Result`.

---

## 9. Inline Rust

A `Rust … End Rust` block is a **Rust block expression** spliced verbatim into
the generated function.

```
Dim x As T = Rust
    <raw Rust>
End Rust
```
- **Inputs:** in-scope VBR variables are available by their emitted (snake_case)
  Rust name. No declaration needed.
- **Output:** the block's value is its last line **without** a semicolon (Rust
  tail expression). A trailing `;` discards the value.
- **Multiple outputs:** return a tuple; bind with `Dim a, b = Rust … End Rust`.
- **Typed form** (`Dim x As T = …`): the block must produce a VBR-expressible
  type `T`, which crosses fully back into VBR.
- The block body is captured **verbatim** (not tokenised); terminated by a line
  that is exactly `End Rust`.

**Opaque handles** — `Dim name = Rust … End Rust` with **no `As`**:
- Hold a value whose type is not VBR-expressible; Rust infers and owns the type.
- The handle's **only** legal use is being spliced into a later inline-Rust block
  (by name). Any other appearance — printing, comparison, assignment, passing to
  a procedure — is rejected (§12).
- Confined to one function (it cannot cross a procedure boundary, since VBR has
  no type name for it). Within the function it may flow between any number of
  Rust blocks, persisting state between them.
- Emitted `let mut` (VBR cannot see whether a later block mutates it).

An inline block may use an external crate: declare it with `Use` (§13) and run
the project with `runproject`.

---

## 10. Standard library

Provided by the `vbr_stdlib` crate, auto-imported when referenced. Calls are
**namespaced**: `Namespace.member(...)` → `Namespace::member(...)`.

- **Stateless namespaces:** `FileSystem`, `Regex`, `Http` — module-style
  functions. `Http.Get(url)` / `Http.Post(url, body)` are blocking, one-shot
  requests returning `Result<String>` (the body); for a reused client/session,
  use inline Rust or a `.rs` module.
- **Wrapper types:** `DateTime`, `Json` — opaque value types with methods
  (`DateTime.Now()`, `value.Format(...)`, `Json.Parse(...)`, `j.GetString(...)`,
  etc.). Static calls use `.` → `::`; instance calls use `.` → `.`.

Every dependency-bearing namespace is behind a Cargo **feature** (`json`,
`datetime`, `regex`, `http`); `FileSystem` is std-only and always available. The
project generator enables exactly the features a program uses, so a project only
compiles the wrappers it touches.

Programs that reference the stdlib must be built/run via the project run mode
(§13), not the single-file `run`.

---

## 11. Diagnostics

Three levels, prefixed in output:

- `✘` **error** — rejects the program; explains why and what to do instead.
- `⚠` **warning** — compiles, but flags a hazard; emitted once per kind.
- `ℹ` **note** — informational.

Diagnostics are teaching-oriented: each rejection states the Rust reason and the
idiomatic alternative.

---

## 12. Rejected constructs

These parse but are deliberately refused, each with guidance:

| Construct                       | Reason / redirect                              |
|---------------------------------|------------------------------------------------|
| `Currency`                      | No fixed-point type; use `Double` or minor units in `Long`. |
| `Variant`                       | Types must be known at compile time; declare the concrete type. |
| `Date`                          | Use `DateTime` from the stdlib (a bare date is just a number). |
| `ReDim`                         | Use a `Vec` (grows dynamically).               |
| `On Error …`                    | Use `Result<T>` + `Err`/`?`.                   |
| Mutable module-level globals    | Use `Const`, or pass state / wrap it in a `Type`. |
| `Option Base` / `Option Explicit` | Rust is always zero-indexed and explicit.    |
| `Exit` (other than Do/For/Function) | Only those three targets.                  |
| Ignoring a returned `Result`    | Must propagate or handle.                       |
| `?` outside a `Result`/`Option` function | Propagation needs a fallible caller; use `Match`, or declare `As Result<T>`. |
| Passing a literal to `ByRef`    | Needs an assignable place.                      |
| Declaring a struct uninitialised | Construct fully at `Dim`.                      |
| Indexing where a bound/type is unknown | Compile-time error with explanation.     |
| Using an opaque handle as a value | Only pass it back into a `Rust` block (§9).   |

(`With` blocks and a few others are similarly rejected; see the error snapshot
suite for the authoritative set.)

---

## 13. Tooling & run modes

The CLI compiles a `.vbr` source through lexer → parser → resolver → transpiler.

| Command       | Behaviour                                                       |
|---------------|-----------------------------------------------------------------|
| `run <file>`  | Transpile a single file with `rustc` and execute. **Errors** if it uses the stdlib or any `Use` crate (those can't be linked by `rustc` alone — use `runproject`). |
| `runproject [dir]` | Generate a visible `build/` Cargo project — multifile `.vbr`/`.rs` modules, the stdlib, and `Use` crates — then `cargo run` it. Defaults to the current directory. |
| `build [dir]` | Generate the project without running.                           |
| `transpile <file>` | Write the generated Rust to `<file>.rs` (or `-o`).         |
| `emit <file>` | Print the generated Rust to stdout.                             |

### Projects (multifile)

A **project is a folder of `.vbr` files**, built by `runproject`/`build`:

- The file with `Function Main()` (default `main.vbr`) is the **entry** → crate
  root `main.rs`, which declares the others with `mod <name>;`.
- Every other `<File>.vbr` becomes a module named `to_snake(File)`
  (`MyHelpers.vbr` → module `my_helpers`).
- **Cross-module calls are qualified:** `Shapes.CircleArea(r)` →
  `crate::shapes::circle_area(r)`. The callee must be `Public`.
- A sibling **`.rs` file is a hand-written module**, included **verbatim** (it
  skips transpilation) and called with the same qualified syntax —
  `Text.Shout(s)` → `crate::text::shout(s)`. This is the in-project "wrapper"
  for stateful or unwrapped Rust, with no published crate required. Since VBR
  doesn't see its signatures, argument types must match the Rust side directly.
- Generated layout is **visible and explorable** under `build/`
  (`src/main.rs`, `src/<module>.rs`, `Cargo.toml`); regenerated each run.

### External crates — `Use`

A top-level **`Use <crate> <version>`** declares a Cargo dependency:

```
Use rand 0.8          →   [dependencies]  rand = "0.8"
```
- A version is **required** (reproducible builds); omitting it is an error.
- Declarations across all project files are aggregated into one `Cargo.toml`.
- `Use` only adds the dependency; the crate is *used* from an inline `Rust`
  block (which brings in its own traits, e.g. `use rand::Rng;`) or from a `.rs`
  module. This is what makes inline Rust and `.rs` modules able to reach the
  crate ecosystem.
- Any `Use` makes the program a project build: single-file `run` refuses it and
  points to `runproject` (`rustc` alone can't link crates).
