## VBR Version 0.1 Complete Specification — Revision 3

---

## Core Philosophy
- VBA syntax, Rust semantics — never choose the VB way when there is conflict
- Idiomatic Rust output (verbose option later)
- Educational errors — never preachy, warn once not repeatedly
- Inline Rust escape hatch for advanced features
- CLI first, others later
- **When in doubt, use Rust syntax not VBA syntax. VBR is a teaching tool, not a comfort blanket**
- **Use Rust syntax when it appears in small doses and makes the user curious. Keep VBA syntax when replacing it would overwhelm the user**
- **If Rust knows the size at compile time it copies freely. If it doesn't you must be explicit**

---

## Syntax Origin Reference

| Rust syntax in VBR | VBA syntax kept in VBR |
|---|---|
| `::` navigation | `Function/End Function` |
| `<>` generics | `If/ElseIf/Else/End If` |
| `Use` keyword | `Match/Case/End Match` |
| `?` operator | `Dim` |
| `.unwrap()` | `For/Next` |
| `Ok`, `Err`, `Result` | `Do While/Loop` |
| `mut` | `Module/End Module` |
| `&`, `&mut` borrowing | `Select Case/End Select` |
| `Set` for borrowing | |
| `clone()` for explicit copy | |

---

## Error Message Levels

| Symbol | Meaning |
|---|---|
| `✘` | Hard error — will not compile |
| `⚠` | Warning — compiles but you should know |
| `ℹ` | Teaching note — one time explanation |

---

## Program Structure

| VBR | Rust | Notes |
|---|---|---|
| `Function Main()` | `fn main()` | Required entry point |
| `Module/End Module` | `mod` | Explicit, never implicit |
| `Use std::collections::HashMap` | `use std::collections::HashMap` | std library |
| `Use rand 0.8` | `Cargo.toml` + `use rand` | External crate |
| No module — `Main` only | `main.rs` no modules | Valid minimal program |

---

## Data Types

| VBA | Rust | Notes |
|---|---|---|
| `Integer` | `i16` | Fixed size — copies freely |
| `Long` | `i32` | Fixed size — copies freely |
| `LongLong` | `i64` | Fixed size — copies freely |
| `Single` | `f32` | Fixed size — copies freely |
| `Double` | `f64` | Fixed size — copies freely |
| `Boolean` | `bool` | Fixed size — copies freely |
| `Byte` | `u8` | Fixed size — copies freely |
| `Date` | `i64` | Fixed size — copies freely. ⚠ No date semantics |
| `String` | `String` / `&str` | Unknown size — ownership rules apply |
| `Currency` | ✘ Hard error | Use f64 or i64 explicitly |
| `Variant` | ✘ Hard error | Rust requires explicit types |

---

## Assignment Rules

| VBR | Rust | Notes |
|---|---|---|
| `Dim a As Long = b` | `let a: i32 = b` | Fixed size, copies freely |
| `Dim a As Double = b` | `let a: f64 = b` | Fixed size, copies freely |
| `Dim a As String = "hello"` | `let a: &str = "hello"` | Literal fine — fixed size |
| `Dim a As String = b` | ✘ Hard error | Unknown size — see error |
| `Dim a As String = b.clone()` | `let a: String = b.clone()` | Explicit copy |
| `Set a = b` | `let a = &b` | Immutable borrow |
| `Set Mut a = b` | `let a = &mut b` | Mutable borrow |
| `Dim a As Vec<T> = b` | ✘ Hard error | Unknown size — see error |
| `Dim a As HashMap<K,V> = b` | ✘ Hard error | Unknown size — see error |
| `Dim a As MyStruct = b` | ✘ Hard error | Unknown size — see error |

---

## Unknown Size Error Message

```
✘ Cannot assign 'b' to 'a' directly.

  Unlike integers or doubles, [Type] is not a fixed size —
  it can grow to any length. Rust won't silently copy something
  of unknown size. You need to be explicit:

  Set a = b                    ' borrow — a looks at b, no copy made
  Set Mut a = b                ' mutable borrow — a can modify b
  Dim a As [Type] = b.clone()  ' explicit copy — you are asking
                                ' for a copy knowing it has a cost

  The same rule applies to any type that can grow to an unknown
  size. Fixed size types like Long, Double and Boolean copy
  freely because Rust knows exactly how big they are.
```

