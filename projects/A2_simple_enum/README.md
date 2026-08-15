# A2 — Simple Enum Demo

The second A-series project demonstrates the use of `Enum` in Bust:
a tiny traffic-light state machine.

## What it does

- `traffic.vbr` defines a `Public Enum TrafficLight` with three unit variants
  (`Red`, `Yellow`, `Green`).
- Two public functions:
  - `DurationSec(light As TrafficLight) As Long` returns the light time in
    seconds via a `Match` over the enum.
  - `Name(light As TrafficLight) As String` returns the printable name.
- A few `Public Const` values hold the default timings.
- `main.vbr` prints a table by iterating over a `Vec<TrafficLight>` and
  calling the two helper functions; it also shows a few spot checks.
- Everything is deterministic and pure core language (no stdlib).

```text
projects/A2_simple_enum/
  main.vbr           entry point — Function Main(); prints the table
  traffic.vbr        the logic: Public Enum + DurationSec + Name + constants
  traffic.test.vbr   Test / Assert specs for traffic.vbr
  expected_output.txt exact stdout of running main.vbr
```

## Which Bust language features it tests

- A **multi-module project**: `main.vbr` calls the sibling module with a
  **qualified name** — `Traffic.DurationSec(...)`.
- **`Public Enum`** (unit variants) and how it becomes a Rust `enum`
  with `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`.
- **`Match`** over an enum — exhaustive, each arm `variant => body`.
- **`Public Const`** (`DefaultRedSec` etc.) read cross-module; constants are
  uppercased in the generated Rust.
- **`Vec<T>`** and **`For Each`** loop (the loop variable is borrowed).
- **String concatenation** with `&`.
- The **`Test` / `Assert` harness**: `Assert a = b` lowers to `assert_eq!`,
  so a failure shows both operands and the `.vbr` line.

**Standard library:** none — pure core language, deliberately.

## Run it

```sh
vbr runproject projects/A2_simple_enum
# or, without the vbr binary installed:
cargo run -- runproject projects/A2_simple_enum
```

Its stdout matches `expected_output.txt` exactly.

## Test it

```sh
vbr test projects/A2_simple_enum
# or:
cargo run -- test projects/A2_simple_enum
```

Expected:

```
  � ✓ red light duration
  � ✓ yellow light duration
  � ✓ green light duration
  � ✓ constants are positive

  4 passed
```

## Notes for the transpiler

- No new quirks surfaced; the project built and ran on the first try after
  making the enum `Public` (required for cross-module use) and providing a
  `Public Function Name` to print the variant (Bust does not yet derive
  `Display` for enums, so `Debug.Print light` fails to compile).
- Two quirks from A1 still apply when passing literals:
  1. Qualified calls don't yet adapt integer literals to `Double`/`Long`
     parameters — use `.0` literals or a local widened variable.
  2. `For`-loop counters are never adapted to `Double`/`Long` parameters —
     widen through a local `Dim` before passing.

Both workarounds are documented in `projects/notes.md` (A1 entry).
