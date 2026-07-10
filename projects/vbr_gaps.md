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

### 6. `Http` had no request timeout

ureq requests ran with no overall timeout; a hung LLM endpoint meant the call
never returned — natively the UI stayed live (the call is `Await`ed off-thread)
but the event never completed, a Screen stuck on "sending…" with no error.

**Fix** (`vbr_stdlib/src/http.rs`): `Http.Get`/`Http.Post` carry a 60-second
overall timeout (generous — LLM generations take a while); a hang comes back as
an `Err` string like every other failure. Hermetic test: a loopback server that
accepts and then says nothing, hit through a 1-second-timeout helper.

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

### 10. The state-field rewrite didn't recurse through wrapper expressions

`rewrite_expr_with` (the `db` → `state.db` rewrite for event bodies) had a
catch-all that swallowed `Ref`, `Try`, `Tuple`, `List`, `StructLit`, `Closure`,
`Deref`. First seen as `AddIdea(db)` emitting `addidea(&db)` instead of
`addidea(&state.db)` — the resolver wraps the ByVal-struct arg in `&` first, and
the rewrite stopped at the `&`. The same hole affected any state field inside a
`?` chain, a tuple, a list literal, or a closure in an event.

**Fix** (`surface.rs`): the rewrite recurses through all wrapper/aggregate
expression forms. Zero snapshot churn (no existing example hit any of them).

### 13. Surface helpers calling stdlib weren't imported at the file top

A `Window`/`Screen` *helper function* that only *calls* a stdlib namespace
(e.g. `FileSystem.Read` inside `LoadConfig()`) didn't get its
`use vbr_stdlib::{FileSystem}` — the file-top `use` was built from declared
*types* (`stdlib_types_declared`) and event bodies, but a call receiver inside a
plain function is neither. Json/Database only worked because they were also
field types. Found building the idea engine (`FileSystem.Read` in a config helper
→ E0433).

