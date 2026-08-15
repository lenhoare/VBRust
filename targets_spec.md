# Bust — Alternative Targets: Python & C

Bust is a **Rust-first** language: the semantics are Rust's, the language was
designed around Rust, and `run`/`runproject` (→ `rustc`/Cargo) is the primary,
fully-featured path. Everything in `language_spec.md` describes that target.

On top of that, Bust can transpile the **same source** to two other languages:

- **Python** — `vbr py <file.vbr>`
- **C** — `vbr c <file.vbr>`

These are **additive bolt-ons**, not a redefinition of the language. They consume
the same parsed AST as the Rust backend and cover the **core language** (and, for
Python, the full standard library); they exist to *show the same program in three
languages* — a teaching lens on how Rust's ownership, `Option`/`Result`, pattern
matching and iterators map onto a garbage-collected language and onto manual C.

The **ground-truth discipline**: for deterministic programs, the Python and C
output is checked **byte-for-byte against `vbr run`** (the Rust output). Where a
program is inherently non-deterministic (a `HashMap`'s iteration order, wall-clock
time, the network), the generated code is snapshotted but not diffed.

---

## 1. The shared front-end (how three targets stay in sync)

The three backends share a small typed/desugared front-end — grown from the
duplication between them, not designed up front:

- **`convert_returns`** — the VB `name = expr` → `Return expr` desugaring.
- **`types.rs`** — a target-neutral **typing pass**: the single authority for
  "what type is this expression?", keyed by source span. (The Rust resolver keeps
  its own inference for Rust-specific lowering — casts, borrows — but the Python
  and C backends both read this one.)
- **`pattern.rs`** — the `Match`-pattern model (literals, ranges, alternation,
  enum tags & data variants, `Some`/`None`/`Ok`/`Err`).
- **`iter.rs`** — the iterator-chain model (base + adapter steps + terminal
  consumer: `filter`/`map`/`take`/`skip`/`rev` and `collect`/`sum`/`any`/`find`/…).

Each backend *parses, types and analyses* through these, then renders its own
way. Adding a construct (a new pattern shape, a new iterator adapter) is a
one-place change both targets can then lower. This is Bust's lightweight "IR": the
AST plus a shared semantic layer, lowered per target.

---

## 2. Surfaces that stay Rust-only

The GUI/TUI/Web surfaces are **Rust-only by design** — they *are* their host
frameworks (Iced, ratatui, Yew) and have nothing neutral to lower to:

- `Window` (GUI), `Screen` (TUI), `Page` (Web), `Sketch` (draw/animate), `Canvas`/`Draw`.

`vbr py`/`vbr c` on a program that uses one emits a `⚠` warning and skips it. The
alternative targets are for the **core language and computation**, not the app
surfaces.

---

## 3. The Python target — `vbr py`

Idiomatic Python 3, on Python's batteries (no pip installs for the stdlib).

- **Coverage:** the **entire core language** — functions, `Dim`, arithmetic (with
  the widening rules), `If`/`For`/`Do`, `Match`, `Enum`, `Type`/methods, `Const`,
  `Vec`/`HashMap`, iterators, `Option`/`Handle`/`RaiseError` — **and the full standard
  library**.
- **Idioms:** `Type` → `@dataclass`; `Match` → `match`/`case`; iterator chains →
  **comprehensions/generators**; `Option` → a tiny `Some` prelude (`None` is
  Python's own); fallible calls propagate like the Rust target (`Handle` /
  `RaiseError` / `Raw`). Output prints
  byte-identically to Rust (a `_vb()` display helper matches Rust's `Display`:
  `true`/`false`, whole floats without `.0`).

### Packaging

- A **core-language program** is a single `.py`: `vbr py f.vbr` prints it (or
  `-o out.py`), run with `python3`.