---

## Variables & Scope

| VBA | VBR/Rust | Notes |
|---|---|---|
| `Dim x As Long` | `let x: i32` | |
| `Dim x As Long = 5` | `let x: i32 = 5` | |
| `Dim x As New HashMap<String, Long>` | `let mut x: HashMap<String, i32> = HashMap::new()` | ℹ mut added automatically |
| `Private` at module level | Private by default | |
| `Public` at module level | `GlobalVariables` struct | ⚠ Warning once |
| Mutable globals | Generated | ⚠ Threading caveat noted |
| `Option Base 1` | ✘ Hard error | Rust is always zero indexed |
| `Set a = b` | `let a = &b` | Immutable borrow |
| `Set Mut a = b` | `let a = &mut b` | Mutable borrow |
| `With/End With` | ✘ Hard error | Use variable name directly |

---

## Procedures

| VBA | VBR/Rust | Notes |
|---|---|---|
| `Sub` | ✘ Hard error | Use Function with no return type |
| `Function MyFunc()` | `fn my_func()` | |
| `Function MyFunc() As Long` | `fn my_func() -> i32` | |
| `ByVal x As Long` | `x: i32` | |
| `ByRef x As Long` | `x: &mut i32` | |
| `ByVal x As String` | `x: &str` | |
| `ByRef x As String` | `x: &mut String` | |
| Unspecified non-primitive | ✘ Hard error | |
| `FunctionName = value` | Implicit return | |
| `Return value` | Implicit return | |
| `PascalCase` name | ⚠ Warning + rename | snake_case enforced |

---

## Error Handling

| VBA/VBR | Behaviour | Rust output |
|---|---|---|
| `On Error GoTo` | ✘ Hard error | Full explanation given |
| `As Result<Type>` | Allowed | `-> Result<Type, String>` |
| `Return Ok(value)` | Allowed | `return Ok(value)` |
| `Return Err("msg")` | Allowed | `return Err("msg".to_string())` |
| `Match/Case Ok/Case Err` | Allowed | `match` / `Ok(v)=>` / `Err(e)=>` |
| `.Unwrap()` | ⚠ Allowed with warning | Training wheels |
| `?` operator | Carried over directly | |
| Ignoring a Result | ✘ Hard error | |

---

## Control Flow

| VBA | Rust | Notes |
|---|---|---|
| `If/ElseIf/Else/End If` | `if/else if/else` | Single line expanded to block |
| `Select Case` no `Case Else` | ✘ Hard error | Match must be exhaustive |
| `Select Case/Case/Case Else` | `match` | |
| `Case 2, 3` | `2 \| 3` | |
| `Case 4 To 10` | `4..=10` | |
| `Case Else` | `_` | |
| `For i = 1 To 10` | `for i in 1..=10` | |
| `For i = 0 To 20 Step 2` | `(0..=20).step_by(2)` | |
| `For i = 10 To 1 Step -1` | `(1..=10).rev()` | |
| `Do While` | `while` | |
| `Do Until` | `while !` | Condition inverted |
| `Exit For/Do` | `break` | |
| `Continue` | `continue` | VBR extension over VBA |

---

## Strings

| VBA | Rust | Notes |
|---|---|---|
| `&` concatenation | `format!()` | Always, avoids ownership issues |
| `Len(s)` | `s.len()` | |
| `Left(s, 3)` | `&s[..3]` | |
| `Right(s, 3)` | `&s[s.len()-3..]` | |
| `Mid(s, 2, 3)` | `&s[1..4]` | ⚠ 1-indexed adjusted, warn once |
| `UCase(s)` | `s.to_uppercase()` | |
| `LCase(s)` | `s.to_lowercase()` | |
| `Trim(s)` | `s.trim()` | |
| `InStr(s, "x")` | `s.find("x")` | ℹ Returns Option — teaching moment |
| `Replace(s, "a", "b")` | `s.replace("a", "b")` | |
| `Str(x)` | `x.to_string()` | |
| `Val(s)` | `s.parse::<T>()` | ℹ Returns Result — teaching moment |

---

## Arrays — 1D

