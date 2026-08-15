# Bust Error Handling: New Model 02

## Summary

Bust should treat error propagation as an implicit language feature.

The programmer should not normally have to write `.unwrap()`, `?`, `Result<T, E>`, or similar Rust error-handling machinery at every operation that can fail.

Instead, the default Bust rule should be:

> **Errors propagate automatically unless they are explicitly handled at the call site.**

This preserves the important idea behind Rust's `Result` model while removing repetitive syntax that is particularly distracting in a VB-style language.

The corresponding implementation model is:

- Bust functions are internally fallible.
- Ordinary Bust calls implicitly behave like Rust calls followed by `?`.
- Bust variables contain successful values, not `Result` wrappers.
- Errors can only be intercepted syntactically as part of the call that produced them.
- Inline Rust sees ordinary Bust variables, while direct calls to Bust functions expose their real Rust `Result` types.
- `Raw` suppresses automatic propagation and returns the underlying `Result<T, E>` as a value.

---

## The Problem

A large amount of ordinary Rust code involves operations that return `Result`:

```rust
let text = std::fs::read_to_string(path).unwrap();
let n = text.trim().parse::<i32>().unwrap();
```

For experienced Rust programmers this is familiar, but in Bust it creates two problems.

First, it produces a great deal of visual noise. From a VB perspective, repeatedly writing `.unwrap()` after ordinary library operations can make otherwise simple code look almost comically ceremonial.

Second, it obscures the useful lesson.

The important Rust concept is not:

> "You must type `.unwrap()` after lots of things."

The important concept is:

> "Operations can fail, and failure is represented explicitly."

Bust should preserve the second idea without forcing the first.

---

## Default Bust Semantics

A normal Bust statement such as:

```vb
text = File.ReadAllText(path)
```

should mean:

> Perform the operation. If it succeeds, assign the successful value to `text`. If it fails and the error is not explicitly handled here, propagate the error from the current function.

Conceptually, Bust would generate Rust similar to:

```rust
let text = std::fs::read_to_string(path)?;
```

rather than:

```rust
let text = std::fs::read_to_string(path).unwrap();
```

This distinction is important.

`unwrap()` means:

> If this operation fails, panic here.

`?` means:

> If this operation fails, propagate the error to my caller.

The second behaviour is the better default for Bust.

---

## All Bust Functions Are Implicitly Fallible

To make automatic propagation work consistently, Bust functions and procedures should normally be considered fallible internally.

The programmer should not have to declare this.

For example:

```vb
Function LoadNumber(path As String) As Integer
    text = File.ReadAllText(path)
    Return Integer.Parse(text)
End Function
```

could compile approximately to:

```rust
fn load_number(path: &str) -> anyhow::Result<i32> {
    let text = std::fs::read_to_string(path)?;
    let n = text.trim().parse::<i32>()?;
    Ok(n)
}
```

The visible Bust return type remains:

```vb
As Integer
```

because `Integer` describes the successful value produced by the function.

The possibility of failure is part of Bust's normal execution model and therefore does not need to appear in every function signature.

---

## Bust Variables Are Not `Result` Values

This is an important consequence of the model.

If every Bust function returns a Rust `Result<T>`, that does **not** mean every Bust variable must itself contain a `Result<T>`.

For example:

```vb
a = MyFunc()
b = a + 1
```

can compile to:

```rust
let a = my_func()?;
let b = a + 1;
```

`my_func()` returns:

```rust
anyhow::Result<i32>
```

but `?` immediately does one of two things:

- on success, extracts the `i32` and assigns it to `a`;
- on failure, returns the error from the current function.

Therefore `a` is simply an `i32`.

This is crucial for keeping Bust simple and for making inline Rust pleasant to use.

The implicit error channel exists between function calls. It does not infect ordinary local values.

---

## Calling Other Bust Functions

Suppose another Bust function calls `LoadNumber`:

```vb
Function DoubleNumber(path As String) As Integer
    n = LoadNumber(path)
    Return n * 2
End Function
```

The generated Rust might be:

```rust
fn double_number(path: &str) -> anyhow::Result<i32> {
    let n = load_number(path)?;
    Ok(n * 2)
}
```

The error automatically moves upward through the call chain.

The Bust programmer does not have to repeatedly acknowledge the same failure:

```text
read file -> parse number -> LoadNumber -> DoubleNumber -> Main
```

Each level simply propagates the error unless that particular call is explicitly handled.

---

## Explicit Error Handling: `If Error`

