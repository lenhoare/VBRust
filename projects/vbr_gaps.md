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

*(2026-07-11: the sibling case in a **list argument to a stdlib method** —
`db.Execute(sql, [id, name])` where `id`/`name` are `ByVal As String` (`&str`)
params. The list emitter already owns a string *literal* element, but a `&str`
*variable* rendered bare → mismatch with the `Vec<String>` slot, so you had to
`.clone()` or `CStr(...)` each one. **Fix** (`resolver.rs`, the `stdlib_recv`
arg block): a non-literal `&str` element (`VType::Str`) in a `List` argument to a
stdlib method is owned with `.to_string()`; literals stay with the emitter and
owned-String locals move in as-is (no needless clone). Guarded warning-free:
`examples/database.vbr` gained an `AddScored(ByVal text, ByVal score)` helper that
inserts its `&str` params through the params list — compiled *and run* in the
guard. Zero churn on the existing literal/`CStr` inserts.)*

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

*(2026-07-11: the scan looked only at **event bodies**, so a `HashMap` `Dim` in a
surface **helper function/method** — like an idea engine's `ChatComplete` building
request headers — still compiled to code referencing an unimported `HashMap`.
Renamed `event_std_imports` → **`surface_std_imports(events, helpers)`** and it now
scans helper bodies too; the three emitters pass `&program.functions` through
`emit_window`/`emit_screen`/`emit_page`. The old workaround — a dummy
`Dim _headers As HashMap` in an event to trigger the scan — is no longer needed.
Guarded warning-free: `examples/tui_ideas.vbr` gained a `BonusPoints()` helper
whose only HashMap lives in that plain function, called from the `Add` event.)*

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