| VBA/VBR | Behaviour | Rust output |
|---|---|---|
| `Dim x(10) As Long` | ⚠ Size not upper bound | `[i32; 10]` |
| `Dim x() As Long` | Allowed | `Vec<i32>` |
| `Dim x As New Vec<Long>` | Preferred dynamic syntax | `Vec<i32>` |
| `ReDim x(10)` | ⚠ Data lost | `vec![0; 10]` |
| `ReDim Preserve x(20)` | Allowed | `.resize(20, 0)` |
| `x(0)` for access | ✘ Hard error | Use .get() |
| `x[0]` for access | ✘ Hard error | Use .get() |
| `x.get(0)` | Allowed | `.get(0).ok_or(...)` |

---

## Arrays — 2D

| VBA/VBR | Behaviour | Rust output |
|---|---|---|
| `Dim grid(10, 20) As Long` | ⚠ Size not upper bound | `[[i32; 20]; 10]` |
| `Dim grid(,) As Long` | Allowed dynamic | `Vec<Vec<i32>>` |
| `ReDim grid(10, 20)` | ⚠ Data lost | `vec![vec![0; 20]; 10]` |
| `ReDim Preserve grid(10, 30)` | ⚠ Last dimension only | `row.resize(30, 0)` |
| `grid(2, 3)` for access | ✘ Hard error | Use .get() |
| `grid[2][3]` for access | ✘ Hard error | Use .get() |
| `grid.get(2, 3)` | Allowed | `.get(2).and_then(row.get(3)).ok_or(...)` |

---

## Structs

| VBA | VBR/Rust | Notes |
|---|---|---|
| `Type/End Type` | `struct` | Private by default |
| `Public Type` | `pub struct` | |
| `Public Name As String` | `pub name: String` | |
| `Private Name As String` | `name: String` | |
| Declare then assign fields | ✘ Hard error | Must initialise fully at creation |
| `Dim p As Person = Person { name: "Alice", age: 42 }` | Allowed | Full initialisation |
| `Me` | `self` | |
| `Function Person.Method()` | `impl Person { fn method(&self) }` | |
| `PascalCase` field | ⚠ Warning + rename | snake_case enforced |

---

## Constants

| VBA | Rust | Notes |
|---|---|---|
| `Const MAX As Long = 100` | `const MAX: i32 = 100` | |
| `Public Const MAX As Long = 100` | `pub const MAX: i32 = 100` | |
| `PascalCase` constant | ⚠ Warning + rename | SCREAMING_SNAKE_CASE enforced |
| User defined `PI` | ⚠ Suggest `std::f64::consts::PI` | |

---

## Comments

| VBA | Rust | Notes |
|---|---|---|
| `' comment` | `// comment` | Clean mapping |

---

## Maths Functions

| VBA | Rust | Notes |
|---|---|---|
| `Sqr(x)` | `x.sqrt()` | |
| `Abs(x)` | `x.abs()` | |
| `x ^ 2` | `x.powi(2)` | |
| `x ^ 2.5` | `x.powf(2.5)` | |
| `Int(x)` | `x.floor()` | |
| `Round(x)` | `x.round()` | |
| `Sin(x)` | `x.sin()` | |
| `Cos(x)` | `x.cos()` | |
| `Tan(x)` | `x.tan()` | |
| `Log(x)` | `x.ln()` | |
| `Exp(x)` | `x.exp()` | |
| `Rnd()` | ✘ Hard error + snippet | Use rand crate |

---

## I/O

| VBA | Rust | Notes |
|---|---|---|
| `Debug.Print x` | `println!("{}", x)` | |
| `Debug.Print "text" & x` | `println!("text {}", x)` | |
| `InputBox("prompt")` | `stdin().read_line()` | ℹ CLI equivalent |

---

## Collections — HashMap

| VBR | Rust | Notes |
|---|---|---|
| `Dim x As New HashMap<String, Long>` | `let mut x: HashMap<String, i32> = HashMap::new()` | ℹ VBA equivalent is Scripting.Dictionary |
| `x.insert("key", value)` | `x.insert("key".to_string(), value)` | ℹ to_string() automatic |
| `x.get("key")` | `x.get("key").ok_or(...)` | Returns Result |
| `x.contains_key("key")` | `x.contains_key("key")` | Clean mapping |
| `x.remove("key")` | `x.remove("key")` | Clean mapping |
| `For Each k, v In x` | `for (k, v) in &x` | ℹ First look at iterators |

---

