## VBR V1.1 Feature Additions


## Pattern Matching — Full Support

| VBR | Rust | Notes |
|---|---|---|
| `Case Person { name, age }` | `Person { name, age } =>` | Struct destructuring — explicit struct name required |
| `Case []` | `[] =>` | Empty Vec |
| `Case [first]` | `[first] =>` | Single element Vec |
| `Case n If n < 0` | `n if n < 0 =>` | Guard conditions |
| `Case _` | `_ =>` | Wildcard |
| No wildcard | ✘ Hard error | Match must be exhaustive |

---

## Iterators

| VBR | Rust | Notes |
|---|---|---|
| `.filter(\|x\| condition)` | `.filter(\|x\| condition)` | Keep matching elements |
| `.map(\|x\| expression)` | `.map(\|x\| expression)` | Transform each element |
| `.collect()` | `.collect()` | Materialise into Vec |
| `.sum()` | `.sum()` | Sum all elements |
| `.count()` | `.count()` | Count elements |
| `.any(\|x\| condition)` | `.any(\|x\| condition)` | Any match? |
| `.all(\|x\| condition)` | `.all(\|x\| condition)` | All match? |
| `.first()` | `.first()` | First element → Result |
| `.last()` | `.last()` | Last element → Result |

HashMap iterators:
| VBR | Rust | Notes |
|---|---|---|
| `.filter(\|_, v\| condition)` | `.filter(\|(_, v)\| condition)` | Filter by value |
| `.filter(\|k, _\| condition)` | `.filter(\|(k, _)\| condition)` | Filter by key |

---

## Tuples

| VBR | Rust | Notes |
|---|---|---|
| `Dim x As (Long, Long) = (1, 2)` | `let x: (i32, i32) = (1, 2)` | Declaration |
| `x.0` / `x.1` | `x.0` / `x.1` | Element access |
| `Function F() As (Long, String)` | `fn f() -> Result<(i32, String), String>` | Multiple return values |
| `Dim a, b = F()` | `let (a, b) = f()?` | Destructuring |
| `Case (a, b)` | `(a, b) =>` | Pattern matching |
| `Case (x, 0)` | `(x, 0) =>` | Partial matching |

---

## Option\<T\>

| VBR | Rust | Notes |
|---|---|---|
| `As Option<Type>` | `Option<Type>` | Return type |
| `Return Some(value)` | `return Some(value)` | Has value |
| `Return None` | `return None` | Absent |
| `Case Some(x)` | `Some(x) =>` | Pattern match |
| `Case None` | `None =>` | Pattern match |
| `x.ok_or("msg")` | `x.ok_or("msg")` | Option → Result |
| `x.ok()` | `x.ok()` | Result → Option |
| `?` on Option | `?` | Propagate None |
| `.Unwrap()` on Option | `.unwrap()` | ⚠ Training wheels |

V0 code unchanged — Option never forced on existing code.

---

## Format Strings

| VBR | Rust | Notes |
|---|---|---|
| `Format(x, "#,###.00")` | ✘ Hard error + example | Use num_format crate |

