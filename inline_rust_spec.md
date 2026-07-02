# VBR Inline Rust — Spec

The escape hatch *and* the learning ramp. Inline Rust is VBR's "inline assembly":
just as 80s languages let you drop into `asm { … }` for the bits the high-level
language couldn't express, VBR lets you drop into a block of real Rust — and only
a plain value crosses back into VBR.

This is how VBR calls **arbitrary Rust crates** without dumping Rust's complexity
(traits, generics, lifetimes, ranges, macros) onto the VBR programmer: that
complexity stays *sealed inside the block*.

---

## The shape

```vb
Dim sum As Long = Rust
    a + b
End Rust
```
→
```rust
let sum: i32 = {
    a + b
};
```

A `Rust … End Rust` block is a **Rust block expression**. You assign it to a
typed `Dim`; the declared type (`As Long`) anchors what the block must produce,
so the Rust compiler checks your block against it.

---

## The two rules (that's all there is)

1. **Passing IN is automatic.** The block is spliced into the same function, so
   every VBR variable you've declared is already in scope — just write its name.
   No `pass` keyword, no re-declaration. (One catch: inside the block you use the
   *Rust* spelling — a VBR name is simply its lowercase self, so `myValue` is
   `myvalue` — you're writing Rust there.)

2. **Passing OUT is the last line with no semicolon.** In Rust, a block's value
   is its final expression *when you leave the semicolon off*:

   ```rust
   let sum = { a + b };    // sum = 12   — no `;` → "a + b" IS the value
   let sum = { a + b; };   // nothing    — the `;` discards it
   ```

   So **no trailing semicolon = "this is my answer."**

This is the same mechanic VBR already uses for `Return x + y` (which compiles to a
tail `x + y` with no semicolon). Inline Rust just exposes the trick you've been
benefiting from all along.

---

## Calling a crate

`Use crate version` declares the dependency (it writes the `Cargo.toml` line).
The actual calling happens in the block, where traits/ranges/etc. are all legal:

```vb
Use rand 0.8

Function Main()
    Dim x As Double = Rust
        use rand::Rng;
        rand::thread_rng().gen_range(0.0..1.0)
    End Rust

    Debug.Print "random: " & x
End Function
```

The crate type (`ThreadRng`), the trait (`Rng`), the range (`0.0..1.0`) — all
sealed in the block. What comes out is a plain `Double`. VBR never has to *name*
a crate type or understand a trait.

---

## The rules that keep it simple and honest

- **Block-expression form** (assign the result to a typed `Dim`) is the primary
  shape. Likely the only shape — it keeps "read in, return out" crisp.
- **Data leaves only through the return value.** A block doesn't reach in and
  mutate your variables. Need two results? Return a tuple and destructure with
  the existing `Dim a, b = …`:
  ```vb
  Dim q, r = Rust
      (n / d, n % d)    ' Rust inside the block — returns a tuple
  End Rust
  ```
  → `let (q, r) = { (n / d, n % d) };`
- **Crate objects live and die inside the block.** You don't hold a `ThreadRng`
  or a `reqwest::Client` in a VBR variable — do the whole interaction in one
  block and return the simple value. (Only VBR-expressible values cross back.)
- **The block handles its own imports** (`use rand::Rng;` inside it). Self-contained.

---

## Where it sits in the architecture

This collapses the dependency story into two clean tiers:

1. **Curated wrappers (the stdlib)** — the comfortable, no-Rust surface for common
   needs (files, json, dates, regex, …). Grows over time.
2. **Inline Rust + `Use`** — the escape hatch for *everything else*. Crate types
   and Rust idioms stay sealed; only values cross back.

The fragile `.`→`::` "pass-through for arbitrary crates" idea is **dropped** — it
only half-worked, and inline Rust does the job properly. (`.`→`::` remains only
for the stdlib, which we control.)

---

## Why it's a great teaching ramp

A VBer who needs `rand` writes three lines of real Rust, sees it work, and gets a
result back into familiar territory. Over time the blocks grow and the VBR
shrinks — which is the whole point. It's the most natural on-ramp from "VB with
training wheels" to "I'm writing Rust now."

---

## Implementation note

The lexer must treat a `Rust … End Rust` block as **verbatim text** — do NOT
tokenise the Rust inside (it would choke on `::`, `<>`, `|`, lifetimes, macros).
One deliberate carve-out: on seeing `Rust` at the start of a statement /
initialiser, swallow raw source until `End Rust` and hand it back as one opaque
chunk. The transpiler then splices it inside `{ … }`.

---

## Open / leaning

- **Delimiter:** `Rust … End Rust` (reads like VB; pairs with `End Function`).
- **Forms:** block-expression only (lean), vs. also a side-effect statement form.
- **`Use crate version`:** declares deps → Cargo.toml. Needs the cargo-project
  run mode to actually build/run (separate, still to design).
