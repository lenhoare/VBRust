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
- **Keywords and type names are case-insensitive** (`Dim`, `dim`, `DIM`).
  Identifiers are case-sensitive and emitted to Rust as written, except where a
  mapping applies (procedure names → `snake_case`).
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
| `Integer`  | `i16`    |                                                  |
| `Long`     | `i32`    |                                                  |
| `LongLong` | `i64`    |                                                  |
| `Single`   | `f32`    |                                                  |
| `Double`   | `f64`    |                                                  |
| `Boolean`  | `bool`   |                                                  |
| `Byte`     | `u8`     |                                                  |
| `Date`     | `i64`    | No calendar semantics; a plain number (warns).   |
| `String`   | `String` | Owned, unknown size — ownership rules apply.     |

Compound / declared types:

- **Tuple:** `(T1, T2, …)` → `(T1, T2, …)`.
- **Vec:** `Vec<T>` → `Vec<T>`. Element `T` may be a primitive or a named type.
- **Map:** `HashMap<K, V>` → `std::collections::HashMap<K, V>`.
- **Fixed array:** `[T; N]` (see §3).
- **Named type:** a user `Type` (§7) or stdlib type (§10), used by name.

`Currency` and `Variant` are rejected (§12). There are no implicit numeric
coercions beyond integer-literal-to-float on assignment.

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

### Collections — `New`
```
Dim v As New Vec<T>
Dim m As New HashMap<K, V>
```

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
- `Sub` is rejected — use a no-`As` `Function` instead.
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
- `ByVal` (default for fixed-size types) passes by value; `String` by value is
  emitted as `&str` where it is not mutated.
- `ByRef` passes a mutable reference (`&mut T`); the caller's binding is updated.
- Passing a literal to a `ByRef` parameter is rejected.

---

## 5. Expressions & operators

Operators, tightest binding first to last:

| Level | Operators            | Meaning                                  |
|-------|----------------------|------------------------------------------|
| 1     | `^`                  | exponentiation (`.powf`/`.powi`)         |
| 2     | unary `-`            | negation                                 |
| 3     | `*`  `/`             | multiply, divide                         |
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
  (operands are `bool`); there is no bitwise `And`/`Or` overload, and no `Mod` /
  integer-divide (`\`) at this revision. For bitwise or other Rust operators, use
  an inline `Rust` block (§9).

Other expression forms: literals, identifiers, `Me` (→ `self`), calls
`f(a, b)`, method/field access `recv.method(...)` / `recv.field`, indexing
`a(i)`, tuple index, `New` (§3), and inline Rust (§9).

---

## 6. Statements & control flow

### Assignment
```
target = expr            ' Ident or place expression (a.field, a(i))
Set target = expr        ' accepted; same as assignment
```

### Conditional
```
If cond Then
    …
ElseIf cond Then
    …
Else
    …
End If
```

### Select Case
```
Select Case subject
    Case v1, v2
    Case lo To hi             ' inclusive range
    Case Is > n               ' (where supported by guards)
    Case Else
End Select
```
- Lowered to a Rust `match`. A `Case Else` is **required** unless the arms are
  already exhaustive (`Ok`/`Err`, `Some`/`None`) or a bare-binding catch-all is
  present. A non-exhaustive `Select` without `Case Else` is rejected.

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
`Debug.Print expr` → `println!`. Terminal input/prompt replace `MsgBox` /
`InputBox` (§10).

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
- **Methods:** a `Function`/`Sub` whose first parameter is the receiver (`Me`)
  becomes an `impl` method; calls use `recv.method(...)`. Static/associated
  calls use `Type.method(...)` → `Type::method(...)`.

---

## 8. Error model

VBR has no `On Error`. Failure is values, not jumps.

- A fallible procedure declares `As Result<T>` and returns `Err("…")` / `Ok(v)`.
- Optional results use `As Option<T>` with `Some`/`None`.
- A returned `Result` may not be silently ignored at the call site; propagate it
  (Rust `?`) or handle it.
- `On Error …` is rejected with guidance toward `Result`.

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

Crate-using inline Rust additionally requires the project run mode (§13).

---

## 10. Standard library

Provided by the `vbr_stdlib` crate, auto-imported when referenced. Calls are
**namespaced**: `Namespace.member(...)` → `Namespace::member(...)`.

- **Stateless namespaces:** `FileSystem`, `Regex` — module-style functions.
- **Wrapper types:** `DateTime`, `Json` — opaque value types with methods
  (`DateTime.Now()`, `value.Format(...)`, `Json.Parse(...)`, `j.GetString(...)`,
  etc.). Static calls use `.` → `::`; instance calls use `.` → `.`.
- No HTTP at this revision.

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
| `ReDim`                         | Use a `Vec` (grows dynamically).               |
| `On Error …`                    | Use `Result<T>` + `Err`/`?`.                   |
| Mutable module-level globals    | Use `Const`, or pass state / wrap it in a `Type`. |
| `Option Base` / `Option Explicit` | Rust is always zero-indexed and explicit.    |
| `Exit` (other than Do/For/Function) | Only those three targets.                  |
| Ignoring a returned `Result`    | Must propagate or handle.                       |
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
| `run <file>`  | Transpile + `rustc` a single file + execute. **Errors** if the program needs the stdlib (use `runproject`). |
| `runproject <dir>` | Generate a visible `build/` Cargo project (linking `vbr_stdlib`), then `cargo run`. |
| `build <dir>` | Generate the project without running.                           |
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

`Use crate version` dependency declarations (for `.rs` modules or inline Rust
that pull external crates) are specified for the project mode but not yet built —
so today's `.rs` modules may use `std` or `vbr_stdlib`, not arbitrary crates.