*(2026-07-11: the numeric `Cxxx` family is now mapped — see "`Val` semantics" in
the fixed-bugs section below.)*

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
yet** (structs/enums stay file-local — became its own slice, #28 below), and
verbatim `.rs` modules stay name-only (no VBR interface to harvest). Example +
guard:
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

### 24. `Screen`/`Window` programs ignore sibling modules (fixed — slice 3)

`surface::build_tables` hard-coded `modules: HashSet::new()` and the surface
emitters never emitted `mod life;` — a multi-file project couldn't put its
`Screen` in `main.vbr` and its logic in `life.vbr`. **Fix**: the three surface
emitters (Window / native+web Screen / Page) take the module set + interfaces,
`build_tables` carries them, the entry emits `mod` declarations
(`surface::emit_mod_decls`, shared), and `resolve_event_body` resolves against
them — so a **State initialiser** (`Dim grid As Vec<Long> = Life.NewGrid()`),
an **event** (`Life.SetCell(grid, …)` → `crate::life::setcell(&mut state.grid,
…)`), and a **helper function** all call siblings with the full local argument
treatment. `fallible_init` also learned cross-module Result-returners, so
`Dim db As Database = Store.OpenDb()` gets the clean-bail `init()` like a local
call would. Example + guards: `examples/life_screen/` (Screen in main.vbr,
logic in life.vbr; snapshot `screen_project_matches_snapshot` + a real project
build in the compile guard). Zero churn in single-file surface snapshots
(empty module set = no-op).

### 28. Types don't cross modules (fixed — types are project-global, VB6-style)

The scope line drawn in #22/#23, erased 2026-07-11. The design question was
whether types should be module-qualified like functions (`Life.Rule`) — answer:
**no**. In VB6 a Public UDT/Enum in any module was global by bare name
(`Module.Type` syntax never existed); in idiomatic Rust, types are *imported*
(`use crate::life::Rule;`) and used bare while functions stay path-qualified.
Both agree, so VBR does both at once. **Fix**: `ModuleInterface` also harvests
public `Type` fields, `Enum` names, and public methods (with `&mut self`-ness);
`merge_sibling_types` folds them into the same bare-name tables local types use
(so inference, ByVal `&`, struct literals, and `Match` patterns just work,
plain programs and surfaces alike); `add_sibling_type_uses` scans the
*generated* code (strings/comments blanked) and inserts one
`use crate::module::Name;` per foreign type actually mentioned. Rules: local
definition wins; same name Public in two files = ambiguity error on use; a
sibling's Private type or a module-qualified type (`Life.Rule`) each earn a
teaching error. Fields another file touches must be `Public` → `pub`.
Examples: `examples/life_project/` (Rule + CellState cross, method + Match),
`examples/life_screen/` (a Screen holds a sibling's type in State). Guards:
`crossmodule_interfaces_compile`, `crossmodule_type_diagnostics`.

### 29. A Rust reserved word as a field/variable name breaks the build (OPEN)

Found probing #28: `Dim box As Rect` in a Screen's State emits
`box: Rect` — `box` is a *reserved* Rust keyword, so the generated struct
doesn't compile (raw `r#box` isn't even allowed for it). Same risk for any
keyword VBR's lowercasing lands on: `type`, `match`, `loop`, `move`, `ref`,
`impl`, … The fix wants a rename pass (`box` → `box_`, with a one-time note)
or a teaching error listing the reserved names. Rare in practice, ugly when
hit — the rustc error at least points at the right line today.

### 26. View expressions can't read a sibling module's constant (OPEN)

`Text "grid " & Life.WIDTH & " wide"` in a `View` emits broken `life.width` —
views are declarative and never run the resolver, and their lightweight
expression rewrite doesn't know modules. Events, State initialisers, and
helper functions all resolve `Life.WIDTH` fine — so the workaround is to
mirror the value into state or read it through a helper. Fixing it means
teaching the view expression path about module consts (a view-tree rewrite
pass) — small, self-contained, deferred until it actually bites someone.

### Watch: a temporary struct literal moves its String fields

`FormatRule(Rule { birth: birthPart, … })` then reusing `birthPart` is a
use-after-move — a genuine ownership lesson, left to rustc's translated error
(the backstop). If it keeps biting, consider a teaching note.

---

## Architecture: the surface path vs the plain-function path — 2026-07-11

A design conversation (not a single bug) about *why* bugs like #10 and #16 keep
recurring in the same shape. Diagnosis: VBR has **one shared resolver core**
(`resolve_stmts`) and **one shared statement emitter** (`emit_stmt`), but the
surface path (Window/Screen/Page events) bolts a surface-only **post-pass**
between them — the bare-field → `state.field` rewrite (`rewrite_stmt` /
`rewrite_expr_with` / `coerce_state_strings`) plus the genuinely-separate `Await`
split. That post-pass is a second tree-walker, and every coverage hole in it
(a node variant its `_ =>` catch-all silently dropped) is a #10/#16-shaped bug.
The async model staying deliberately narrow (`Await` only as a top-level `Match`
/ `Dim` value) is a *choice* — VBR is a teaching tool, not a second Rust — so
"Await can't sit in `If`" is a known trade, not an accident.

Agreed three-tier plan:

- **Tier 1 — make the rewrite passes total (DONE 2026-07-11).** Removed the
  silent-drop `_ =>`/`other =>` wildcards from `surface::rewrite_expr_with`,
  `surface::rewrite_stmt`, `surface::coerce_state_strings`, and the sibling
  `gui::rewrite_canvas_stmt`; every `Expr`/`Stmt` variant is now listed, so
  Rust's exhaustiveness check *forces* a new node to be handled instead of
  dropped. Two real holes closed: `rewrite_expr_with` never recursed
  `TupleIndex` (defensive — a tuple *State* field is rejected upstream today, so
  not yet reachable), and `coerce_state_strings` only descended `If`/`Match`, not
  `For`/`ForEach`/`DoLoop` — so a `String` field assigned a literal inside a loop
  in an event emitted `state.s = "x"` (missing `.to_string()`) → won't compile.
  Proven with a probe: `status = "running"` inside a `For` now emits
  `state.status = "running".to_string();`. Zero snapshot churn; compile guard
  green. This *closes the class* at compile time — the "million patches" fear.
- **Tier 2 — unify (deferred, deliberate).** Delete the rewrite post-pass by
  teaching the shared emitter about "receiver fields" (emit `state.field` inline,
  as it already does `Me.field` for methods), so there is one walker, not two.
  Medium work (thread a receiver-fields context through `render_expr`/`emit_stmt`,
  manage snapshot churn); do it as a standalone refactor when there's appetite,
  not forced by any one bug.
- **Tier 3 — async model (its own project).** A real `Await`-anywhere lowering
  (a small CPS/state-machine transform) would make every "Await can't sit in X"
  vanish together. Larger; separate design; only if the narrow model bites hard.

**Same class, found later (2026-07-11): a focusable `List`/`Input`/`Table` inside
a view `Match`/`If` lost its state field.** `tui::collect_focusables`'s `walk`
recursed `Column`/`Row`/`Constrained` but its `_ => {}` swallowed the `Match` and
`If` view nodes — so a `List` nested in a `Match` arm was never collected, its
`<field>_state` (the ratatui `ListState`) never declared/inited/key-wired, yet the
*renderer* (which does recurse into `Match`/`If`) still emitted
`state.<field>_state` → a reference to an undeclared field. Textbook coverage hole:
one walker recurses a node the sibling walker drops. **Fix**: `walk` now recurses
into `Match` arms and `If` branches/else, and lists every leaf `ViewNode`
explicitly (no `_`), so a future container-shaped view node must decide there.
Proven by stash-diff (pre-fix: `<field>_state` declared 0×, rendered 1×) and a
real ratatui build. Guarded: `examples/tui_list_tabs.vbr` (two Lists in two Match
arms, both fully wired). The lesson reinforces Tier 1: **exhaustive matches in
every structural walker** — the view tree has its own family of them
(`collect_focusables`, `render_view_node`, the keymap collector), and they must
agree on which nodes carry children.

---

## `Val` semantics + the strict `Cxxx` conversions — 2026-07-11

Reported as a bug ("`Val(x)?` error type isn't `String`"), but interrogating it
found the real problem was deeper: `Val(x)` lowered to `x.parse::<f64>()` — a
**`Result<f64, ParseFloatError>`**, which is (a) un-VB (VB's `Val` is *infallible*,
returns a `Double`, `0` on garbage) and (b) already broken against our own
`examples/string_options.vbr` (`Dim num As Double = Val("3.14")` assigned a
`Result` to an `f64`; hidden because that example is transpile-only). Len's
instinct — "`Val` should return a number, the strict conversion should be the
fallible one" — was right, pinned to the wrong name (there's no `CVal` in VB6;
the strict family is `CInt`/`CLng`/`CDbl`, which raised a runtime "type mismatch"
→ a natural `Result`).

