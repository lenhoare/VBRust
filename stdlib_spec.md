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
| `http` (`Http`) | `ureq` blocking | include |
| `database` (`Database`) | `rusqlite` (bundled SQLite) | include — see §8 |
| `shell` (`Shell`, `Process`) | `std::process` | include — see §9 |

HTTP is built on **`ureq`** (blocking, no async runtime, minimal deps) rather
than `reqwest`, so the crate stays fast and offline-friendly; it lives behind a
Cargo feature like the rest. Two functions, each one-shot (no shared client or
session — reach for inline Rust / a `.rs` module for that):

- **`Http.Get(url)`** → `Result<String, String>` — the response body, or the
  failure as a `String`.
- **`Http.Post(url, body, headers)`** → `Result<String, String>` — POST `body`
  (a string) with `headers`, a `HashMap<String, String>` of request headers
  (`Content-Type`, an `Authorization: Bearer …` token, …); pass an empty map
  for none. The map is passed **by value** (consumed), so build one per request.

Both are **awaitable** in a `Window`/`Screen` event — `Match Await Http.Get(url)`
/ `Match Await Http.Post(url, body, headers)` runs the request off the UI thread
so the interface stays live. (In a browser `Page`/`Screen`, only `Await Http.Get`
is supported today — it maps to the browser's `fetch`; POST there is deferred.)

Both carry a **60-second overall timeout**: a hung server comes back as an
`Err` string like every other failure, never a call that waits forever
(generous, because LLM endpoints legitimately take a while).

---

## 4. Design rules (unchanged from spec_03)

- Every fallible operation returns `Result<T, String>`.
- A VBA-equivalent comment on each public function (the teaching value — keep it).
- Real, idiomatic Rust inside, so the source is itself a learning resource.
- Include the `#[cfg(test)]` tests for each module.

---

## 5. Changes from the original spec_03

1. ~~**Drop the `http` module** for this pass~~ — `http` is now **included**,
   built on `ureq` (not `reqwest`); see §3. `Http.Get` and `Http.Post` (with a
   headers map), both awaitable in a Window/Screen event.
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

- `http`: `Await Http.Post` in a browser `Page`/`Screen` (native + blocking
  work); other verbs (PUT/DELETE), custom timeouts, a reusable client/session.
- Cargo-project run mode for programs that use the stdlib. *(Built: `runproject`
  / `build`.)*

---

## 8. SQLite — the `Database` module (slice 1) — **BUILT 2026-07-09**

The first genuinely **stateful** stdlib module: a database is a live connection
you hold, so `Database` is a **newtype-wrapper handle** like `DateTime` / `Json`
/ `DataFrame` — a static constructor plus instance methods. Built on **`rusqlite`
with the `bundled` feature** (compiles SQLite from source → no system
`libsqlite3` to install, consistent with `ureq`'s "no system setup"), behind a
`database` Cargo feature.

### Surface

```vb
' open (fallible → Result), then run statements on the handle
Match Database.Open("ideas.db")
    Ok(db) =>
        db.Execute("CREATE TABLE ideas (id INTEGER PRIMARY KEY, gen INT, text TEXT, score REAL)", [])

        db.Execute("INSERT INTO ideas (gen, text) VALUES (?, ?)", [CStr(gen), ideaText])

        Dim rows As Vec<Json> = db.Query("SELECT text, score FROM ideas WHERE gen = ? ORDER BY score DESC", [CStr(gen)])
        For Each row In rows
            Debug.Print row.GetString("text").Unwrap() & " — " & row.GetFloat("score").Unwrap()
        Next
    Err(message) => Debug.Print "open failed: " & message
End Match
```

| Method | Rust | Returns |
|---|---|---|
| `Database.Open(path)` | `Database::open(path) -> Result<Database, String>` | the handle |
| `db.Execute(sql, params)` | `execute(&self, sql, params: Vec<String>) -> Result<i64, String>` | rows affected |
| `db.Query(sql, params)` | `query(&self, sql, params: Vec<String>) -> Result<Vec<Json>, String>` | one `Json` object per row |
| `db.LastInsertId()` | `last_insert_id(&self) -> i64` | last auto-increment rowid (for lineage) |

### Design decisions (settled)

- **Rows come back as `Vec<Json>`**, reusing the `Json` wrapper already taught.
  Each column is mapped to its **natural SQLite storage type** (INTEGER → Json
  int, REAL → Json float, TEXT → Json string, NULL → Json null), so
  `GetInt`/`GetFloat`/`GetString` return real typed values — no text round-trip
  on read. No new row type to learn.
- **Parameters are `Vec<String>`, passed by value** (consumed, like `Http.Post`'s
  headers map — the one shape that needs no reference-plumbing). Bound to `?`
  placeholders positionally; numbers go in as text and land in typed columns via
  **SQLite column affinity** (so declare columns `INTEGER`/`REAL`). Real bound
  parameters, never string concatenation — injection-safe. A `&str` element in the
  params list — a `ByVal As String` param (`db.Execute(sql, [id, name])`), a
  `Trim(x)` — is **owned for you** (`.to_string()`), so no manual `.clone()`; a
  string literal is already owned, a `Long` needs `CStr(n)` to become text.
- **`Database` handle**: passes into functions as `&Database` (a ByVal struct
  param already lowers to a shared borrow — rusqlite methods take `&self`, so
  this is exactly right) and holds fine as a local. **Holding it on surface
  state: BUILT (slice 2, 2026-07-09)** — a `State` field initialiser may be a
  fallible call (`Dim db As Database = Database.Open("ideas.db")`, or your own
  `Result`-returning function): the state is built by a generated
  `init() -> Result<State, String>` that runs *before* the window/terminal
  starts, printing `could not start: <why>` and exiting on failure. All native
  surfaces (Window + Screen); browser surfaces get a teaching fence. See
  `gui_spec.md`/`tui_spec.md` §2.1 and `examples/tui_ideas.vbr`.

### Prerequisite fix (general, not SQLite-only) — **fixed**

`db.Execute(sql, …)` where `sql` is a **String variable** exposed a latent bug:
a stdlib **wrapper instance** method didn't `&`-reference its owned-String args
(only stdlib *type* receivers like `Http.`/`FileSystem.` did) — `doc.GetInt(k)`
with `k As String` emitted `doc.get_int(k)` (String into a `&str` param → won't
compile); every working example used string *literals*, which hid it. The
resolver's arg-ref rule now also fires when the receiver's declared type is a
stdlib wrapper (`DeclType::Named(n)` with `stdlib_type(n)`), fixing
`Json`/`DateTime`/`Database` alike. Logged in `projects/vbr_gaps.md`.

### Also picked up along the way

- **`Json.IsNull()`** — new: true for JSON null. Needed to spot a NULL column
  (`row.Get("parent")?.IsNull()`); previously null was only visible as a failed
  typed read.
- **`CStr(x)`** — new alias for `Str(x)` (→ `.to_string()`): `CStr` was VB's
  *recommended* conversion, so it's what a VB6 hand types first.
- **NULL params:** a `Vec<String>` has no null slot — write NULL in the SQL
  itself (`VALUES (?, NULL)`). Reading it back: `IsNull`.
- **BLOB columns:** a clean `Err` pointing at inline Rust (not silently mangled).

### Slice-1 scope & known friction

- **In:** `Open`/`Execute`/`Query`/`LastInsertId`, Json rows, string params,
  bundled rusqlite, feature-gated, hermetic tests (a temp-file db, like `http`'s
  loopback server).
- **Deferred:** in-memory (`:memory:`) dbs on *browser* surfaces (native holds
  a connection in State now), transactions, prepared-statement reuse,
  typed/`Json` params, named parameters, a custom failure policy for a failed
  `State` init (today's policy is fixed: message + exit — the rare "show a
  picker window instead" case would need `Run`-args, designed but not built).
- **Params ergonomics — the inline list literal (BUILT first).** VBR now has an
  **inline list literal** `["a", "b"]` → `Vec<T>` (string elements owned,
  numbers typed from the target; empty `[]` allowed), so params read cleanly:
  `db.Execute("INSERT … VALUES (?, ?)", [CStr(gen), ideaText])`. A no-parameter
  statement passes `[]` (there are no `Optional` params). This was built ahead of
  the SQLite module so the API is pleasant from day one — see `list_literal.vbr`,
  `language_spec.md` (Collections).

---

## 9. Shell — commands and child processes — **BUILT 2026-07-11**

VB6's `Shell`, grown up: `std::process` behind two verbs. Std-only (no Cargo
feature, like `FileSystem`), so it's always available.

- **`Shell.Run(cmd)`** → `Result<String, String>` — run `cmd`, **wait** for
  it, capture output: `Ok(stdout)` (trailing newline trimmed) on exit 0,
  `Err(stderr)` (or the exit status when stderr is empty) otherwise.
- **`Shell.Start(cmd)`** → `Result<Process, String>` — launch and **don't
  wait**: VB6's actual `Shell` semantics. The child's stdin/stdout/stderr are
  detached, so a background server can't scribble over a terminal UI. Dropping
  the handle does *not* stop the process — call `Kill`.
- **`proc.IsRunning()`** → `Boolean` — a snapshot (`try_wait`), safe in a UI
  event. **`proc.Kill()`** — stop and reap it (a no-op if already dead).
  **`proc.Wait()`** → `Long` — block for the exit code (`-1` when unknowable,
  e.g. after a kill).

Commands go **through the system shell** (`sh -c` / `cmd /C`), so pipes,
redirects, and PATH behave like a terminal.

Integration: `Shell.Run` joins the blocking-call check (un-`Await`ed in an
event is a teaching error) **and** the awaitable set (`Match Await
Shell.Run(cmd)` runs off the UI thread, same shape as `Http.Get`).
`Shell.Start`/`Shell.Run` are fallible `State` initialisers — a `Process` can
live in a `Screen`'s state, started before the terminal opens with the clean
`could not start` bail-out (the local-LLM-server shape: `examples/tui_shell.vbr`).
`Kill`/`Wait`/`IsRunning` are registered mutating methods (`let mut`, `&mut`
through state).

Deferred: capturing a started process's output (a log pane wants a pipe +
non-blocking reads); `proc.Wait()` inside an event isn't caught by the
blocking check (it only sees stdlib-*type* receivers, not instances); an
`Await Shell.Run` browser story (no processes in a sandbox — permanent fence).

Examples: `examples/shell.vbr` (run/capture, failure, start/check/kill — runs
end-to-end), `examples/tui_shell.vbr` (a `Process` behind a `Screen`). Both in
the compile guard; 4 hermetic stdlib tests.