**Fix** (`tui.rs`/`gui.rs`): the file-top `use` is now built from the marks —
`stdlib_used(diags)` — after `emit_shared_items` has run `note_builtins` on the
helpers (which marks call receivers), plus `State` inits and event bodies
(collected Await-aware via `collect_event_stdlib`). One `use` line per native
program/window, so nothing imports twice. Zero snapshot churn on GUI (every GUI
example's stdlib is event-only); TUI async examples just moved their `use` to
the file top.

### 11. Surface programs never ran the stdlib-type marking pass

`mark_stdlib_types` (which turns declared types like `As Database` /
`Vec<Json>` into Cargo features + the `use vbr_stdlib::{…}` line) ran only on
the *plain-program* path — the GUI/TUI/web dispatches returned before it. A
Screen program with `ByVal db As Database` helper functions would have compiled
against a stdlib built with the wrong features, with no `use` for the types its
items name (the TUI emitter had no file-top vbr_stdlib `use` mechanism at all —
only scope-local ones inside `fn main` for event calls).

**Fix** (`transpiler.rs`/`tui.rs`/`gui.rs`): a shared `stdlib_types_declared`
scan (function signatures/bodies, struct fields, and now `State` fields) marks
features and feeds a file-top `use` in the TUI/GUI emitters. Event-body calls
keep their scope-local imports — the two never collide.

---

## Project 2: Cellular Automata Lab (Conway's Life) — 2026-07-10

Len built a Game of Life (`projects/1_cellular_automata_lab` in the projects
checkout): flat-Vec grid, B/S rule strings, text serialisation, a TUI with
cursor/step/run. It surfaced a *cluster* of real bugs. Three of its findings
were already fixed by then-uncommitted work (`Chr`/`vb…` constants, the
sync-Screen stdlib `use` from #13, the `build/` cwd = #15) — the checkout
predated them.

### 16. Event state-rewrite skipped loop bodies (fixed)

`rewrite_stmt` (the `field` → `state.field` pass over event bodies) recursed
into `If`/`Match` but had a catch-all that swallowed `For`, `For Each`,
`Do…Loop`, `Set`, `Dim (a,b) = …`, and `Return` — so `grid[i] = 1` inside an
event's `For` emitted bare `grid` → E0425. Any loop in any event on any
surface. **Fix** (`surface.rs`): recurse through every statement form that
carries expressions or bodies. Zero snapshot churn (no existing example looped
in an event — that's how it hid). Example: `examples/tui_life.vbr`.

### 17. State initialisers skipped the resolver (fixed)

A `State` field initialised by a *call with arguments* emitted the args raw —
no `&` on a ByVal `Vec`/struct, no owned strings, no numeric casts:
`Dim status As String = StatusLine(RuleLife(), 0, SeedGrid(), 5, 5)` → five
type errors. **Fix** (`surface::render_init`): the initialiser runs the
ordinary resolver pass first, as a synthetic `Dim` of the field's type — an
initialiser and a function-body `Dim` are the same language. All three emitters
(Window/Screen/Page) share it. Zero snapshot churn.

### 18. Forwarding a ByRef param emitted `&mut` again (fixed)

`PlaceBlinker(ByRef grid)` calling `SetCell(ByRef grid)` emitted
`setcell(&mut grid, …)` where `grid` is *already* `&mut Vec<_>` → E0596
("not declared as mutable"). **Fix** (`resolver.rs`): `Binding` now records
ByRef-collection/struct params; forwarding one passes it bare and Rust
reborrows. ByRef *primitives* keep their `&mut *n` (already correct).

### 19. An owned `String` isn't a `Pattern` (fixed)

`digits.contains(ch)` with `ch` from `Mid(…)` (an owned `String`) failed —
`str::contains` wants a `Pattern` (`&str`, `char`, `&String`…). **Fix**
(`resolver.rs`): on a string receiver, an owned-String argument to
`contains`/`starts_with`/`ends_with` is borrowed (`&ch`); literals and slices
stay bare. (The Vec arm of `contains` already borrowed — strings didn't.)

### 20. `Dim x = lines[i]` moved the element (fixed)

Indexing moves the element out of a `Vec` (E0507), so a `Dim` from an index
failed for any non-Copy element. **Fix** (`resolver.rs`): the `Dim` clones —
`String`, nested collections, and user structs (they derive `Clone`); numbers
and Booleans are Copy and stay bare. VB assignment semantics (a copy) preserved.

### 21. A `Dim`'d For counter warned as unused (fixed)

`Dim dy As Long` + `For dy = -1 To 1` — Option Explicit muscle memory — emitted
a `let dy: i64;` that the `For`'s own binding shadows → unused-variable warning
in otherwise clean output. **Fix** (`transpiler.rs`): the dead `Dim` is elided
(scalar, no initialiser, never assigned outside a `For` that binds it). Spec
notes the VB6 difference: the counter doesn't exist after the loop.

### 22. Cross-module `Module.Const` isn't lowered (fixed — slice 2)

`Life.WIDTH` parsed as field access on a value; the resolver never tried it as
a sibling module's constant → "cannot find value life". **Fix**: with the
module's interface (see #23), `Life.WIDTH` → `crate::life::WIDTH` for a
`Public Const` (which emits `pub const`). A private const, or an unknown
member, gets a teaching error (the latter notes that a function call needs
parentheses — the VB6 no-parens habit).

### 23. Cross-module calls skip the ByVal/ByRef rewrite (fixed — slice 2)

`Life.StepLife(g)` rewrote by *name* to `crate::life::steplife(g)`, but the
argument treatment looked the signature up under the qualified name and missed —
no `&`/`&mut`, no coercions, so `Vec`/`String` args couldn't cross modules. Root
cause: `compile_module` compiled each file alone, knowing only sibling module
*names*. **Fix — the agreed two-pass project compile**: pass 1
(`vbr::module_interface`) parses every `.vbr` module and harvests its public
surface (fn signatures with modes/types + consts; private names kept for the
visibility diagnostics); pass 2 compiles each file with the sibling interfaces
in the resolver (`ProjectInterfaces` in `Ctx`), and the *same* `apply_fn_sig`
that treats local call arguments treats qualified ones — `&mut` ByRef, `&`
ByVal collections/strings, return types feeding inference. Calling a `Private`
function cross-module now earns a teaching error instead of rustc's raw
"function is private". Scope drawn deliberately: **types don't cross modules
yet** (structs/enums stay file-local — own slice), and verbatim `.rs` modules
stay name-only (no VBR interface to harvest). Example + guard:
`examples/life_project/`, `crossmodule_interfaces_compile`. Zero churn in the
existing geometry/mixed project snapshots (they cross only primitives).

### 25. An unread `For` counter warned as unused (fixed)

Found *building the fix for #23*: VB's "repeat N times" idiom
(`For i = 1 To WIDTH * HEIGHT` with `i` never read in the body) emitted
`for i in …` → rustc's unused-variable warning in otherwise clean output.
**Fix** (`transpiler.rs`): the emitter scans the loop body (via the
`collect_stmt_idents` walker, moved from gui.rs and completed — Do conditions,
Match guards, Set/destructure, inline Rust/Python as opaque "uses everything")
and names the binding `_` when the counter is never read. Companion to #21 —
the two together make the whole Dim-and-For VB6 habit warning-free.

### 24. `Screen`/`Window` programs ignore sibling modules (OPEN — slice 3)

`surface::build_tables` hard-codes `modules: HashSet::new()` and the surface
emitters never emit `mod life;` — a multi-file project can't put its `Screen`
in `main.vbr` and its logic in `life.vbr`. Depends on #23's project table;
phase 2 of the same work.

### Watch: a temporary struct literal moves its String fields

`FormatRule(Rule { birth: birthPart, … })` then reusing `birthPart` is a
use-after-move — a genuine ownership lesson, left to rustc's translated error
(the backstop). If it keeps biting, consider a teaching note.

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

### 12. Optional Rust-style escaped strings (todo, not a bug)

VBR strings have no backslash escapes, deliberately — a VB dev writes Windows
paths like `"C:\new\table"` constantly, and Rust escaping would silently corrupt
them. So the default string must stay literal. Newlines/tabs are covered by
`Chr(n)` and the `vb…` constants (`vbNewLine`/`vbLf`/`vbCrLf`/`vbCr`/`vbTab`).
*Someday*, if a Rust-escaped literal is genuinely wanted, add it as an **opt-in
prefixed form** (a distinct syntax, so normal strings stay path-safe) rather
than changing the default. Parked by choice, not blocking anything.

### 14. A comment between `Screen`/`Window` members is rejected

Inside a `Screen`/`Window` block, a `'` comment line between members (e.g. above
an `Event`) is a parse error: "Unexpected Comment … expected Title, State, View,
`On Key`, `Every`, Event, or `End Screen`." Comments are fine inside function
bodies and above the block, just not between surface members — so you can't
document individual events in place. Small parser papercut; the block member
loop should skip comment tokens. Worked around by moving the comment.

### 15. Projects don't copy data files into `build/`

A project that reads a data file at runtime (the idea engine reads
`config.json`) looks for it in the *current working directory*, which for
`vbr runproject`/`build` is the generated `build/` folder — not the project
folder where the file lives. Today you copy `config.json` into `build/` by hand.
`runproject`/`build` could copy non-`.vbr`/`.rs` files (or a declared data dir)
into `build/`, or the run could set the working directory to the project root.
Found building the idea engine; documented in its README meanwhile.

### 7. No map literal — "no headers" / empty-HashMap calls are clunky

The new list literal (`[]`) covers `Vec`, but there's no `HashMap` literal, so
`Http.Post(url, body, [])` can't express "no headers" — the caller must
`Dim h As HashMap<String, String>` and pass it unused. First concrete argument
for a future `{k: v}` / `{}` **map literal** — consistent with having reserved
`{}` for maps when choosing `[]` for lists. Not yet built.

---

## Capability: SQLite stdlib namespace — BUILT (slices 1 + 2)

`Database` is in the stdlib (`stdlib_spec.md` §8): `Open` / `Execute` / `Query`
(rows as `Json`) / `LastInsertId`, rusqlite bundled, `database` feature.
Verified end-to-end: `examples/database.vbr` runs — typed reads, NULL lineage
roots via `IsNull`, parent links via `LastInsertId`.

**Slice 2 — fallible `State` initialisers — BUILT.** A `State` field may be
initialised by a fallible call (`Dim db As Database = Database.Open("ideas.db")`,
or your own `Result`-returning function). State is then built by a generated
`init() -> Result<State, String>` run *before* the window/terminal starts;
failure prints `could not start: <why>` and exits — never a half-alive UI.
All native surfaces (Window + Screen); browser surfaces fence it with a
teaching error. Verified: `examples/tui_ideas.vbr` (a Database in Screen state,
events via `state.db`, helper functions borrowing `&Database`) compiles clean,
and the failure path prints and exits before the terminal is touched. The
considered-and-shelved alternative (`Run`-args seeding from `Main`) is
documented in stdlib_spec §8 Deferred — only needed for custom failure UI.
