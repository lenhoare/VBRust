# Bust Error Handling — Final Model

Normative. Replaces `language_spec.md` §8.

**Rule:** errors propagate automatically unless intercepted at the producing call.

Failure is a value, not an exception. Propagation is implicit. Handling is local. Ordinary Bust never writes `Result`, `?`, `Ok`, `Err`, or `.Unwrap()`.

---

## 1. Implicit channel

Every user `Function` / `Sub` is internally fallible. The visible return type is the success value.

```vb
Function LoadNumber(path As String) As Integer
    text = File.ReadAllText(path)
    Return Integer.Parse(text)
End Function
```

Generated Rust (approximately):

```rust
fn load_number(path: &str) -> Result<i32, String> {
    let text = std::fs::read_to_string(path)?;
    Ok(text.trim().parse::<i32>()?)
}
```

- A normal call is an implicit `?`. The Bust variable holds `T`, not `Result<T>`.
- `Return value` → `return Ok(value)`. End of a `Sub` → `Ok(())`.
- Infallible builtins stay infallible (`Trim`, `Abs`, `+`). User functions are always `Result`.
- The channel is `Result<T, String>` so `vbr run file.vbr` stays bare `rustc` (no anyhow/Cargo for hello world). Stdlib already returns `Result<T, String>`.

`Option` is separate. Absence is not failure. Do not auto-`?` `Option`.

---

## 2. The four call forms

| Form | Meaning | Rust analogue |
|------|---------|----------------|
| `a = MyFunc()` | propagate | `let a = my_func()?;` |
| `a = MyFunc() Handle err` | intercept this call | `match my_func() { Ok(a) => a, Err(err) => … }` |
| `a = Raw MyFunc()` | take the `Result` as data | `let a = my_func();` |
| `RaiseError "…"` | fail from here | `return Err(…);` |

```vb
a = MyFunc()                              ' propagate

a = MyFunc() Handle err                   ' intercept
    Print err.Message
    Return
End Handle

r = Raw MyFunc()                          ' Result<T, E> as a value

If b = 0 Then RaiseError "cannot divide by zero"
```

No `Try`/`Catch` region. No global `Err`. `On Error` stays rejected.

---

## 3. `RaiseError`

```vb
RaiseError "cannot divide by zero"        ' message
RaiseError err                            ' re-raise from a Handle block
```

`Return n` is success. `RaiseError` is failure. A string is the normal Bust path; `err` is the object bound by `Handle`.

---

## 4. `Handle err`

Postfix on **one call**, at statement level. Binds a block-scoped name (conventionally `err`). The name is the programmer’s; there is no `Error` value.

```vb
text = File.ReadAllText(path) Handle err
    Print err.Message
    Return
End Handle

File.WriteAllText(path, text) Handle err  ' statement form
    Print err.Message
    Return
End Handle

File.Delete(oldPath) Handle err           ' swallow: must be explicit
    ' already gone is fine
End Handle
```

`err` exists only inside the block. Surface: `.Message` (enough for v1).

**Value form** (`a = F() Handle err`): every path must diverge (`Return`, `Exit For`, `Continue`, `RaiseError`, …) or produce a `T`.

**Statement form** (`F() Handle err`): falling through consumes the error. Bare `F()` still propagates.

`Handle` on an infallible call is a teaching error.

Nested fallible calls are a teaching error — intercept each call separately:

```vb
' ✘  n = Integer.Parse(File.ReadAllText(path)) Handle err

text = File.ReadAllText(path) Handle err
    Print err.Message
    Return
End Handle
n = Integer.Parse(text) Handle err
    Print err.Message
    Return
End Handle
```

**Deferred:** `n = CInt(text) OrElse 0` — fallback `T`, error discarded.

---

## 5. `Raw`

Drops through the Bust layer. `E` is Rust’s real `E`.

```vb
r = Raw LoadNumber(path)                  ' Result<i32, String>
```

`Raw` cannot fail at the Bust layer; it always yields the box. Inspect with `Match` or inline Rust. `Handle` and `Raw` do not combine.

Ordinary Bust functions still declare `As Integer`, not `As Result<…>`. `Result<T, E>` appears only when the programmer asked for the box.

---

## 6. Inline Rust

Bust variables are already unwrapped. Inline Rust that *calls* a Bust function sees the real `Result` and uses `?` / `match`.

```vb
a = MyFunc()
r = Raw MyFunc()
Rust
    println!("{a}");          ' T
    let x = my_func()?;       ' calling Bust from Rust: Result
    match r { … }
End Rust
```

Teaching ladder: Bust call → `Handle err` → inline `?` → inline `match` → real `E` via `Raw`.

---

## 7. Top-level sinks

Same channel everywhere. Only the last catcher changes.

| Boundary | Unhandled error |
|----------|-----------------|
| `Main` | print, exit 1 |
| `State` / `init()` | print `could not start: …`, exit 1 |
| `Event`, timer, `Await` continuation, Godot `On …` | event ends, app keeps running |

**Main** (fatal):

```rust
fn vbr_main() -> Result<(), String> { … }

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
```

**Events** (non-fatal): the body is internally `-> Result<(), String>`. The dispatcher catches `Err`.

- Assignments above the failure stay (no rollback). The failed call does not assign.
- Bare `Return` in an event is `return Ok(())` (fixes Quirk 25).
- Helper `Sub`s propagate into the event with implicit `?`.
- Views stay infallible.

Report an unhandled event error with both:

1. `Log.Error`, including the event name (Godot: also `godot_error!`).
2. Runtime chrome — not a Bust `Err` object: Screen bottom bar; Window/Page banner (Page also `console.error`). Cleared when the next event starts.

To show a failure on *your* widgets, `Handle err` and store it in state. No default `MsgBox`. No `Event ErrorOccurred`.

Host/runtime I/O (`terminal.draw()?`, stdin died) stays fatal and does not use this sink.

`Await` is an ordinary fallible call. A failed `Http.Get` hits the continuation sink unless `Handle`d. UI rules unchanged: no blocking call without `Await`; one top-level `Await` per event.

---

## 8. What leaves ordinary Bust

`?`, `As Result<T>`, `Return Ok(…)`, `Return Err(…)`, `.Unwrap()`.

`Match` stays for `Option`, enums, and `Raw` results. `On Error` stays rejected; the diagnostic points here.

---

## 9. Compiler sketch

```text
Function F(…) As T     →  fn f(…) -> Result<T, String>
Sub F(…)               →  fn f(…) -> Result<(), String>
x = F()                →  let x = f()?;
x = F() Handle err     →  match f() { Ok(x) => x, Err(err) => { … } }
x = Raw F()            →  let x = f();
Return v               →  return Ok(v);
RaiseError m           →  return Err(…);
end of Sub             →  Ok(())
Event body             →  inner Result, dispatcher is the sink
```