- A **stdlib program** is emitted as a **project folder** (the parallel of
  `runproject`): `main.py` plus a copied **`vbrpy/`** package — the Python
  analogue of the `vbr_stdlib` crate, implementing `FileSystem`/`Regex`/`Json`/
  `Database`/`DateTime`/`Http`/`DataFrame`/`Shell` on Python's standard library
  (`sqlite3`, `urllib`, `subprocess`, …) with **zero pip installs**. `DataFrame`
  lowers to idiomatic **polars** (the one pip dependency).

### External libraries

Two ways to reach the pip ecosystem, mirroring how inline Rust / `Use` reach
Cargo:

- **`Use <package> <version> [As <module>]`** → an `import` plus a generated
  `requirements.txt` line. On the Python target `Use` targets **pip** (on the Rust
  target the same keyword targets Cargo). The `As` alias handles packages whose
  import name differs from their install name — `Use PyYAML 6.0 As yaml` →
  `import yaml`, `PyYAML==6.0` in `requirements.txt`.
- **Inline `Python … End Python`** — on the Python target the block *is* Python,
  so it splices through verbatim (the mirror of inline `Rust` on the Rust target).
  Anything imported at module scope, or inside the block, is in reach.

---

## 4. The C target — `vbr c`

Portable C, emitted as a **single self-contained `.c`** with a small runtime
inlined at the top. Build it with any C compiler:

```sh
vbr c examples/hello.vbr -o hello.c
cc hello.c -lm && ./a.out
```

- **Coverage:** the **entire core language** — scalars/strings, `If`/`For`/`Do`,
  `Type`/methods, `Const`, `Match`/`Enum`, `Vec`/`HashMap`, iterators,
  `Option`/`Result`/`Handle` — **plus the standard library** (see below).
- **Idioms:**
  - `Type` → a `typedef struct`; methods → free functions taking a `Struct* self`
    (`Me.field` → `self->field`).
  - `Enum` → a C `enum` (payload-free) or a **tagged union** (`{ tag; union … }`)
    for data-carrying variants; `Match` → an `if`/`else-if` chain over a scrutinee
    temp (C's `switch` can't do ranges, guards or bindings).
  - `Vec<T>`/`HashMap<K,V>` are **monomorphised** — each instantiation gets its
    own typed struct + functions (`Vec_longlong`, `Map_str_longlong`, …).
  - Iterator chains have no C expression form, so they become **explicit loops**.
  - `Option<T>`/`Result<T,E>` → small `{ is_some/is_ok, … }` structs; fallible
    calls propagate with an early return; `Handle` is a match; `Raw` yields the
    struct.
- **Float formatting** matches Rust byte-for-byte via shortest-round-trip
  (increasing `%g` precision until it re-parses to the same bits).

### The C standard library — single file vs. project folder

The stdlib namespaces lower to C in two packagings, chosen by whether the
namespace needs anything beyond libc:

- **Self-contained** (`FileSystem`, `DateTime`, `Shell`, `Regex`) — the runtime
  is inlined over libc/POSIX, so the program stays a **single `.c`** you build
  with plain `cc`. No external dependency.