Bust should not use `Try/Catch` for ordinary recoverable errors.

`Try/Catch` suggests an exception model in which errors are thrown independently of return values.

That is not the model Bust is trying to teach.

Instead, an error should only be interceptable as part of the exact call that produced it.

For example:

```vb
a = If Error MyFunc() Then
    Print Error.Message
    Return
End If
```

This means:

1. Call `MyFunc()`.
2. If it succeeds, assign its successful value to `a` and skip the block.
3. If it fails, enter the `If Error` block.
4. Inside that block, `Error` refers to the error produced by this particular call.
5. Outside the block, that `Error` value does not exist.

Conceptually this maps to Rust:

```rust
let a = match my_func() {
    Ok(value) => value,
    Err(error) => {
        println!("{}", error);
        return Ok(());
    }
};
```

This is deliberately closer to Rust's `Result`/`match` model than to exceptions.

---

## `Error` Is Contextual, Not Global

There should be no persistent or global Bust error object analogous to classic VB's `Err`.

This must not be legal:

```vb
a = MyFunc()
DoSomethingElse()

If Error Then
    ...
End If
```

There is no ambient "last error" state to inspect.

`If Error` is only meaningful when syntactically attached to a fallible expression:

```vb
a = If Error MyFunc() Then
    ...
End If
```

The `Error` value belongs solely to that call and solely to that handler block.

This avoids temporal coupling and makes it impossible to accidentally inspect an error left behind by some earlier operation.

---

## The Two Core Forms

The error model can therefore be understood through two very simple forms.

### Propagate automatically

```vb
a = MyFunc()
```

means approximately:

```rust
let a = my_func()?;
```

### Intercept this call

```vb
a = If Error MyFunc() Then
    ...
End If
```

means approximately:

```rust
let a = match my_func() {
    Ok(value) => value,
    Err(error) => {
        ...
    }
};
```

This gives Bust a direct analogue of Rust's two common patterns:

```rust
thing()?
```

and:

```rust
match thing() {
    Ok(value) => ...,
    Err(error) => ...,
}
```

without exposing either syntax in normal Bust code.

---

## Control Flow Inside `If Error`

There is one important compiler rule.

If the call fails, the successful value does not exist.

Therefore:

```vb
a = If Error MyFunc() Then
    Print Error.Message
End If

Print a
```

cannot simply continue unless Bust has defined what value `a` receives.

The cleanest initial rule is:

> **An `If Error` handler attached to a value-producing expression must not fall through unless it establishes a valid replacement value.**

For the first implementation, Bust could simply require the handler to leave the current control flow using something such as:

```vb
Return
```

or:

```vb
Exit Sub
```

For example:

```vb
a = If Error MyFunc() Then
    Print Error.Message
    Return
End If

Print a
```

is well-defined because execution only reaches `Print a` when `MyFunc()` succeeded.

A later language slice could add explicit fallback-value syntax if useful.

---

## Main

Bust should own the top-level error behaviour.

For example:

```vb
Sub Main()
    text = File.ReadAllText("data.txt")
    Print text
End Sub
```

could generate something structurally similar to:

```rust
fn vbr_main() -> anyhow::Result<()> {
    let text = std::fs::read_to_string("data.txt")?;
    println!("{text}");
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
```

This gives Bust control over how an unhandled program error is presented.

It also means generated Rust does not need to panic simply because the Bust programmer did not explicitly handle an error.

---

## Why `anyhow` Fits This Model

`anyhow` is a plausible implementation choice for Bust-generated Rust.

A Bust function may call many unrelated libraries:

- file I/O,
- parsing,
- networking,
- JSON,
- databases,
- image libraries,
- operating-system APIs,
- Bust runtime functions.

These operations may all have different Rust error types.

Using:

```rust
anyhow::Result<T>
```

allows Bust-generated functions to propagate many different underlying errors through one common internal type.

For example:

```rust
fn do_work() -> anyhow::Result<i32> {
    let text = std::fs::read_to_string("number.txt")?;
    let value = text.trim().parse::<i32>()?;
    Ok(value)
}
```

Both the I/O error and the integer parsing error can propagate naturally.

`anyhow` should therefore be regarded as an implementation convenience, not as part of Bust's visible language semantics.

Bust programmers need not know that `anyhow` is involved unless they inspect or write inline Rust.

---

## Inline Rust

Inline Rust is an important Bust feature, so the error model must interact with it cleanly.

Fortunately, implicit propagation works well here.

Consider:

