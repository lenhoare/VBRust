# VBR Field Findings

Bug/quirk log from building test projects in this folder. Written 2026-08-16 onward.
No Rust/transpiler changes made — observations only.

Kinds: **bug** (wrong behaviour/crash), **quirk** (works but surprising),
**ergonomic** (friction), **positive** (worked better than expected).

Format: `F-NNN | date | kind | project | title`, then repro and notes.

---

## F-001 | 2026-08-16 | positive | bottles
First program compiled and ran correctly on the first attempt. `For n = 99 To 1 Step -1`
works and earns a teaching line ("becomes `(lo..=hi).rev()`"). VB-style string building
with `&`, `Return` inside `If`, and empty-string `Debug.Print ""` all behaved as a VB6
programmer would expect.

## F-002 | 2026-08-16 | quirk | bottles
Wrote the VB6 habit `Dim n As Long` *before* `For n = 99 To 1 Step -1`. It compiles
silently — the declaration is dropped and the loop makes its own `n`. The guide says
"drop any separate Dim", but the compiler says nothing. A gentle hint ("the For loop
creates its own `n` — this Dim is unused") would teach the rule at the moment of the
mistake. Related: the emitted Rust carries redundant casts at the call sites
(`wallline(n as i64)` where `n` is already `i64` from the range).

Repro: `qwenprojects/bottles.vbr`, `vbr emit` shows `for n in (1..=99).rev()` followed
by `n as i64` casts.

## F-004 | 2026-08-16 | positive | guess
Wrote the VB6 reflex `Int(Rnd() * 100) + 1`. The compiler refused it with a *great*
message: explains randomness lives in the `rand` crate, and shows the exact
`Use rand 0.8` + inline-Rust snippet. Applied verbatim, compiled first try. This is
the teaching model working as designed. Also confirmed: single-line
`If x Then Debug.Print ...` works, and `Handle` + `Continue` inside both `For` and
`Do` loops works.

## F-005 | 2026-08-16 | bug | guess
`InputBox` on exhausted stdin (piped input ended, or Ctrl+D) returns `""` forever
instead of signalling EOF. Any `Do` loop that reads and Handles bad input becomes a
busy infinite loop: `printf 'abc\n50\n' | vbr runproject guess` printed the rejection
message ~100×/second until killed. Generated code: `read_line` returns `Ok(0)` at EOF
and the result is discarded (`let _ =`), leaving the buffer empty. This affects every
interactive program — `goodexamples/ledger` spins the same way (`line = ""` →
`Continue`). Suggestion: `Ok(0)` should RaiseError ("end of input") so a program can
`Handle` it and quit; or InputBox returns `Option<String>`.

Repro: `printf 'abc\n' | timeout 5 vbr runproject qwenprojects/guess` (watch it spin).
Workaround used in guess: treat `line = ""` as quit.

Also minor: emitted Rust shows `line.trim().trim()` — my explicit `Trim()` plus CLng's
own internal trim. Harmless, just redundant codegen.

## F-006 | 2026-08-16 | bug | caesar
`Len` counts **bytes**, but `Mid` counts **characters** — the two disagree on any
non-ASCII string, which breaks the most idiomatic VB string loop
(`For i = 1 To Len(s)` + `Mid(s, i, 1)`):

```
Len("café") = 5        Mid("café", 4, 1) = "é"     (char 4 — correct by chars)
Len("日本語") = 9      Mid("日本語", 2, 1) = "本"
```

Generated loop bound is `for i in 1..=text.len() as i32` (bytes) while Mid lowers to
`chars().skip(i-1).take(1)` (chars), so the extra iterations yield `""`. Harmless in
caesar (appending empty strings), wrong anywhere positions matter (palindromes,
column extracts, wrapping). The teach line even says Mid counts chars "so it stays
correct on any text" — `Len` undercuts that. Suggested fix: `Len` → `s.chars().count()`.

Repro: `qwenprojects/.scratch` (temporary) — printed values above.

## F-007 | 2026-08-16 | quirk | caesar
`Chr` is 8-bit only: `Chr(8364)` fails at transpile time with a clean VBR-level
message ("literal out of range for `u8`"). Faithful to VB6's `Chr` (the wide version
was `ChrW`, which VBR doesn't have), but sits oddly next to Unicode-aware `Mid`.
Either add a wide variant or document the limit where the Mid teach line appears.
`Asc` returns the full code point, so `Asc`/`Chr` don't round-trip past 255.

## F-008 | 2026-08-16 | ergonomic | calendar
`Space()` (VB6 `Space$`) doesn't exist — had to hand-roll a `Spaces(n)` helper.
`String(n, ch)` is absent too. Small but frequently-missed VB builtins worth adding.
Side note: the error shown for the missing function was rustc's "cannot find function
`space`" with a teach hint about *lowercase spelling inside Rust blocks* — the hint
doesn't fit "this builtin doesn't exist" and points the user the wrong way.

## F-009 | 2026-08-16 | quirk | calendar
`Return n` where the function is `As String` and `n` is `Long` is rejected
("expected `String`, found `i64`"). VB6 would have coerced. `Return "" & n` is the
workaround. The teach hint talks about declared `As` types generally; a more targeted
suggestion ("to convert, use `"" & n` or a conversion function") would help — this is
a hit every VB programmer will take within their first hour.

## F-010 | 2026-08-16 | positive | calendar
One-line `If IsLeap(y) Then Return 29 Else Return 28` *inside a Match arm* parses and
works. Sakamoto's day-of-week with `Int(yy / 4)` truncation, `Mod`, `Vec` literals and
indexing all behaved; three test months came out correct against a real calendar
(including leap-year Feb 2024).

## F-011 | 2026-08-16 | ergonomic | textstat
No `Split(s, sep)` — VB6 had one, so this is a conspicuous gap;
`goodexamples/ledger/money.vbr` hand-rolls SplitSpaces for the same reason.
`Join` likely absent too. Candidate builtins: `Split(s, sep)`, `Join(parts, sep)`,
`Replace(s, a, b)`, `IndexOf(s, sub)` (InStr exists, but returns 1-based VB style —
fine, just noting the set).

## F-012 | 2026-08-16 | positive | textstat
A non-trivial program worked first try: `FileSystem.Read` of a data file (the
copy-into-build behaviour works), `HashMap.get(w) Is Some(c)` if-let, `freq.Len()`,
`For Each w, c In map`, building `Type` literals from borrowed iteration values,
Vec index assignment with non-Copy types (`list[i] = list[best]`), and a hand
selection sort. Word counts came out correct. Also: the teach line explaining
HashMap string keys get `.to_string()` automatically is exactly the kind of
invisible magic worth surfacing.
Naming observation: Vec methods are VB-capitalised (`Push`, `Len`, `Clone`) while
HashMap methods are Rust-lowercase (`insert`, `contains_key`, `get`) — the mix reads
oddly in one file; both are defensible, but a consistent style would feel calmer.

## F-013 | 2026-08-16 | bug | calc
Unary minus on a Double method result doesn't compile: `Return -Me.ParseFactor()`
generates a subtraction from an *integer* zero — rustc: "cannot subtract `f64` from
`{integer}`". Workaround: `Return 0.0 - Me.ParseFactor()`. Unary minus on integer
expressions presumably works (0 - x with integer inference), so the lowering should
type the zero from its operand.

## F-014 | 2026-08-16 | quirk | calc
String ORDER comparisons fail: `c >= "0"` (c is String) → "expected `String`, found
`&str`", while string EQUALITY (`Mid(...) = " "`) works fine. Rust's String has no
PartialOrd<&str>, so the transpiler would need `.as_str()` on one side. Workaround:
compare `Asc(...)` codes. VB6 allowed string ordering comparisons, so VB fingers will
type this.

## F-015 | 2026-08-16 | bug | calc
`vbr py` and `vbr c` emit calls to `mid`, `len`, `asc`, `chr` but provide no runtime
definitions for them: Python dies with `NameError: name 'mid' is not defined`; C fails
to link (`undefined reference to 'asc'`, implicit-declaration warnings). Scope checked:
`bottles.vbr` (no string builtins) transpiles and runs identically under BOTH targets,
so the gap is specifically the string-builtin runtime for py/C, not the targets
generally. (targets_spec promises byte-for-byte parity with `vbr run` — presumably only
for the covered subset.)

## F-016 | 2026-08-16 | positive | calc
The testing story is excellent. `vbr test` printed all 9 descriptions with ✓ and
"N passed"; exit code 0. `Handle` inside a `Test` block works (error-path testing:
call, Handle, Assert on the message, Return to skip the success Assert). Method
receiver inference worked across a recursive descent parser — `&mut self` methods
(ParseExpr assigns Me.pos) freely calling `&self` methods (Peek) with no annotations.
Qualified cross-module calls from both main.vbr and calc.test.vbr worked identically.

## F-017 | 2026-08-16 | positive | pascal
Naming a variable `next` is refused with a kind message that suggests `next_`.
`pascal.vbr` rendered 64 exact rows of Pascal's triangle and a perfect Sierpiński
triangle first try; passing a Double expression (`(Rows - r) / 2`) into a Long
parameter got the promised invisible narrowing cast.

## F-018 | 2026-08-16 | bug | primes
`Dim a As Long` then passing `a` to a `ByRef` parameter fails at rustc level:
"used binding `a` isn't initialized". The guide promises `Dim count As Long` is
"declared, 0 by default" — that holds when the first use is an assignment, but not
when it's a ByRef call. Vec defaults (`Dim out As Vec<String>` then `.Push`) DO
work. So numeric defaults need to be emitted when the first use is `&mut`. The VB6
pattern `Dim a As Long: Call F(a)` is extremely common; this will bite. Workaround:
`Dim a As Long = 0`.

## F-019 | 2026-08-16 | bug | primes
`For m = p * p To limit Step p` (VARIABLE step) fails: "expected `usize`, found
`i64`" — the variable-step lowering appears to use a usize counter/step and doesn't
type it to the loop's Long. Constant steps are fine: `Step -1` (bottles) and
`Step 2` (primes tests) both work. Workaround: Do While loop with manual increment.

## F-020 | 2026-08-16 | quirk | primes
`For p = 2 To even / 2` fails: "the trait `Step` is not implemented for `f64`" —
`/` is float division, so the bound is f64 and Rust can't iterate it. Notably,
`Int(even / 2)` does NOT fix it — Int() returns Double (faithful to VB6). The guide's
"just store the result in a Long (it truncates)" is the working path:
`Dim half As Long = even / 2` then `For p = 2 To half`. Maybe the For lowering could
insert the same narrowing cast an assignment gets.

## F-021 | 2026-08-16 | positive | primes
`Public Sub` works (with a friendly ⚠ that it's just a Function with no return).
ByRef out-parameters — the classic VB multi-return idiom — work across modules and
from Test blocks. Bare-condition `Assert Primes.IsPrime(2)` and `Assert Not ...`
work. All numerics verified: 78,498 primes ≤ 10⁶; Goldbach splits correct;
Collatz(27)=111; champion under 1000 is 871 with 178 steps.

## F-022 | 2026-08-16 | ergonomic | lissajous
`Atn` is missing (VB6: `4 * Atn(1)` is THE way to get π). `Sin`, `Cos`, `Sqr`,
`Chr(27)`, `Sleep` all verified working. Adding to the missing-builtins list from
F-008/F-011: `Atn` (and presumably `Tan`, `Exp`, `Log` — untested), `Space`,
`Split`, `Join`. Workaround used: pi as a literal constant.

## F-023 | 2026-08-16 | positive | mandel/chaos/lissajous
The maths/fractal batch was nearly frictionless: mandel (Mandelbrot + Julia, escape
time), chaos (bifurcation diagram + Hénon attractor, density grids), pascal
(Sierpiński), lissajous (120-frame ANSI animation with Chr(27) cursor home) — four
programs, only pre-known traps hit. Double maths, Vec<Boolean>/Vec<Long> grids with
index assignment, `Sleep`, and big single-string frame builds all held up. Terminal
animation via one Debug.Print per frame + ESC[H is smooth.

## F-024 | 2026-08-16 | ergonomic | cellab
`Dim slash As Long = InStr(code, "/")` fails with "expected i64, found
Option<i64>". The teach line for InStr (returns Option, handle Some/None) printed
earlier in the same compile, but the error itself doesn't repeat the guidance.
When the teach line exists, echoing it (or part of it) into the error would help.

## F-025 | 2026-08-16 | bug | cellab
`InStr("abc", "c")` returns **2** — a ZERO-based position, inside the Option.
VB6's InStr is 1-based, and everyone's muscle memory (and every ported snippet)
assumes that. This one won't even fail to compile: `Mid(s, InStr(s, x), 1)` will
silently return the character BEFORE the match. Char-based (not byte — good:
InStr("café", "é") = 3), and misses return None (at least that's surfaced).
Suggested fix: add 1 so the position matches VB and the 1-based Mid/Len world
it lives in; the teach line should spell out the base either way.

## F-026 | 2026-08-16 | positive | cellab/boids
cellab: cross-module `Public Type Rule` used bare in main.vbr, rule-string parsing,
toroidal stepping through a ByRef Vec, three automata (Life, Day&Night, Maze) all
showed their known behaviour. boids: O(n²) flocking over Vec<Boid> with per-boid
rebuild-and-assign, `Abs`, `Sqr`, Type literals with computed Double fields, and a
Vec<String> char grid — compiled first try and the flock visibly aligns by ~frame
150. Negative For bounds (`For dy = -1 To 1`) work.

## F-027 | 2026-08-16 | bug | amortize
`(1 + r) ^ n` with Double base and Long exponent fails: "expected f64, found i64" —
`^` doesn't reconcile mixed numeric types (it appears to pick integer pow from the
exponent). Workaround: `Dim nd As Double = n` then `(1 + r) ^ nd`. VB's `^` happily
mixed Integer/Double; suggest inserting the cast from the declared result type.

## F-028 | 2026-08-16 | positive | amortize
First GUI form worked first try: Window/State/View/Event with TextInput (+ On Submit),
Slider (Integer state), Button, Text, Rule, Scrollable + Table of a Vec<Type>, a
cross-module `Loan.Amortize` call with three chained Handle blocks inside one Event,
and a bare `Dim rows As Vec<ScheduleRow>` state field. Loan maths verified by 7 tests
(monthly 599.55 for 100k@6%/30y; balance lands exactly on 0.00; Money() formatting
incl. negatives). `vbr test` required main.vbr to exist even for module-only tests.
Window launched and stayed alive (smoke test only — couldn't drive the widgets
headlessly). Iced build is cached after the first compile, as promised.

## F-029 | 2026-08-16 | positive | juliagpu
First GPU sketch compiled and ran first try: a `Gpu Draw` kernel with its own
escape loop, Sin/Cos palette (lowered to WGSL fine), `Every 16 Tick` timer, a
`t` uniform advanced in the Tick event, and CPU `Text` overlay on top. Window
opened on WSLg at 640×480 with the timer running and stayed alive (wgpu backend;
NOTES.md's tiny-skia size deaths did not recur). Operational note for the log
writer, not the language: `vbr runproject` from a short-lived wrapper shell dies
with the wrapper — long-lived GUI/sketch runs need proper backgrounding.

## F-030 | 2026-08-16 | quirk | greyscott
`u` (and presumably `v`) can't be used as variable names inside `Gpu Draw` — they
are reserved kernel names. The error is clean and actionable ("`u` is a Gpu Draw
name. Pick another."), so it's a well-handled quirk; worth listing the reserved set
(`u`, `v`, `width`, `height`, `mouse_x`, `mouse_y`, `frame`, `t`?) somewhere in the
Sketch docs so kernel writers don't discover it one rename at a time.

## F-031 | 2026-08-16 | bug | greyscott
Inside a `Gpu Draw` kernel, the ordinary pattern `Dim dx As Long` followed by
`For dx = -1 To 1` generates WGSL with the variable declared TWICE
(`var dx = 0.0;` from the Dim, `var dx = -1.0;` from the loop), which naga rejects
at runtime: "redefinition of `dx`" → wgpu panic, window dies. The identical pattern
is legal (and silently deduplicated) in normal Bust → Rust — see F-002 and cellab's
main loops. So the rule differs per surface, invisibly. Workaround: drop the Dim in
kernels. The failure is also late (runtime shader compile) — this one could be caught
at transpile time alongside the reserved-name check.

## F-032 | 2026-08-16 | positive | greyscott
After the two fixes, the riskiest project works: a full Gray–Scott reaction-diffusion
kernel — `Sample(frame, x, y).r / .g` channel read-back (channel access on Sample is
NOT documented anywhere I could find; discovered by trying it — worth documenting),
a 3×3 neighbourhood of frame samples per pixel, clamping, and mouse seeding — runs
live on the GPU with the frame-feedback loop closed. State-carrying simulations in
pure VBR kernels are feasible.

---

# Round 2 — regression sweep (2026-08-16, after commits `Rnd`, `Bugs`, `Missing functions`)

Binary rebuilt same day; re-tested the round-1 findings:

- **FIXED — F-013** unary minus on Double (`Dim neg As Double = -d` now compiles).
- **NEW — `Str(x)`** works for Double and Long (`Str(3.5)` → "3.5").
- Still open: **F-006** (`Len("café")` = 5, bytes), **F-025** (`InStr` zero-based Option),
  **F-018** (uninitialised Dim → ByRef), **F-019** (variable `Step` usize mismatch),
  **F-020** (f64 `For` bound), **F-027** (`Double ^ Long`), **F-005** (InputBox EOF),
  **F-015** (py/C targets still emit undefined `mid`), and missing builtins
  Split/Join/Space/Atn/Format.
- Improved: the `Format` refusal now suggests `Str(x)` / `format!("{:.2}", x)` /
  num-format crate; `Rnd` message unchanged (good). The "cannot find function"
  hint about lowercase-in-Rust-blocks still fires for genuinely missing builtins
  (F-008 side note).

---

## F-033 | 2026-08-16 | ergonomic | controlroom
Widget grammar takes a little discovery, but the transpiler's error messages
dictate the exact fix each time — excellent DX:
- `ProgressBar 0..=100, field` (the range is mandatory; bare `ProgressBar field`
  is refused with the correct shape shown).
- `Radio "label", field, value` — field BEFORE value.
- `Toggler "label", field` — the label is mandatory.
Suggest putting those exact shapes in a cheat-sheet section; everyone will hit all
three in their first Window.

## F-034 | 2026-08-16 | quirk | recipes
Numeric narrowing (i64 → i32) is never inserted on reads: `servings = book[i].servings`
(Long struct field → Integer slider state) and `Dim y As Integer = j.Get_Int("k")`
both fail. Widening (i64 → f64) also fails when the RHS flows through a Handle/`?`
("operator `?` has incompatible types"), though it works from a plain struct read.
Working idiom: hop through Double — `Dim d As Double = book[i].servings` then
`servings = d` (both steps cast fine, including plain assignments). Related: Slider
rejects Long-bound fields outright (Integer/Single/Double only) — the message is
clear. Since sliders are Integer and most data is Long, the hop will be needed in
every form app; worth either auto-casting reads or an explicit `CInt()`-style builtin.

## F-035 | 2026-08-16 | ergonomic | recipes
Typed `Json.Save(path, value)` / `Json.Load(path)` (used by the vault I first read)
no longer exist — the stdlib Json is the dynamic object/array API (Parse, Get_*,
Set_*, Push, To_Pretty). Persistence now takes ~30 lines of get/set per Type. Also
`NewText()` is gone; TextArea state initialises from a string literal
(`Dim method As TextArea = ""`). Both are fine once known — just document the
current pattern, ideally with a "saving your own Type" recipe in stdlib_spec.

## F-036 | 2026-08-16 | quirk | recipes
Ownership seams surface inside Event handlers as raw rustc errors (E0382/E0507),
not translated VBR-level ones: (1) a String event parameter is moved by the first
`field = value` assignment — later reads of the parameter fail; (2) building a
Type literal from String state fields moves them — needs `.Clone()`; (3) assigning
a Type into a Vec in a loop AND pushing it afterwards needs `.Clone()` on the loop
assignment. All fixable with `.Clone()` and the compiler even suggests it, but a
teach line ("Bust strings move on assignment — add .Clone() to keep a copy") would
turn three confusing rustc walls into one lesson. Note Reads from Vec/book structs
are auto-cloned — it's only the write paths above that bite.

## F-037 | 2026-08-16 | positive | controlroom/recipes
Two substantial form apps landed: controlroom (Tabs/Frames/ProgressBars/Slider/
Radio/Toggler/Table/Tooltip/Markdown, Dracula) compiled and launched; recipes
(List/TextInput/Chooser/Checkbox/Slider/TextArea/Tooltip + JSON file persistence
through the dynamic Json API, state initialised from functions — `Dim book As
Vec<Recipe> = LoadBook()` works, CatppuccinMocha theme accepted) compiled and
launched. Slider-on-Integer, Table-of-Type with On Select, function-initialised
state, and cross-event field updates all behaved.

## F-038 | 2026-08-16 | bug | wavebench
An `Event` can't call another `Event` by name. `RefreshWaves` inside a sibling
event lowered to a bare `refreshwaves;` statement — "cannot find value `refreshwaves`
in this scope" (×6). Events aren't callable functions; they're message handlers. The
transpiler should refuse this at parse time ("Events can't call events — factor the
shared work into a `Sub` and pass the state fields ByRef"). Workaround used: a module
`Sub RefreshWaves(ByRef sum…, ByVal a1…)` taking every touched field. Related and
fine: `For Each` over a Vec<Type> in a Canvas `Draw`, indexed `pts[i-1]` reads in
Draw, and Double→Integer narrowing via a plain `Dim` (the only narrowing that casts).

## F-039 | 2026-08-16 | positive | fourier/newton/wireworld/ecology
The whole terminal maths/life batch — fourier epicycles, the Newton z³−1 fractal,
wireworld with two clock loops feeding an OR junction, and a fox/rabbit/grass
predator-prey sim with live sparklines — compiled with ZERO language errors as
single-file `vbr run` programs. All iteration was on the simulation logic, not the
language. Highlights that just worked: nested `For` with negative bounds in kernels
and terminal code, `Exit For`/`Exit Do` from deep conditionals, Vec trimming via
rebuild-copy, `Sub` with ByRef seed threading, multi-pass grid updates through
ByRef Vecs, and Long→Double→narrowing via `Dim`. This is the strongest signal so
far that the core-language surface is stable for computation-heavy code.

## F-040 | 2026-08-16 | quirk | raymarch
`Dim step As Long` / `For step = …` is refused — `step` is a keyword, and the
message is friendly ("Pick another (for example … `step_`)"). Same family as the
`next` refusal (F-017) and the kernel reserved names (F-030): consistent,
well-messaged. Note the follow-on parse confusion: because the `For step` line
failed, the matching `Next` later reported "Expected End to close Gpu Draw" — one
bad name produced two errors. Fine, just noting cascades happen. Also confirmed in
this batch: `Atan2` and `Sqr` exist in Gpu Draw kernels (they lowered to WGSL).

## F-041 | 2026-08-16 | quirk | tunnelgpu/raymarch/orbitsgpu
Environmental, not language: on this WSLg box, TWO concurrent wgpu Sketch windows
kill each other (both processes die with no panic output — logs stop at
"→ cargo run"). Verified three times: all three sketches run perfectly one at a
time; the moment a second GPU window opens, both die. GUI Windows (Iced widget
apps) didn't show this when two ran together earlier, so it looks like GPU-surface
contention in the WSL compositor rather than an Iced problem. Practical note for
demoing: show the GPU sketches one window at a time.

## F-042 | 2026-08-16 | positive | tunnelgpu/raymarch/orbitsgpu
All three GPU showpieces compiled and run: a demoscene tunnel (Atan2 + Sqr + Noise
in the kernel — all lowered to WGSL), a raymarched sphere-on-checkerboard scene
(48-step SDF march, finite-difference normals, per-pixel lighting in pure kernel
code), and three Lissajous particles with decaying trails (Copy frame +
Sample(frame,…).r/.g/.b fade loop). Reserved keyword handling (`step` refused with
a suggestion), kernel locals, nested For + Exit For inside kernels, and Double
colour maths all behaved. The kernel surface comfortably handles real graphics
programs now.

## F-003 | 2026-08-16 | ergonomic | bottles
The ℹ teaching lines (name lowercasing, `as` casts, Step -1) print on *every*
compile/run/emit. Each is individually useful once, but in a tight edit-run loop they
become noise that buries the program's own output. Maybe first-run-only per project,
or a `--quiet-teaching` flag. (Also: one line renders as "ℹ ℹ" — doubled prefix.)
