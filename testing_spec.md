# VBR Testing — Spec

Tests in VBR are **executable specifications**. You write the code and the tests;
a reader verifies your work by reading the tests — a description they can check
against without re-deriving the implementation. That makes `vbr test` less a
language feature than a *trust protocol*: the suite is the contract, and the
runner reports it in the same words you wrote.

(Companion to `language_spec.md`. BUILT 2026-07-11.)

---

## The `Test` block

A `Test "description" … End Test` block is an ordinary statement body
(Arrange-Act-`Assert`) with a human description — the spec sentence.

```vb
Test "a blinker oscillates"
    Dim g As Vec<Long> = Blinker()
    g = Life.StepLife(g)
    Assert g = Vertical()
End Test
```

The description is what `vbr test` prints, so write it as the promise being made
("a dead cell with three neighbours is born"), not as a label.

## `Assert`

`Assert <expr>` checks a condition. The **operator picks the Rust assertion**, so
the `=`/`<>` you'd write anyway give operand-level failure messages:

| VBR | Rust | On failure shows |
|-----|------|------------------|
| `Assert a = b`  | `assert_eq!(a, b)` | `left` and `right` values |
| `Assert a <> b` | `assert_ne!(a, b)` | the equal values |
| `Assert cond`   | `assert!(cond)`    | just the location |

`=` inside an `Assert` is equality (as everywhere in an expression), never
assignment. A block may hold several `Assert`s; the first to fail reports.

## Where tests live: `<module>.test.vbr`

In a project, gather a module's tests in a **`<module>.test.vbr` file beside it**
— `life.vbr` holds the code, `life.test.vbr` holds its specs. The suite then
reads as a document you scan top-to-bottom as the module's contract, rather than
test blocks scattered through implementation. A test file calls the code by its
qualified name (`Life.StepLife(g)`), so the function under test must be `Public`
— you test the public surface, which is exactly the contract worth pinning.

A `.test.vbr` file is compiled **only** by `vbr test`. `vbr run` and `vbr build`
skip it entirely, so logic that exists only to be exercised by tests never counts
as unused in the app build.

`Test` blocks may also sit inline in any `.vbr` file (next to the code, Rust's
`#[cfg(test)]` style); the dedicated `.test.vbr` file is the recommended home.

## Running: `vbr test [path]`

`vbr test` builds the project with its tests active and runs them, reporting each
by description:

```
  ✓ a live cell with two neighbours survives
  ✗ a lone cell wrongly expected to survive
      expected left == right
      left:  false
      right: true
      at life.test.vbr:6

  1 passed, 1 failed
```

A failure shows the operand values and the **`.vbr` line** (translated from the
generated Rust, like every other VBR error). The process exits non-zero when any
test fails, so `vbr test` drops into CI.

## Under the hood

Each `Test` lowers to a Rust `#[test] fn` (a slug of the description) in a
`#[cfg(test)] mod`, and `vbr test` runs `cargo test` — so tests get real
isolation, run in parallel, and panics are caught, all for free, while the output
is translated back to your descriptions. It also means a VBR test *is* a Rust
test: the same discipline, one layer down.

## Scope

- **Logic, not surfaces.** You test pure functions and a module's public API
  (`StepLife`, `ParseRule`), not GUI/TUI/web rendering — which is also the lesson:
  keep logic separate from the interface so it's testable.
- **Deferred:** shared setup/fixtures (use a helper `Function` for now), custom
  failure messages (`Assert cond, "why"`), and test-only helper visibility. Add
  them when a real suite needs them.
