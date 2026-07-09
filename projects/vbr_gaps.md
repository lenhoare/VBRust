# VBR gaps & bugs found while building projects

A running log of bugs and missing capabilities in VBR, surfaced by trying to
build real projects (starting with the **Idea Evolution Engine**, a TUI idea
generator). Each entry: what we hit, why, and how it was resolved.

The pattern: building a project *pulls the language forward* — we build the
capability properly (or log it) rather than working around it in the example.

---

## Feature: `Http.Post` with headers (built)

The idea engine calls an LLM, and every chat API is `POST` + an auth header +
a JSON body. VBR only had `Http.Get`. We added:

- **`Http.Post(url, body, headers)`** → `Result<String, String>`, where
  `headers` is a `HashMap<String, String>` (VB's `Scripting.Dictionary`). The
  map is passed **by value** (consumed) — build one per request. `ureq` sets
  each header, then sends the string body.
- **Awaitable** in a `Window`/`Screen` event, exactly like `Http.Get`
  (`Match Await Http.Post(url, body, headers)`), so the request runs off the UI
  thread and the interface stays live.
- **Deferred:** `Await Http.Post` in a *browser* `Page`/`Screen` (native +
  blocking work). Browser POST isn't wired to `fetch` yet, and sending an API
  key from a browser is a bad idea anyway (CORS + secret exposure).

Design note — headers as a by-value `HashMap` (rather than a `&HashMap` or a
request-builder object): it's the one shape that works identically in the
blocking and awaited paths without threading reference-tracking through the
async snapshot machinery, and it reads naturally ("hand the request its
headers"). No line-continuation in VBR rules out a multi-line chained builder.

Examples: `examples/http_post.vbr` (blocking, in `Main`),
`examples/tui_post.vbr` (awaited in a Screen — the LLM-call shape). Both in the
compile guard.

---

## Bugs found (all fixed)

Building `Http.Post` exercised code paths no existing example did, exposing four
real bugs. Three of them are **not POST-specific** — they were latent in the
HashMap / event-body / async machinery and would bite any program of that shape.

### 1. `HashMap<String, String>` insert didn't own a string-literal *value*

`dict.insert("k", "v")` lowered the key to `"k".to_string()` but left the value
as a bare `&str` — so a `HashMap<String, String>` failed to compile (`insert`
wants an owned `String`). Only the key position was owned. The existing
`hashmap.vbr` example has `Long` values, so it never surfaced.

**Fix** (`transpiler.rs`): own a string *literal* in any `insert`/`push`
argument position, not just the key. The `Expr::Str` guard keeps it to literals
(a Vec's numeric index is never a string literal, so it's untouched).

### 2. `HashMap` import missing in `Screen`/`Window`/`Page` output

The `use std::collections::HashMap;` auto-import only scanned top-level
functions, not surface **event bodies**. A HashMap used only inside an event
(like `Http.Post` headers) compiled to code referencing an unimported `HashMap`.

**Fix** — first landed only in `tui.rs`, which exposed a deeper point: there are
**four surfaces but three emitter files** (`gui.rs` = Iced Window, `web.rs` = Yew
Page, `tui.rs` = *both* native ratatui and web Ratzilla `Screen`s via a `web`
flag), and each emitter hand-rolls its own imports because they need different
*crates*. A per-file patch would leave the GUI Window broken (it can await
`Http.Post`) and silently reappear on any future surface.

So the import decision moved into the shared core: **`surface::event_std_imports(events)`**
(uses `transpiler::body_uses_hashmap`, now `pub(crate)`) returns the `std` `use`
lines a surface's event bodies need. All three emitters call it in their
preamble → all four surfaces covered, and a new surface opts in with one line
instead of rediscovering the bug. `std` types are common across renderers; only
the crate imports (Iced/ratatui/Yew) stay per-emitter.

### 3. Event-body locals never got `let mut`

`emit_event_stmts` emitted every event statement with an **empty** "mutated"
set, so a local reassigned or mutated in place (`headers.insert(…)`) came out as
`let headers` (not `let mut`) → won't compile. Plain functions run a
`collect_mutated` pre-scan; event bodies skipped it.

**Fix** (`surface.rs`, using `transpiler::collect_mutated` made `pub(crate)`):
run the same mutated-locals pre-scan over the event body and pass it to
`emit_stmt`.

### 4. Awaited call passed a local owned-`String` arg by value into a `&str` param

The async snapshot machinery (`snapshot_args`) referenced (`&x`) only *state
fields* of `String` type. A **local** owned `String` — e.g. a request body built
with `Dim body As String = …` before the `Await` — was passed by value into the
stdlib's `&str` param → type mismatch. (A string *literal* body hid it; a `Dim`
local exposed it.)

**Fix** (`surface.rs`): collect the types of locals in scope at the `Await`
(event params + `Dim`s before it) via a new `local_types`, and have
`snapshot_args` borrow an owned-`String` local as `&str`, just as it does for a
field.

### 8. Stdlib *wrapper-instance* methods didn't `&`-reference owned-String args

The arg-ref rule (owned `String` → `&x` for a `&str` param) only fired for
stdlib **type** receivers (`Http.Get(url)`, `FileSystem.Read(p)`), not for
methods on a wrapper **instance** — `doc.GetInt(k)` with `Dim k As String`
emitted `doc.get_int(k)` → won't compile. Hidden because every existing
Json/DateTime example used string *literals*; surfaced by designing
`db.Execute(sqlVar, …)`.

**Fix** (`resolver.rs`): the rule also fires when the receiver's declared type
is a stdlib wrapper (`DeclType::Named(n)` with `stdlib_type(n)` — `Json`,
`DateTime`, `Database`…). Zero snapshot churn, which is the hiding confirmed.

### 9. `CStr` didn't exist (and an unknown function is silent)

VBR had `Str(x)` but not `CStr(x)` — yet `CStr` was VB's *recommended*
conversion, so it's what a VB6 hand types. Worse, `CStr(root)` fell through as
a call to a nonexistent `cstr()` with **no VBR diagnostic** (rustc catches it
later, via error translation — the backstop worked, but a teaching hint would
have been kinder).

**Fix**: `CStr` is now an alias of `Str` (→ `.to_string()`). The broader
"unknown function passes through silently" behaviour is by design (rustc is the
backstop) but worth watching — if it keeps biting, known VB6 names (`CInt`,
`CLng`, `CDbl`…) deserve mappings or teaching notes.

---

## Open bugs (not yet fixed)

### 5. Fallible call assigned straight to a non-`Result` `Dim` isn't caught

`Dim s As String = FileSystem.Read("notes.txt")` — a stdlib call returning
`Result<String, String>` bound directly to a `String` `Dim` — emits
`let s: String = FileSystem::read("notes.txt");` with **no diagnostic**, which
won't compile (`Result<String, String>` ≠ `String`). VBR should either require
the result be handled (`Match` / `?`) or teach the mismatch. Found while
explaining why a `Database` handle can't sit in a `State` field (same shape:
`Open` returns `Result`, the slot wants the bare type). Not yet fixed.

