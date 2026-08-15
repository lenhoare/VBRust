# 53 — Periodic Table

A terminal app that reads element data and prints a table with lookups.
This project exists to exercise the **Python target** (`vbr py`): the same
Bust source transpiles to idiomatic Python and its output is checked
**byte-for-byte against the Rust build** (the ground-truth discipline from
`targets_spec.md`).

## Structure

The project is split to work around two Python-target limitations
(documented in notes.md, Quirks 48–49):

- **`main.vbr` + `periodic.vbr` + `periodic.test.vbr`** (project root) —
  the full CSV-parsing version: `FileSystem.Read_Lines` → `Periodic.ParseRow`
  for all 24 elements → table + lookups. This runs on the **Rust** target
  (`vbr runproject`) and is fully unit-tested (7/7).
- **`py/main.vbr`** (subfolder) — the single-file variant that `vbr py`
  can transpile. It keeps the stdlib exercise (one `FileSystem.Read_Lines`
  proves the `vbrpy` package works) but hardcodes a 5-element table,
  because the Python backend has no `Mid`/`Left`/`Val` string builtins.

## What the Python target test shows

```sh
vbr runproject projects/53_periodic_table/py   # Rust ground truth
vbr py projects/53_periodic_table/py/main.vbr -o out/   # → Python project
cd out && python3 main.py                       # zero pip installs
```

The Python output is **byte-identical** to the Rust output. The generated
code is idiomatic: `Type` → `@dataclass`, `.Len()` → `len()`, f-strings,
and the `vbrpy/` stdlib package (FileSystem on pure Python batteries).

## Bust language features tested

**Python target (the point of the project):**
- `vbr py` single-file transpilation → project folder with `vbrpy/`
- `FileSystem.Read_Lines` (stdlib) lowering to the Python package
- Byte-for-byte output parity with Rust
- Idiomatic lowering: `@dataclass`, `list.append`, `len()`, f-strings

**Core language (in periodic.vbr, 7 unit tests on the Rust target):**
- `Public Type Element` with 5 fields; struct-in/struct-out lookups
- `SplitCsv` / `ParseRow` (Mid-based, Rust-target)
- Sentinel-element "not found" pattern (`NoneElement`)
- `Pad3` / `PadRight` formatting helpers

## Running it

```sh
vbr runproject projects/53_periodic_table       # Rust: full CSV version
vbr test        projects/53_periodic_table      # 7 logic tests
vbr runproject projects/53_periodic_table/py    # Rust: single-file variant
vbr py          projects/53_periodic_table/py/main.vbr -o /tmp/pt_out
cd /tmp/pt_out && python3 main.py               # Python target
```

## Expected output

- `expected_output.txt` (project root) — the **multi-module Rust build**:
  the full 24-element table from the CSV.
- `py/expected_output.txt` — the **single-file variant** (5 hardcoded
  elements + the FileSystem read). The Python target output matches this
  byte-for-byte (verified by diff).