```vb
a = MyFunc()

Rust
    println!("a = {}", a);
End Rust
```

The generated Rust can be approximately:

```rust
let a = my_func()?;

println!("a = {}", a);
```

Because the implicit `?` has already extracted the successful value, `a` is an ordinary Rust value.

Inline Rust therefore does **not** need to write:

```rust
a.unwrap()
```

when using Bust variables.

This is one of the strongest reasons to resolve the `Result` at Bust call boundaries rather than storing `Result<T>` in Bust variables.

---

## Calling Bust Functions From Inline Rust

The reverse direction is intentionally different.

If inline Rust directly calls a generated Bust function, it is calling the actual Rust function.

Therefore this:

```vb
Rust
    let x = my_func();
End Rust
```

really does give `x` the type:

```rust
anyhow::Result<T>
```

That is appropriate.

Inline Rust is explicitly dropping below Bust's abstraction layer and seeing the Rust machinery that Bust normally hides.

Because the surrounding generated Bust function itself returns a `Result`, idiomatic inline Rust can normally write:

```rust
let x = my_func()?;
```

or handle the result explicitly:

```rust
let x = match my_func() {
    Ok(value) => value,
    Err(error) => {
        ...
    }
};
```

This creates a useful and consistent boundary:

> **Bust syntax implicitly propagates errors. Inline Rust uses Rust's explicit `Result` semantics.**

---

## Inline Rust as a Teaching Bridge

This interaction makes the new model particularly suitable for Bust's teaching goal.

A learner may first write:

```vb
a = MyFunc()
```

and understand:

> If `MyFunc` fails, this function fails too.

The generated Rust reveals:

```rust
let a = my_func()?;
```

Later, inside inline Rust, the same learner can write:

```rust
let a = my_func()?;
```

themselves.

Similarly, Bust:

```vb
a = If Error MyFunc() Then
    ...
End If
```

can be explained as the Bust equivalent of explicitly matching the Rust `Result`.

So the progression is:

1. Bust implicit propagation.
2. Bust call-site `If Error`.
3. Inline Rust using `?`.
4. Inline Rust using `match`.
5. Full Rust error types and custom handling.

This teaches the semantic model first and reveals the Rust syntax progressively.

---

## `unwrap()` Should Be Exceptional

Bust may still occasionally want to generate `unwrap()` or equivalent behaviour.

But this should represent an explicit assertion:

> This operation must succeed; failure represents a programming error.

That is very different from using `unwrap()` merely because Bust has no other error model.

Normal Bust calls should never require this behaviour.

The default is propagation.

The explicit recovery mechanism is `If Error`.

An explicit "this cannot fail" feature, if Bust ever wants one, should be a separate language construct.

---

## Capturing the Raw `Result<T, E>`

Most Bust code should not manipulate `Result` values directly.

Normally:

```vb
a = MyFunc()
```

means approximately:

```rust
let a = my_func()?;
```

Bust extracts the successful value and automatically propagates any error.

Sometimes, however, the programmer may deliberately want the underlying Rust-style `Result<T, E>` value itself.

Bust should provide the `Raw` keyword for this purpose:

```vb
r = Raw MyFunc()
```

This means approximately:

```rust
let r = my_func();
```

No automatic propagation occurs.

If `MyFunc()` internally returns:

```rust
Result<Integer, SomeError>
```

then `r` is itself a Bust:

```text
Result<Integer, SomeError>
```

value.

`Raw` therefore means:

> **Do not apply Bust's normal result handling to this call. Give me the underlying result value itself.**

This gives Bust three distinct and complementary ways to call a fallible function.

### Normal call: propagate automatically

```vb
a = MyFunc()
```

Conceptually:

```rust
let a = my_func()?;
```

The successful value is assigned to `a`. Failure propagates from the current Bust function.

### Immediate handling: `If Error`

```vb
a = If Error MyFunc() Then
    Print Error.Message
    Return
End If
```

Conceptually:

```rust
let a = match my_func() {
    Ok(value) => value,
    Err(error) => {
        ...
    }
};
```

The error is handled at the exact call site that produced it.

### Capture the result itself: `Raw`

```vb
r = Raw MyFunc()
```

Conceptually:

```rust
let r = my_func();
```

The `Result<T, E>` becomes an ordinary value which the program can store, pass elsewhere, inspect, or handle later.

This makes `Result<T, E>` useful in Bust without making it part of ordinary function signatures or ordinary error handling.

---

## The Bust `Result<T, E>` Type

