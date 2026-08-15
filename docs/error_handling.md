# Error handling in Bust

There is no `On Error GoTo`. Failure is a value, not an exception. Ordinary Bust
never writes `Result`, `?`, `Ok`, `Err`, or `.Unwrap()`.

**Rule:** errors propagate automatically unless intercepted at the producing call.

The type you declare is the success value. The compiler wraps every user
function as `Result<T, String>` behind the scenes. A Bust variable holds `T`.

---

## The four call forms

| Form | Meaning |
|------|---------|
| `a = MyFunc()` | propagate |
| `a = MyFunc() Handle err` … `End Handle` | intercept this call |
| `a = Raw MyFunc()` | take the `Result` as data |
| `RaiseError "…"` | fail from here |

```vb
Function Divide(ByVal a As Long, ByVal b As Long) As Long
    If b = 0 Then RaiseError "cannot divide by zero"
    Return a / b
End Function

Function Main()
    Dim q As Long = Divide(10, 2)             ' q is a Long

    Dim n As Long = Divide(7, 0) Handle err   ' intercept this call
        Debug.Print err
        Return
    End Handle

    Debug.Print n
End Function
```

`Return n` is success. `RaiseError` is failure. Infallible builtins (`Trim`,
`Abs`, `+`) stay infallible. User functions are always fallible.

`Option` is separate. Absence is not failure. A missing `HashMap` key is
`None`, not an error — use `If x Is Some(v) Then`, `Match`, or `.Unwrap_Or`.

---

## `Handle err`

Postfix on **one call**, at statement level. The bound name (conventionally
`err`) is a `String` and exists only inside the block.

```vb
text = FileSystem.Read(path) Handle err
    Debug.Print err
    Return
End Handle

FileSystem.Delete(oldPath) Handle err         ' swallow: must be explicit
    ' already gone is fine
End Handle
```

**Value form** (`a = F() Handle err`): every path must diverge (`Return`,
`Exit For`, `Continue`, `RaiseError`) or produce a replacement `T`.

**Statement form** (`F() Handle err`): falling through consumes the error.
Bare `F()` still propagates.

`Handle` on an infallible call is a teaching error. Nested fallible calls
inside that call are also a teaching error — intercept each call separately.

---

## `Raw`

Drops through the Bust layer and yields the `Result` as a value. Inspect it
with `Match`. `Handle` and `Raw` do not combine.

```vb
Dim r As Result<Long> = Raw Divide(10, 2)
Match r
    Ok(n) => Debug.Print n
    Err(e) => Debug.Print e
End Match
```

Ordinary functions still declare `As Long`, not `As Result<Long>`.

---

## What leaves ordinary Bust

`?`, `As Result<T>` on a function, `Return Ok(…)`, `Return Err(…)`, `.Unwrap()`.

`Match` stays for `Option`, enums, `Await`, and `Raw` results. `On Error` stays
rejected; the diagnostic points here.

---

## Where an unhandled error goes

| Boundary | Unhandled error |
|----------|-----------------|
| `Main` | print, exit 1 |
| Event, timer, `Await` continuation, Godot `On …` | event ends, app keeps running |

A failed `Http.Get` in an event hits that continuation sink unless you
`Handle` it. Host I/O (`terminal.draw()?`) stays fatal and is separate.

The teaching ladder: a Bust call → `Handle err` → inline `?` → inline `match`
→ the real `Result` via `Raw`.
