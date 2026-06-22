# VBR Standard Library — Revised Spec (decisions)

A short companion to `VBR_spec_03_stdlib.md` recording the decisions we made
before implementing. Explanations only — the original file still holds the
module-by-module code.

---

## 1. Purpose

`vbr_stdlib` is a **separate Rust crate** (its own `Cargo.toml`, not part of the
transpiler's build) that gives VBR programs friendly, native replacements for
the things VB leaned on COM for — file access, JSON, dates, regex. Every
fallible function returns `Result<T, String>`, which maps straight onto VBR's
`As Result<T>`.

---

## 2. Calling convention — namespaced (Option A)

Stdlib modules are **namespaces of functions, not stateful objects** — there's
nothing to instantiate. So you call them directly through the type name:

```vb
Dim text As String = FileSystem.Read("notes.txt")
```

- **Source uses `.`** (VB-natural). No `Set fs = New FileSystem` ceremony.
- **Output uses `::`** — the transpiler turns `FileSystem.Read(x)` into
  `FileSystem::read(x)`. Seeing `::` in the generated Rust is the intended
  "small dose of Rust to spark curiosity."
- **Keep the Rust type names** — `FileSystem`, `Json`, `DateTime`, `Regex`.

### Transpiler support needed
- Recognise the known stdlib type names; when one is a method-call receiver,
  emit `Type::method(args)` instead of `recv.method(args)`.
- Auto-emit `use vbr_stdlib::FileSystem;` (etc.) for each stdlib type used —
  exactly the way `HashMap` is auto-imported today. No explicit `Use` needed.

---

## 3. Modules included now

| Module | Wraps | Status |
|---|---|---|
| `filesystem` (`FileSystem`) | `std::fs` / `std::io` / `std::path` | include |
| `json` (`Json`) | `serde_json` | include |
| `datetime` (`DateTime`) | `chrono` | include |
| `regex` (`Regex`) | `regex` | include |
| `http` (`Http`) | `reqwest` blocking | **deferred** |
| `database` | — | deferred (was already V2) |

HTTP is left out for now: `reqwest` drags in tokio/hyper/TLS, needs network to
build, and its tests hit the network. We'll add it later, behind a Cargo
feature so the rest stays fast and offline-friendly.

---

## 4. Design rules (unchanged from spec_03)

- Every fallible operation returns `Result<T, String>`.
- A VBA-equivalent comment on each public function (the teaching value — keep it).
- Real, idiomatic Rust inside, so the source is itself a learning resource.
- Include the `#[cfg(test)]` tests for each module.

---

## 5. Changes from the original spec_03

1. **Drop the `http` module** for this pass (see §3).
2. **`DateTime.year/month/day`**: use chrono's `Datelike` methods
   (`dt.year()`, `dt.month()`, `dt.day()`) instead of the
   `format("%Y").parse().unwrap()` round-trip — cleaner and it can't panic.
3. **Remove the `tokio` dev-dependency** — the library and tests are all
   synchronous, so it isn't needed.
4. **Cargo.toml**: only `serde` / `serde_json` / `chrono` / `regex` now.
   Bump `reqwest` to a current version *if/when* http returns.
5. **Cosmetic**: fix the file-tree diagram (stray `regex.rs` indentation; drop
   the deferred `database.rs` line).

---

## 6. Integration plan (two honest pieces)

Our `--run` invokes `rustc` directly, and rustc alone can't link an external
crate like `vbr_stdlib`. So:

- **Now:** build the `vbr_stdlib` crate (with the §5 fixes) so it exists and
  compiles, **and** teach the transpiler the `.`→`::` + auto-`use` so the
  *generated Rust is correct* for stdlib calls.
- **Later (optional):** a cargo-project build mode — generate a `Cargo.toml`
  with `vbr_stdlib` as a dependency and `cargo run` it — so stdlib programs
  actually execute end-to-end. Until then we can compile/inspect the output.

---

## 7. Deferred / open

- `http` module (behind a feature, current `reqwest`).
- `database` module (needs async — V2).
- Cargo-project run mode for programs that use the stdlib.