### 6. `Http` has no request timeout

ureq requests run with no overall timeout, and for LLM endpoints (slow by
nature) a hung server means the request never returns. Natively the UI stays
live (the call is `Await`ed off-thread) but the event never completes — a
Screen sits on "sending…" forever with no error. `Http`'s calls should carry a
sensible default timeout (30–60s) so a hang comes back as an `Err` string like
every other failure. Follow-up from the `Http.Post` review; not yet built.

### 7. No map literal — "no headers" / empty-HashMap calls are clunky

The new list literal (`[]`) covers `Vec`, but there's no `HashMap` literal, so
`Http.Post(url, body, [])` can't express "no headers" — the caller must
`Dim h As HashMap<String, String>` and pass it unused. First concrete argument
for a future `{k: v}` / `{}` **map literal** — consistent with having reserved
`{}` for maps when choosing `[]` for lists. Not yet built.

---

## Capability: SQLite stdlib namespace — BUILT (slice 1)

`Database` is in the stdlib (`stdlib_spec.md` §8): `Open` / `Execute` / `Query`
(rows as `Json`) / `LastInsertId`, rusqlite bundled, `database` feature.
Verified end-to-end: `examples/database.vbr` runs — typed reads, NULL lineage
roots via `IsNull`, parent links via `LastInsertId`. **Slice 2 (todo):** hold a
handle on surface state — a general "seed state from `Main`" seam across *all*
surfaces (a `:memory:` db forces it; open-per-event covers file dbs today).