If Bust exposes Rust's result type, it should expose the complete type:

```text
Result<T, E>
```

rather than the existing halfway form:

```text
Result<T>
```

The error type is part of what a Rust `Result` is.

However, under the new error model, programmers should rarely need to write `Result<T, E>` in normal Bust code.

Ordinary Bust functions still appear as:

```vb
Function LoadNumber(path As String) As Integer
```

even though their generated Rust representation may be:

```rust
fn load_number(path: &str) -> anyhow::Result<i32>
```

A Bust `Result<T, E>` generally appears only when the programmer deliberately asks to preserve the result as data, most naturally through `Raw`:

```vb
r = Raw LoadNumber(path)
```

This distinction is important:

> **Function fallibility is normally implicit in Bust. `Result<T, E>` is explicit only when the result itself is wanted as a value.**

This avoids teaching a Bust-specific imitation of Rust's `Result` while still allowing advanced code to work directly with the real concept.

---

## `Raw` and Inline Rust

`Raw` also makes the boundary between Bust and inline Rust especially clear.

These two pieces of Bust:

```vb
a = MyFunc()
r = Raw MyFunc()
```

produce conceptually:

```rust
let a = my_func()?;
let r = my_func();
```

Therefore:

- `a` is the successful value `T`;
- `r` is the complete `Result<T, E>`.

Inline Rust can use either naturally:

```vb
Rust
    println!("value = {:?}", a);

    match r {
        Ok(value) => println!("raw result contained {:?}", value),
        Err(error) => println!("raw result contained error: {}", error),
    }
End Rust
```

This preserves Bust's useful inline-Rust feature without introducing hidden `unwrap()` calls on Bust variables.

It also provides a particularly direct teaching bridge:

```vb
r = Raw MyFunc()
```

can be explained as:

> "Here Bust is letting you see the `Result` that it would normally consume with an implicit `?`."


## Interaction With `Option`

`Option` requires separate treatment from `Result`.

A missing value is not always an error.

For example, looking up a dictionary key may legitimately mean:

```text
the key was absent
```

rather than:

```text
the program failed
```

Therefore Bust should not blindly convert every Rust `Option<T>` into an automatically propagated error.

The standard library or binding layer should classify operations according to their Bust semantics.

Some APIs may naturally expose optional values.

Others may expose convenience operations where absence is considered failure.

This is separate from the core `Result` propagation model.

---

## Compiler Model

A useful implementation model is:

```text
Bust:
    Function F(...) As T

Generated Rust:
    fn f(...) -> anyhow::Result<T>
```

and:

```text
Bust:
    Sub F(...)

Generated Rust:
    fn f(...) -> anyhow::Result<()>
```

A normal Bust call:

```vb
x = F()
```

becomes approximately:

```rust
let x = f()?;
```

A handled Bust call:

```vb
x = If Error F() Then
    ...
End If
```

becomes approximately:

```rust
let x = match f() {
    Ok(value) => value,
    Err(error) => {
        ...
    }
};
```

A normal Bust return:

```vb
Return value
```

becomes approximately:

```rust
return Ok(value);
```

and reaching the end of a `Sub` becomes:

```rust
Ok(())
```

The compiler therefore carries the error channel implicitly while ordinary Bust variables remain plain values.

---

## Important Consequence

This approach means fallibility does not infect the visible Bust type system.

In Rust:

```rust
fn load() -> Result<String, Error>
```

the error channel is explicitly part of the function type.

In Bust:

```vb
Function Load() As String
```

the visible type describes the value produced when the function succeeds.

Failure remains real, but it travels on an implicit language-level error channel.

This is a deliberate simplification.

It does not hide errors so much as move the repetitive propagation machinery into the compiler.

---

## Design Principles

The model can be summarized with five rules:

> **1. Every Bust function may fail internally.**

> **2. Errors propagate automatically unless explicitly intercepted.**

> **3. An error may only be intercepted at the exact call site that produced it.**

> **4. Successful Bust variables are ordinary values, not `Result` wrappers.**

> **5. `Raw` explicitly suppresses automatic propagation when the underlying `Result<T, E>` is wanted as data.**

Or more compactly:

> **Bust makes failure explicit as a concept, propagation implicit as syntax, and handling local to the producing call.**

This gives Bust cleaner code, avoids ubiquitous `.unwrap()`, produces more idiomatic generated Rust, preserves inline Rust as a clean escape hatch, and provides a direct teaching path into Rust's `Result`, `?`, and `match` model.