- **Vendored / linked** — a namespace with no libc equivalent is emitted as a
  **project folder** (the parallel of Python's `vbrpy/` mode): `main.c`, the
  bundled library sources from `csupport/`, and a `Makefile`. `vbr c` reports
  this and writes the folder; build it with `cd <name>_c && make && ./main`.
  - **`Json`** vendors **cJSON** (MIT, `csupport/cJSON.{c,h}`) — no system
    package, no network; a `Json` is a thin handle over a `cJSON*` node, with the
    typed `get_*`/`as_*` accessors returning the same `Result<T>` as `vbr_stdlib`.
  - **`Database`** *links* **SQLite** (`-lsqlite3` — the most-deployed library on
    earth, so the pragmatic path over checking in the 9 MB amalgamation, which
    would also recompile on every `make`). A `Database` is a live `sqlite3*`
    connection; params bind positionally as text (column affinity types them);
    `Query` rows come back as `Json` objects keyed by column, so it reuses the
    vendored cJSON — a `Database` program both **links** `-lsqlite3` **and
    vendors** `cJSON`. (The amalgamation stays available: the vendor path exists
    for anyone who prefers a zero-system-dep build.)
  - **`Http`** *links* **libcurl** (`-lcurl`) — one-shot blocking `Get`/`Post`
    with the full `https` support the Rust/Python targets have (TLS rules out a
    hand-rolled or vendored version). `Post` takes a `HashMap<String,String>` of
    request headers. A link-only namespace still becomes a project folder (for
    the `Makefile`'s `-lcurl`), just with no vendored sources.

Linked namespaces need the library's dev package at build time (`libsqlite3-dev`,
`libcurl4-openssl-dev`); vendored ones (cJSON) need nothing but a C compiler.

The `vbr runproject` stdout is the ground truth: a C stdlib example's output is
byte-identical to the Rust build where deterministic (`Json` field reads, file
I/O), and snapshot-only where it isn't (HashMap order, wall-clock, network). The
error/serialisation *failure* paths can differ from Rust's wording (cJSON's parse
message, number formatting) — those aren't on the byte-identical happy path.

The C standard library is **complete** — every namespace the Rust/Python targets
have (`FileSystem`, `DateTime`, `Shell`, `Regex`, `Json`, `Database`, `Http`).
The one exception is **`DataFrame`**: there's no idiomatic C equivalent, so it
warns rather than lowering.

### Memory model — `x = Nothing`

C has no ownership and no GC, so Bust takes the deliberately-simple, **teaching**
stance: **leak by default, release explicitly.** This is exactly what makes a C
target worth having — it puts on the page the manual-memory cost Rust's ownership
hides.

- Heap values (strings, `Vec`, `HashMap`) are individually `malloc`'d and are
  **not freed automatically**; at program exit the OS reclaims them.
- **`x = Nothing`** is the explicit release hook (VB6's object-release idiom,
  carried over). See §5 — it is a real language statement that lowers on all three
  targets.

Arrays and finer memory management are later slices; a `⚠` warning marks
anything a C slice doesn't cover yet.

---

## 5. `x = Nothing` — explicit release (all targets)

`x = Nothing` releases a heap value early. It is a first-class Bust statement, not
a C-only construct, and lowers idiomatically everywhere:

| Target  | Lowering                    | Note                                        |
|---------|-----------------------------|---------------------------------------------|
| Rust    | `drop(x);`                  | what the compiler usually inserts at scope end |
| Python  | `x = None`                  | the garbage collector reclaims it           |
| C       | `free(x); x = NULL;`        | the real work — the reason the hook exists  |

`Nothing` is only valid as an assignment right-hand side on a plain variable
(`x = Nothing`). Because Bust repurposed `Set` to mean **borrow** (not VB6's object
assignment), writing **`Set x = Nothing`** is a teaching error that steers you to
the plain form.

> Testing whether a released reference *is* `Nothing` (VB6's `Is Nothing`) is not
> provided: Rust has no null — a dropped value is gone, not testable. The
> idiomatic "maybe-absent, and I can check it" tool is `Option`/`None` + `Match`.

---

## 6. What's covered, at a glance

| Area                          | Rust | Python | C |
|-------------------------------|:----:|:------:|:-:|
| Core language (all of §1–§9)  |  ✓   |   ✓    | ✓ |
| Standard library              |  ✓   |   ✓    |   |
| GUI / TUI / Web surfaces      |  ✓   |        |   |
| Inline escape hatch           | `Rust` | `Python` | — |
| External deps                 | `Use`→Cargo | `Use`→pip, inline Python | — |
| `x = Nothing`                 |  ✓   |   ✓    | ✓ |

`✓` = supported; blank = not applicable / not yet.
