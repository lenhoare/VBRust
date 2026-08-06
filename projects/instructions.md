# VBR Example-Project Instructions

You are helping build generated example projects for a new programming language
called **VBR: Visual Basic Rust**.

The parent directory home/len/dev/vbrprojects/VBRust contains the VBR language specification. **Read it first and
follow it strictly.** The most relevant specs are:

- `language_spec.md` / `language_reference.md` — the core language.
- `projects_and_run_spec.md` — how a folder becomes a multi-module project.
- `testing_spec.md` — the `Test` / `Assert` harness (see “Testing”, below).
- `stdlib_spec.md` — the standard library, if you use it.

There is also an existing `examples/` folder. **That folder is for reference
only.** `examples/tests.vbr` is a good, minimal model of the test harness.

Your output folder is:

```text
home/len/dev/vbrprojects/VBRust/projects/
```

- Do **not** write generated examples into `examples/`.
- Do **not** modify anything inside `examples/`.
- Do **not** modify any Rust source files.
- **Absolutely do not edit files ending in `.rs`.**
- Do **not** commit anything to git or github.

The purpose of this exercise is to **test the existing VBR transpiler**, not to
improve or change the transpiler itself.

Prefer writing VBR code wherever possible. Avoid inline Rust unless it is
absolutely necessary. The point is to test VBR syntax, VBR semantics, and the
existing VBR standard library.

---

## Project structure

Create a new example project, numbered, under `projects/`:

```text
projects/
  1_constants_and_literals/
  2_simple_receipt/
  3_boolean_gates/
  ...
```
The list of projects is in project_list.md choose the next project that hasnt been attempted in a new directory.

A **folder is a VBR project**: every `.vbr` file in it is a *module* named after
the file (`receipt.vbr` → module `Receipt`). Cross-module calls are **qualified**
(`Receipt.Total(...)`). For every project create:

```text
README.md
main.vbr             ' the entry point: Function Main()
<subject>.vbr        ' the logic under test: Public functions (e.g. receipt.vbr)
<subject>.test.vbr   ' the tests for that module (e.g. receipt.test.vbr)
expected_output.txt  ' exact stdout of running main.vbr
```

Most importantly you should also create notes.md to record your experience with the trqanspiler as you go with any bugs you have found or any mistakes in the literature or any obvious features you feel are omitted.

```text
projects/
notes.md
```

### Why a separate logic module?

The test harness runs a `.test.vbr` file against the **public surface of the
sibling module of the same name** — `receipt.test.vbr` tests `receipt.vbr`. Two
consequences you **must** follow:

1. **Put the code under test in `<subject>.vbr`, not in `main.vbr`.** The entry
   module (`main.vbr`) is *not* reachable from a test file. `main.vbr` should be
   thin: it calls into the logic module and prints.
2. **Mark every function a test calls `Public`** — you test the public contract.

`main.vbr` then reads like:

```vb
Function Main()
    Debug.Print Receipt.Total(100, 3)
End Function
```

and `receipt.vbr` like:

```vb
Public Function Total(ByVal price As Long, ByVal qty As Long) As Long
    Return price * qty
End Function
```

Each `main.vbr` should be a small, runnable VBR program. Keep it deterministic.

---

## Testing — use the real `Test` / `Assert` harness

VBR has a **built-in test harness** (`vbr test`). **Do not** hand-roll PASS/FAIL
helper subs or print your own results — use the language mechanism, which reports
`✓ / ✗` by description, shows the failing operands, and exits non-zero on failure
(so it drops straight into CI).

Write tests as `Test "description" … End Test` blocks in `<subject>.test.vbr`,
calling the module’s **Public** functions by their **qualified** name:

```vb
' receipt.test.vbr — specs for receipt.vbr
Test "quantity multiplies the price"
    Assert Receipt.Total(100, 3) = 300
End Test

Test "zero quantity costs nothing"
    Assert Receipt.Total(100, 0) = 0
    Assert Receipt.Total(100, 0) <> 100
End Test
```

`Assert <expr>` picks its Rust assertion from the operator, so the `=` / `<>` you
would write anyway give operand-level failure messages:

| You write            | Becomes         | On failure shows        |
|----------------------|-----------------|-------------------------|
| `Assert a = b`       | `assert_eq!`    | `left` and `right`      |
| `Assert a <> b`      | `assert_ne!`    | the equal values        |
| `Assert cond`        | `assert!`       | just the location       |

Inside an `Assert`, `=` is **equality**, never assignment. A block may hold
several `Assert`s; the first to fail reports.

- **Exercise functions directly** (`Receipt.Total(100, 3)`), not by running
  `Main()`.
- The description string is what `vbr test` prints — write it as the promise
  being made (`"zero quantity costs nothing"`), not as a bare label.
- Keep tests deterministic: no live network, no wall-clock, no randomness (unless
  a seed is fixed).

A `.test.vbr` file is compiled **only** by `vbr test`; `vbr run` / `vbr build` /
`vbr runproject` ignore it, so test-only code never affects the program build.

---

## `README.md` for each project

Explain, briefly:

- what the example does,
- which **VBR language** features it tests,
- which **standard-library** features it tests, if any,
- how to run the example,
- how to run the tests.

Use these exact commands (run from the repository root):

```sh
vbr runproject projects/2_simple_receipt    # build + run the program
vbr test        projects/2_simple_receipt    # run the tests
```

If the `vbr` binary is not installed, the same commands work through Cargo:

```sh
cargo run -- runproject projects/2_simple_receipt
cargo run -- test        projects/2_simple_receipt
```

`expected_output.txt` must contain the **exact program stdout** — the
`Debug.Print` lines from `main.vbr`, nothing else. (`vbr runproject` prints a few
`→ …` progress lines to *stderr*; those are not part of the expected output.)

---

## Gotchas (verified — avoid these)

- **Reserved type names cannot be identifiers.** `Integer`, `Long`, `LongLong`,
  `Single`, `Double`, `Boolean`, `Byte`, `String`, `Currency`, `Variant` are
  types — you cannot name a function or variable `Double`. The transpiler will
  reject it (e.g. `Function Double(...)` → “Expected a name for the function”).
- **Cross-module calls are qualified:** `Receipt.Total(...)`, where the prefix is
  the *filename* capitalised. A function is only callable from another module (or
  a test) if it is `Public`.
- **`main.vbr` is special** — its functions are not visible to tests. Keep all
  testable logic in a `<subject>.vbr` module.
- Types (`Type`, `Enum`) are **not** qualified — refer to them by their bare name
  across modules.

If a feature is unclear or unsupported, write a note in `notes.md` and choose a
simpler implementation. **Do not invent syntax that is not in the spec.**

---

## Constraints (recap)

- Place all generated projects under `projects/`.
- Do **not** touch `examples/`.
- Do **not** touch any Rust files. Do **not** edit `.rs` files.
- Do **not** modify the transpiler or add Rust stdlib implementation code.
- Use the existing VBR language and existing stdlib only.
- Prefer pure VBR over inline Rust.
- Keep examples deterministic: no live network, no unseeded randomness, no
  wall-clock-dependent output in tests.
- Produce clear, boring, deterministic examples useful for regression testing.

Create the project specified in chat with the user.