**Fix — two homes, not one forced path:**

- `Val(x)` → `x.trim().parse::<f64>().unwrap_or(0.0)` — a lenient `Double`, `0` on
  non-numeric text, whitespace ignored, **never fails**. Inference now types `Val`
  as `Double`, so `Dim n As Long = Val(x)` gets the automatic `as i64` cast. No
  more `Result`, so nothing to `Match`/`?` — the reported awkwardness is gone
  because the operation was never fallible in VB.
- `CDbl`/`CLng`/`CInt` → `x.trim().parse::<…>().map_err(|e| e.to_string())` — the
  *strict* conversions return `Result<_, String>`, so `?` propagates and `Match`
  branches, joining VBR's String-error convention. `ignored_result` now flags a
  dropped `Result` from these (teaching diagnostic), and `Val` is removed from it.

This also closes the "`Cxxx` family unmapped, falls through to rustc" half of #9.
`transpiler.rs` (`lower_builtin`), `resolver.rs` (`builtin_vtype` + `ignored_result`).
Scope: **string-parse only** — VB's number→number rounding (`CInt(2.5)` = 2,
banker's rounding) is a later refinement, noted in both specs. Example +
compile-guarded snapshot: `examples/conversions.vbr` (in `HAPPY`); `string_options`
snapshot updated to the infallible `Val`. Rust-checked end to end (`Val("  42  ")`
= 42, `Val("nonsense")` = 0, `CDbl(x)?` compiles in a `Result<_,String>` fn).

---

## Two "bugs" that are teaching points, not fixes — 2026-07-11

Reviewing the idea-engine findings, two were correctly reclassified (Len: "they
aren't really bugs, they're teaching points") — the behaviour is *by design*; what
was missing was clear teaching. So we sharpened the diagnostic/docs, not the codegen.

- **`Await` can't sit inside `If`/`For`/`Match`.** Deliberate (see the Tier 3
  note): a top-level `Await` lowers to a plain kick-off/continuation pair with no
  hidden state machine — VBR keeps async simple on purpose. The rough failure was
  already a single clean teaching error (`await-position`), now **enriched** to say
  *why* (top-level only) and *how to fix it* (guard before the `Await` — `If busy
  Then Return` / set a flag — or move the guard into the awaited helper). Design
  note added to `tui_spec.md` §7. `surface.rs`.
- **Match-arm bindings are lowercase.** A pattern is verbatim Rust, so `Ok(runId)`
  keeps `runId`, but the body lowercases names (`runId` → `runid`) and the two stop
  matching — worst in an `Await` continuation. This is the case-insensitive-VB /
  case-sensitive-Rust seam, a genuine lesson, not a bug to paper over. Sharpened
  the teaching note in `language_spec.md` §Match and `language_reference.md` §3
  (write `Ok(runid)` in both places). No codegen change.

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

**Update (2026-07-11): the *multi-line* half of this is now built — `Text …
End Text`.** The block is the most-literal string form (backslashes/quotes/braces
stay verbatim — ideal for JSON/SQL/prompts), dedents to its shallowest line, and
lowers to an ordinary `Expr::Str` (`\n` between lines is the *only* way a newline
enters a VBR string; a quoted literal still never grows escapes). It opens only
when bare `Text` ends its line **and** the next line indents under it — so the
`Text` widget, `.Text` members, and a variable named `text` at end of line are
all untouched (VB is case-insensitive, so `text`/`Text` are one word — the
indentation guard is what tells a block from a value). What remains parked is the
*inline* escaped literal (`\n`/`\t` inside a one-line `"…"`), still deferred to a
future opt-in prefix. Examples: `examples/text_block.vbr`; guarded in `HAPPY`.

### 14. A comment between `Screen`/`Window` members is rejected (fixed)

