# receipt — the `<module>.test.vbr` pattern

A minimal, deterministic project that demonstrates VBR's **test harness** the way
the [`testing_spec.md`](../../testing_spec.md) recommends: a module of pure logic,
tested by a `.test.vbr` file beside it.

```text
receipt/
  main.vbr           entry point — Function Main(); prints a receipt
  receipt.vbr        the logic: Public LineTotal / TaxPence / GrandTotal
  receipt.test.vbr   Test / Assert specs for receipt.vbr
  expected_output.txt exact stdout of running main.vbr
```

## What it does

A tiny till totals three line items in whole pence, adds 20% VAT, and prints the
receipt. All the arithmetic lives in the `Receipt` module; `Main` only arranges
the numbers and prints them — so the sums are testable in isolation.

## What it tests

**VBR language features**

- A **multi-module project** (a folder): `main.vbr` calls the sibling module with
  **qualified names** — `Receipt.LineTotal(...)`.
- **`Public` functions** as the tested contract, with `ByVal … As Long` params
  and `Return`.
- **Integer arithmetic**: `*`, `+`, and integer `/` (`Long / Long` truncates), so
  the pence maths is exact and deterministic.
- **String concatenation** with `&` for the printed lines.
- The **`Test` / `Assert` harness**: `Assert a = b` lowers to `assert_eq!`, so a
  failure shows the two operands and the `.vbr` line.

**Standard library:** none — this is pure core language on purpose (nothing
non-deterministic to test against).

## Run it

```sh
vbr runproject examples/receipt
# or, without the vbr binary installed:
cargo run -- runproject examples/receipt
```

Its stdout matches `expected_output.txt` exactly.

## Test it

```sh
vbr test examples/receipt
# or:
cargo run -- test examples/receipt
```

Expected:

```
  ✓ a line total multiplies unit price by quantity
  ✓ an empty line costs nothing
  ✓ tax is a whole-penny percentage of the amount
  ✓ the grand total adds tax onto the net

  4 passed
```

`vbr test` exits non-zero if any `Assert` fails, so it drops straight into CI.