Inside a `Screen`/`Window`/`Page` block, a `'` comment line between members
(e.g. above an `Event`) was a parse error — you couldn't document individual
events in place. **Fix** (`parser.rs`): both surface member loops skip comment
tokens (they aren't carried into the generated Rust — member-level comment
preservation would be its own small feature). `examples/life_screen/main.vbr`
documents its events in place as the regression test. *(2026-07-11: the same
fix applied to `State` blocks — a comment between `Dim` fields was still a
parse error; `life_screen`'s State now carries one as the regression.)*

### 15. Projects don't copy data files into `build/` (fixed)

A project that reads a data file at runtime (the idea engine reads
`config.json`) looked for it in the *current working directory* — the generated
`build/` folder — so you copied it there by hand. **Fix** (`main.rs`): a folder
project's build copies its data across on every build (the project folder is
the source of truth): top-level files that aren't sources (`.vbr`/`.rs`) or
docs (`.md`), and whole subdirectories (`data/…`), skipping dotfiles and
`build/` itself. So `config.json` is just *there*, and a `data/rules.txt` opens
as `"data/rules.txt"`. A failed copy warns instead of killing the build.
Guarded: the compile guard asserts the idea engine's `config.json` lands in
`build/` and its `README.md` doesn't.

### 27. TODO: example + docs for calling C libraries via inline Rust

Not a gap — a documentation debt. The capability already exists: Rust speaks
the C ABI natively, so a C library is called with `Use <binding-crate>
<version>` + an inline `Rust` block + an **opaque handle** for any stateful
library object — no new VBR feature needed. What's missing: a worked example
(a small binding crate — `libc` is the zero-setup candidate; a llama.cpp
binding like `llama-cpp-2` is the motivating one) and a section in
`inline_rust_spec.md` ("Calling C libraries") walking the pattern: pick the
binding crate, `Use` it, hold the handle, thread it through blocks. Companion
idea parked with it: a native inline `C … End C` block (via the `cc` crate +
generated build.rs) — motivated by the do-it-all identity, not by any library.

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

---

## Capability: test harness — `vbr test` — BUILT (2026-07-11)

Framed by Len as the **trust protocol** for the collaboration model: "I write the
code and the tests, you read the tests as a specification you can verify me
against." Conway had faked this with a `tests.vbr` called from `Main`; this makes
it first-class.

- **`Test "description" … End Test`** blocks (top-level, ordinary statement body)
  and **`Assert <expr>`**. The operator picks the Rust assertion — `=` →
  `assert_eq!`, `<>` → `assert_ne!` (operand-level failure messages, the point
  for the trust protocol), else `assert!`. `Test`/`Assert` are recognised only at
  their positions (top-level / statement start), not reserved globally.
- Lowering: each `Test` → a `#[test] fn` (deduped description slug) in a
  `#[cfg(test)] mod vbr_tests`, body resolved exactly like a function body. So
  `vbr run`/`build` compile them out; only `vbr test` runs them.
- **`vbr test [path]`**: generates the project (tests active), `cargo test --no-run`
  (compile errors translated to `.vbr` like `vbr run`), then `cargo test`, and
  translates libtest output → `✓ / ✗` by **description**, in **source order**,
  with operand values and the **`.vbr` line** on a failure. Exits non-zero on any
  failure (CI-ready).
- **Placement — `<module>.test.vbr` sibling files** (the agreed model: the suite
  reads as a module's contract, gathered, not scattered). A `.test.vbr` file is
  discovered only by `vbr test` — declared `#[cfg(test)] mod <name>_test;`, skipped
  entirely by `vbr run`/`build`, so tested-only logic never counts as unused in the
  app build. Tests call the code by qualified name (`Life.StepCell`), so the tested
  function must be `Public` — you test the public surface. Inline `Test` blocks in
  any file also work; the sibling file is the recommended home.
- Scope: **logic, not surfaces** (test pure functions / public API, not
  GUI/TUI/web rendering). Deferred: setup/fixtures, custom `Assert` messages.

Files: `ast.rs` (`TestBlock`, `Stmt::Assert`, `Program.tests`), `parser.rs`
(`parse_test`, the `Assert` stmt arm), `resolver.rs` (`Stmt::Assert` resolves its
expr), `transpiler.rs` (`emit_tests`, `test_fn_names`, the `Assert` emit arm),
`lib.rs` (`TestInfo` on `Compiled`), `main.rs` (`cmd_test`, `.test.vbr` discovery
+ `#[cfg(test)]` mod injection, libtest-output translation). Spec: `testing_spec.md`
(+ `language_spec.md` §13, `language_reference.md`). Example: `examples/tests.vbr`
(HAPPY snapshot). Guard: `vbr_test_runs_specs` (single-file + `.test.vbr`, pass and
fail, and the app build stays warning-free).
