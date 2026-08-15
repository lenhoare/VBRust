# Notes — building the A-series example projects

Experience log for the Bust example-project exercise. Each entry: what was
attempted, what the transpiler accepted/rejected, quirks found, and workarounds
used. Bugs worth fixing in the transpiler are flagged **[BUG]**; features that
feel missing are flagged **[MISSING]**. (The older gap log from the
idea-engine session lives in `vbr_gaps.md`; anything new worth the maintainers'
attention should also be copied there.)

---

## A1 — Temperature Converter (2026-08-06)

Project: `projects/A1_temperature_converter/`. Pure core language, no stdlib.
Went through transpile → build → run → test cleanly after two workarounds.

### What worked well

- Multi-module project layout, qualified calls (`Temps.CtoF`), `Public
  Function`, `Public Const` read cross-module, `If/ElseIf/Else`, `For …
  Step`, `&` concatenation, `Test`/`Assert` — all smooth on the first try.
- `Round()` builtin available and deterministic.
- `Dim d As Double = c` (widen a `Long`/i32 into `Double`) emits the automatic
  `as f64` cast with an informative � ℹ note — nice teaching behaviour.
- `vbr test` reports `��✓` by description and exits 0; `vbr runproject` stdout
  matches `expected_output.txt` byte-for-byte.

### Quirk 1 — [BUG] Qualified calls don't adapt integer literals to `Double` params

```vb
' temps.vbr
Public Function CtoF(ByVal c As Double) As Double
    Return c * 9 / 5 + 32
End Function

' same file — works
Debug.Print CtoF(100)          ' literal adapts: ctof(100.0)

' main.vbr — FAILS to compile
Debug.Print Temps.CtoF(100)    ' � ✘ expected `f64`, found integer
```

The spec (`projects_and_run_spec.md`) promises a qualified call gets "the same
argument treatment as a local call", but the integer-literal → float adaptation
isn't applied when the callee is in another module. Float literals (`100.0`)
and `Double` variables pass through fine; only bare integer literals fail.
This probably never surfaced because every existing cross-module example
(`receipt`, `life`) passes `Long` args to `Long` params.

Workaround: write float literals explicitly (`100.0`), or widen through a
local `Dim d As Double = expr`.

### Quirk 2 — [BUG] `For`-loop counters are never adapted to `Double` params

```vb
For k = -20 To 40 Step 10
    total = total + CtoF(k)    ' � ✘ expected `f64`, found integer — even locally
Next
```

The loop counter is i32 and isn't widened when the target parameter is `f64`.
Workaround: `Dim d As Double = k` inside the loop before the call.

### Observation — float Display

Rust's `{}` on `f64` prints the shortest round-trip representation, so
`-273.15 * 9 / 5 + 32` prints as `-459.66999999999996` (the literal −273.15
isn't exact in binary). Deterministic, but ugly for teaching examples. `Round()`
cleans it up at the cost of whole-degree precision. Worth a note in the
README-style docs: "don't be surprised by float drift in output".

### Testing discipline learned

Float equality in `Assert` is only safe for exactly-representable values
(0.0, 32.0, 100.0, 212.0, 273.15). For anything derived, use a range bound
(`< -450.0`) instead of `=`. Keeps tests deterministic without flaky ulp
comparisons.

---

## A2 — Simple Enum Demo (2026-08-06)

Project: `projects/A2_simple_enum/`. Pure core language, no stdlib.
Went through transpile → build → run → test cleanly after one workaround.

### What worked well

- Multi-module project layout, qualified calls (`Traffic.DurationSec`),
  `Public Function`, `Public Const` read cross-module, `For Each` loop,
  `Match` over an enum (exhaustive, no wildcard needed because all variants
  covered), `&` concatenation, `Test`/`Assert` — all smooth on the first try
  after making the enum `Public`.
- `Round()` builtin available and deterministic.
- `vbr test` reports `��✓` by description and exits 0; `vbr runproject` stdout
  matches `expected_output.txt` byte-for-byte.

### Quirk — [MISSING] Enum does not derive `Display`

```vb
' traffic.vbr
Public Enum TrafficLight
    Red
    Yellow
    Green
End Enum

' main.vbr — FAILS to compile
Debug.Print light  ' � ✘ TrafficLight doesn't implement std::fmt::Display
```

Bust currently does not derive the `Display` trait for `Enum`, so `Debug.Print`
(or any formatted output) of an enum value fails to compile. The workaround is
to provide a `Public Function Name(light As TrafficLight) As String` that
uses a `Match` to return the string literal for each variant. This is exactly
what the VB6 programmer would do anyway, so it’s not a burden, but worth noting
for completeness.

Workaround: add a `Public Function Name` that matches the enum and returns the
appropriate string.

### Literal‑adaptation quirks carried over from A1

The same two A1 quirks apply when passing literals to `Long`/`Double` params in
qualified calls:

1. Qualified calls don't yet adapt integer literals to `Double`/`Long`
   parameters — use `.0` literals or a local widened variable.
2. `For`-loop counters are never adapted to `Double`/`Long` parameters —
   widen through a local `Dim` before passing.

Both workarounds are documented in the A1 entry.

---

## A3 — Bank Account (Type + Result) (2026-08-07)

Project: `projects/A3_bank_account/`. Pure core language, no stdlib.
Demonstrates `Public Type` with methods (`Function Account.Deposit`),
`Me` receiver, inferred `&mut self`, `Result<Long>` returns with
`Ok`/`Err`, `Match` over a Result, `.Unwrap()`, and a `ByRef` helper.

### What worked well

- `Type`/`End Type` with `Public` fields, literal constructor
  `Account { owner: "Ada", balance: 100 }`, method calls on a value
  (`acc.Deposit(50)`), `Me.Balance` mutation → `&mut self` inferred — all
  smooth on the first try in the *program* (main.vbr) build.
- `Result<Long>` + `Return Err("...")` / `Return Ok(...)` works exactly like
  the `result.vbr` example; `Match` with `Ok(v)`/`Err(m)` arms is exhaustive.
- `Return` *inside* a `Match` arm is legal — used by the `TryWithdraw` helper
  to flatten a Result to Boolean. Nice.
- `ByRef acc As Account` renders as `&mut Account` and writes flow back.

### Quirk 3 — [FIXED] Match pattern bindings are raw Rust — lowercase only

```vb
Match acc.Deposit(50)
    Ok(newBalance) => Debug.Print "..." & newBalance   ' � ✘ cannot find value `newbalance`
End Match
```

`Match` patterns are real Rust patterns, so a binding must be written in its
Rust spelling (lowercase). The transpiler even points this out in its hint
("use the lowercase spelling"). Fix: write `Ok(newbalance)`.

### Quirk 4 — [FIXED by Len, commit `2f18230`] Test bodies don't mark locals mut

`vbr test` failed on every `Test` in A3 with:

```
� ✘ cannot borrow `acc` as mutable, as it is not declared as mutable
```

The generated `bank_test.rs` had `let acc` while `main.rs` correctly had
`let mut acc`. Root cause: `collect_mutated` (transpiler.rs) — the pass that
decides which locals need `let mut` — had no case for `Assert`-statement
expressions, so a local passed to a user-defined `&mut self` method
(`acc.Deposit(...)`) inside a `Test` was never marked. The same call in
`main.vbr`'s ordinary statement position *was* marked, which is why the
program built but the tests didn't.

Len fixed it in the transpiler (commit `2f18230 test mut bug`). After
rebuilding the binary (`cargo build`) A3's tests pass 4/4.

Lesson for future test files: mutating methods on a local are only safe in
tests since this fix; before it, the workaround would have been to give the
test something the mutability pass *does* see (e.g. a plain `acc.Balance = x`
assignment anywhere in the test body, since `Stmt::Assign` roots are always
marked), or to avoid mutating methods and test through pure functions.

---

## 1 — Bagels (2026-08-07)

Project: `projects/1_bagels/`. Pure core language, no stdlib. First project
from the 81-list. Deterministic version: fixed secret `"123"`, fixed guesses.
7/7 tests pass; `runproject` output matches `expected_output.txt`
byte-for-byte.

### Quirk 5 — [BUG] Bust arrays/Vec index with brackets, not parens

```vb
Dim used As Vec<Boolean> = [False, False, False]
used(0) = True    ' � ✘ 'used' is an array — index it Rust-style with used[i]
```

The transpiler rejects `used(i)` with a helpful hint to use `used[i]` (or
`used.get(i)` for a safe Option). VB6 muscle memory writes parens; Bust wants
Rust brackets everywhere, including for `Vec`. The vb6_to_vbr_guide says this
("index with brackets, not scores(i)") — it's the single most common
transpiler error so far. Fix: write `used[i]`.

### Quirk 6 — [BUG] `ToCharArray()` doesn't exist (methods pass through to Rust)

```vb
Dim chars As Vec<Char> = guess.ToCharArray()    ' � ✘ no method named `tochararray`
```

Method calls pass straight through to Rust (lowercased), so a .NET/VB method
that Rust doesn't have simply isn't there. `&str` has no `to_char_array`.
Workaround: don't build char arrays — use `Mid(s, i+1, 1)` for per-character
access (the builtin is 1-indexed and character-counted, exactly VB6
semantics), or drop into a `Rust … End Rust` block. Bagels uses `Mid`.

### Quirk 7 — `For Each` borrows elements — clone before keeping them

```vb
For Each part In result
    clue = part        ' � ✘ cannot move out of `*part` which is behind a shared reference
Next
```

`For Each` borrows each element, so assigning it onward fails for `String`.
Workaround: `clue = part.Clone()`. (The guide's note "reading a String out
of a list clones it for you" applies to indexed reads like `list[i]`, not to
`For Each` bindings.)

### Quirk 8 — [DOC GAP] Loop variable is out of scope after `Next`

The guide says the `For` counter is "gone after Next" — the same is true of
the `For Each` binding. Using `guess` after the loop (to test whether the
player won) is a compile error (`used binding isn't initialized`). Workaround:
carry a `Dim solved As Boolean` flag out of the loop, as `main.vbr` does.

### Quirk 9 — `.Length` on a `Vec` → use `.Len()`

`guesses.Length` fails (`no field length on type Vec<String>`); `.Len()` (or
`.Count()`) is the Bust spelling.

### Test-authoring mistake worth recording

My first draft asserted `GetClue("123", "132") = "Fermi Pico"` — wrong. The
clue for a correct-but-misplaced digit is per *digit*, so '2' and '3' are
both Pico: the right answer is "Fermi Pico Pico". When porting a game,
double-check the scoring rule against the book's own examples before
writing the expected string.

---

## 12 — Collatz Sequence (2026-08-07)

Project: `projects/12_collatz/`. 5/5 tests pass; output matches. Nothing new
learned — the language handled `Do While`, `Mod`, integer division and
`Vec<Long>` returns first try. `Assert vec = [...]` list equality works.

---

## 49 — Multiplication Table (2026-08-07)

Project: `projects/49_multiplication_table/`. 5/5 tests pass; output matches.

### Quirk 10 — Test descriptions become Rust fn names: no leading digit

```vb
Test "12 times 12 row ends with 144"    ' � ✘ expected identifier, found `12_times_12_row_ends_with_144`
```

The description is lowercased and used as the generated `#[test] fn` name, so
it must be a valid Rust identifier — no leading digits or punctuation.
Workaround: start the description with a word ("row twelve ends with 144").

### Formatting note

Cells are right-justified to 4 chars and joined with a single space, so the
visible gap between numbers is 1 space + padding; test expectations must
count that exactly (e.g. row 12's last cells are ` 108` / ` 120`).

---

## 26 — Fibonacci (2026-08-07)

Project: `projects/26_fibonacci/`. 5/5 tests pass; output matches. Nothing
new — straightforward `For` loop building a `Vec<Long>`. `Fib(n)` accessor
uses `xs[n]` bracket indexing (Quirk 5) with no issue.

---

## 6 — Caesar Cipher (2026-08-07)

Project: `projects/6_caesar_cipher/`. 8/8 tests pass; output matches.

### Quirk 11 — [MISSING] No `Asc()` builtin (and no `IIf()`)

There is `Chr(n)` (→ one-char string) but **no `Asc(s)`** (the inverse) and
no `IIf(cond, a, b)`. Both are classic VB6 tools. Workarounds:
- For char-code arithmetic, build an alphabet string and look letters up with
  `Mid`/`FindIn` instead of codes (what caesar.vbr does), or use a
  `Rust … End Rust` block.
- Replace `IIf` with an ordinary `If/Else`.

### Quirk 12 — [BUG?] `InStr` returns `usize` — can't `Return p` from a Match arm

```vb
Match InStr(alphabet, ch)     ' � ✘ mismatched types: expected `i64`, found `usize`
    Some(p) => Return p
    None => Return 0
End Match
```

`InStr` lowers to Rust `.find()` (an `Option<usize>`), and the Match-arm
`Return p` doesn't auto-widen `usize` → `i64`. Workaround: avoid the Option
entirely — a `Do While` + `Mid` scan returns a plain `Long` (what `FindIn`
does). Worth checking whether the literal-adaptation work should also cover
`usize` → `Long` here.

---

## 61 — ROT13 (2026-08-07)

Project: `projects/61_rot13/`. 6/6 tests pass; output matches. Reused the
Caesar pattern (`FindIn` + `Mid`) — nothing new; `Rot13(Rot13(x)) = x`
identity test is a nice deterministic property check.

---

## 24 — Factor Finder (2026-08-07)

Project: `projects/24_factor_finder/`. 5/5 tests pass; output matches.

### What worked well (all first try)

- `Public Sub SortAsc(ByRef xs As Vec<Long>)` mutating a caller's `Vec` in
  place — the `&mut` borrow is inserted automatically, and the caller's
  variable is marked `let mut` by the ByRef path. No issues.
- Insertion sort with bracket indexing and swaps; `xs.Len() - 1` tail index.
- Square-root-bound `Do While i * i <= n` with `Mod` and integer division.

---

## 56 — Prime Numbers (2026-08-07)

Project: `projects/56_prime_numbers/`. 5/5 tests pass; output matches.
Nothing new — same idioms as Factor Finder. `Do While out.Len() < count`
grows a Vec until it reaches a size, which works fine.

---

## 50 — Ninety-Nine Bottles (2026-08-07)

Project: `projects/50_ninety_nine_bottles/`. 5/5 tests pass; output matches.

### What worked

- `For n = 5 To 0 Step -1` — counting-down loop emits a reversed range and
  works first try (the transpiler prints a note about it).
- `If / ElseIf / Else` on the count; `Vec<String>` of lines; `CStr(n)`.

---

## 52 — Numeral Systems Counters (2026-08-07)

Project: `projects/52_numeral_systems/`. 5/5 tests pass; output matches.

### Quirk 13 — Count whitespace in padded-string expectations carefully

Two test failures were my own expectation errors, not transpiler bugs: a
12-wide padded field joined after a separator shows *width − len(s) + 1*
visible spaces, and I miscounted twice (for `101` and `11111111`). When
asserting padded output, copy the string from a first run rather than
hand-counting spaces.

### What worked

- Prepending digits: `out = CStr(digit) & out` builds the base string
  correctly (left-to-right).
- `Mid(digits, d + 1, 1)` hex lookup — Mid's 1-based indexing is natural
  here (0-based digit index `d` → `d + 1`).

---

## 16 — Diamonds (2026-08-07)

Project: `projects/16_diamonds/`. 5/5 tests pass; output matches. Both my
test failures were expectation errors: an outline diamond of size n has
2n−1 rows (the widest row appears once — I wrongly expected 2n), and the
widest row has 2×(n−1) inner spaces (I undercounted). The generated art is
correct per the book. `For ... Step -1` for the mirrored bottom half works.

---

## 40 — Leetspeak (2026-08-07)

Project: `projects/40_leetspeak/`. 5/5 tests pass; output matches.

### Quirk 14 — [BUG?] `Match` on a `String` against `&str` literal patterns fails

```vb
Match lower          ' lower As String = LCase(ch)
    "a" => Return "4"    ' � ✘ mismatched types: expected `String`, found `&str`
    ...
End Match
```

Matching a `String` scrutinee against `"..."` literal patterns doesn't
compile — the literals are `&str` and no coercion happens in the pattern
position. Workaround: use an `If / ElseIf` chain with `=` — `String = "a"`
comparisons are fine (Rust's `PartialEq<&str> for String`). Worth checking
whether Bust should lower String match scrutinees via `.as_str()`.

### Mapping note

Classic leetspeak maps both `i` and `l` to `1`, so "hello" → "h3110" (not
"h3ll0"). My first test expectations were wrong; fixed after the first run.

---

## 3 — Bitmap Message (2026-08-07)

Project: `projects/3_bitmap_message/`. 5/5 tests pass; output matches.

### Quirk 15 — [MISSING?] Multiline `[ ... ]` list literals not supported

```vb
Dim bmp As Vec<String> = [
    "     xxxxx     ",
    ...
]
```

Splitting a list literal across lines fails ("Expected an expression, found
Newline"). The literal must be on one line, or the rows built with `Push`
one statement at a time (what main.vbr does). Worth confirming this is
intended (line noise in single-line literals is real for bitmaps).

### Quirk 16 — [BUG] A local variable with the same name as a module is shadowed

```vb
' main.vbr, with sibling module bitmap.vbr (module name Bitmap)
Dim bitmap As Vec<String>
bitmap.Push("...")    ' � ✘ cannot find function `push` in module `crate::bitmap`
```

`bitmap.Push(...)` is parsed as a *qualified module call* (`Bitmap.Push`)
because the variable name collides with the module name. Workaround: name
the variable differently (`bmp`). The same trap presumably applies to any
local matching a sibling module name — worth a resolver error instead of
this confusing rustc message.

---

## 77 — Tower of Hanoi (2026-08-07)

Project: `projects/77_tower_of_hanoi/`. 5/5 tests pass; output matches.

### Quirk 17 — [MISSING] `To` is a reserved word — cannot name a parameter `to`

```vb
Public Sub Move(ByVal n As Long, ByVal from As Long, ByVal to As Long, ...)
'                                                                 ^ � ✘ Expected a name for the parameter
```

`To` is reserved (For loops). Rename to `dst`.

### Quirk 18 — [BUG] `Move` lowers to the Rust keyword `move`

```vb
Public Sub Move(...)          ' � ✘ expected identifier, found keyword `move`
    Move(n - 1, ...)          ' � ✘ expected one of `async`, `|`, or `||`, found `(`
```

The function name `Move` becomes `move` in Rust, which is a keyword — the
transpiler doesn't escape it. Workaround: rename to `MoveDisk`. (Worth a
reserved-name check like the type-name one in the gotchas section.)

### Recursion works

`Public Sub MoveDisk` recursing with a `ByRef Vec<String>` accumulator ran
fine — the generated Rust is ordinary fns. No issues.

### Test bug

I asserted `m.Len() = 3` for "1->3" — it's 4 chars. Parse validation by
position, or check `Len()` carefully, for arrow strings.

---

## 80 — Vigenère Cipher (2026-08-07)

Project: `projects/80_vigenere_cipher/`. 6/6 tests pass; output matches.

### Quirk 19 — [BUG?] `ByVal String` param accepts a variable, not a call expression

```vb
Dim k As Long = FindIn(upper, Mid(keyword, pos, 1))
'                             ^ � ✘ mismatched types: expected `&str`, found `String`
```

A `ByVal String` parameter renders as `&str`, and Bust auto-borrows a String
*variable* at the call site — but a call expression returning String isn't
borrowed, so it fails. Workaround: assign the expression to a local first
(`Dim keyChar As String = Mid(...)`), then pass the variable. Same shape as
Quirk 12 (InStr's usize) — call-expression arguments deserve the same
treatment as variables.

### Off-by-one trap — FindIn is 1-based, shifts must be 0-based

My first draft used `FindIn(upper, keyChar)` directly as the shift, so
"ATTACKATDAWN"+"LEMON" produced "MYGPQWFGSOIS" (everything +1). The key
letter's position is 1-based from FindIn; the shift must be `k - 1`. The
known-answer test caught it immediately — worth always including one.

---

## 64 — Seven-Segment Display (2026-08-07)

Project: `projects/64_seven_segment/`. 6/6 tests pass; output matches.

### Quirk 20 — Structs must be built complete — no empty declaration

```vb
Dim seg As Segment          ' � ✘ A struct must be fully initialised at creation
seg = Dash()                '     You cannot declare it empty and fill fields later
```

A `Dim x As SomeType` without an initializer is refused. Workaround: declare
with the constructor call (`Dim seg As Segment = Dash()`), branching to the
constructor at the point of use rather than filling a placeholder later.
This matches the guide ("built complete, all at once") but is worth knowing
before writing VB6-style declare-then-Set.

### Quirk 21 — [MISSING] Private Type used from a test file must be `Public`

```vb
Type Segment ... End Type      ' in segments.vbr
' segments.test.vbr:
Dim seg As Segment             ' � ✘ The type 'Segment' is Private to 'segments.vbr'
```

The transpiler correctly enforces visibility, but the error appears only
when the *test* file uses the type — `main.vbr` (in the same project) didn't
trigger it for this shape. Rule of thumb: if a test constructs the type,
mark it `Public Type`.

### Val → Long needs a local

`SegmentForDigit(Val(d))` fails ("expected `i64`, found `f64`") — Val is a
Double and the qualified call doesn't narrow. `Dim dv As Long = Val(d)`
inserts the `as` cast (with a note) and works.

### Joining rows

When asserting joined multi-digit rows, count the single space separator
exactly — my first expectations were wrong twice (off by one space per
digit gap).

---

## 7 — Caesar Hacker (2026-08-07)

Project: `projects/7_caesar_hacker/`. 5/5 tests pass; output matches.

### Quirk 22 — [DOC GAP] Project folders are self-contained — no cross-project modules

```vb
' projects/7_caesar_hacker/hacker.vbr
Caesar.Decrypt(secret, k)    ' � ✘ cannot find value `caesar` in this scope
```

Modules are shared within a project *folder*, not across projects — the
Caesar module from `projects/6_caesar_cipher/` is invisible here. I assumed
cross-project reuse; the fix was a local `Shift` copy (12 lines). Worth a
line in the instructions: "a project is a folder; reuse means copying."

### Quirk 23 — `Sub` is a reserved word too

`Contains(ByVal text As String, ByVal sub As String)` fails — `Sub` is a
keyword (like `To` in Quirk 17). Rename to `needle`.

### English-scoring note

The tiny word-list scorer is good enough: it picked key 23 for the classic
pangram and a real-vs-gibberish ordering holds. For harder ciphertext a
bigger word list or letter-frequency scoring would be needed — noted for
future cipher projects.

---

## 65 — Shining Carpet (2026-08-07)

Project: `projects/65_shining_carpet/`. 6/6 tests pass; output matches.

### What worked (all first try)

- The interlocking offset-tile approach: `group Mod 2` chooses a 3-column
  offset, `HexRow(phase)` picks one of six row shapes. Deterministic and
  exact.
- Nested `For` building `Vec<String>` rows; `Mod` phase cycling.

### Note

The book's original carpet uses a different seam treatment (rows of `=`
and `-` between columns); this version tiles hexagons with a half-hex
offset instead, which gives the same interlocking look with simpler
geometry. Fine as a deterministic regression example.

---

## 57 — Progress Bar (TUI) (2026-08-07)

Project: `projects/57_progress_bar/`. First **Screen** (ratatui TUI)
project. 5/5 logic tests pass; the TUI itself verified under tmux (gauge
renders, `q` quits).

### Shape of a TUI project (new pattern)

- Pure logic in `<subject>.vbr` — fully testable by `vbr test`, no UI.
- `main.vbr` holds the `Screen`: State/View/Events, `Gauge`, `Every` timer,
  `On Key "q" Quit`.
- **`expected_output.txt` is a tmux-captured frame**, not stdout: a Screen
  owns the terminal (it panics when piped — ratatui needs a real TTY) and
  `Debug.Print` is banned (use `Log`). Capture at 80x24 (`tmux
  new-session -x 80 -y 24`), strip trailing whitespace, and use a *stable*
  state (the animation's final frame) so the artifact is deterministic —
  an intermediate frame would be timing-dependent. Verified byte-identical
  across two runs.

### Quirk 24 — [BUG] Integer division before Double cast

```vb
Dim p As Double = done / total    ' done, total are Long
' Rust emits (done / total) as i64 division, THEN casts: 25/100 -> 0.0
```

The cast to `f64` happens after the division, so two `Long`s divide as
integers (25/100 = 0). Workaround: widen one operand first —
`Dim d As Double = done` then `Dim p As Double = d / total`. Same family as
Quirks 1–2 (no argument/literal adaptation to float) — worth a note in the
guide's "where VB silently converted numbers" section.

### TUI notes

- View expressions can't call cross-module functions (the resolver doesn't
  run in views) — compute values in events, store in state, bind widgets to
  state fields. The spec's life_screen note says the same.
- The Gauge widget labels itself with the field name (`percent`) — cosmetic.

---

## 31 — Guess the Number (TUI) (2026-08-07)

Project: `projects/31_guess_the_number/`. 5/5 logic tests pass; the TUI was
verified interactively under tmux (typed "50", Enter → "too low — 9 guesses
left", counter dropped to 9; `q` quits). First project with an `Input`.

### Quirk 25 — [DOC GAP] Events can't use a bare `Return`

```vb
Event TryGuess(text As String)
    If won Then
        Return              ' � ✘ `return;` in a function whose return type is not `()`
    End If
```

Screen events lower to `fn ... -> std::io::Result<()>`, so a bare `Return`
in an event is a type error. Workaround: restructure as If/ElseIf so the
event always falls through (what the game does). Worth a line in the TUI
spec's Events section.

### Quirk 26 — [CONFIRMED] View expressions can't call cross-module functions

`Text "Guesses left: " & Guess.Remaining(...)` in a View fails at rustc
level ("expected value, found module `guess`") — the view doesn't run the
resolver, exactly as the spec's life_screen note warns. Workaround: compute
into a state field in the event, bind the widget to the field.

### What worked

- `Input entry` + `On Submit TryGuess` — typed text arrives as the event's
  `text As String` parameter; Backspace/Enter behave.
- `Val(text)` parsing of the input, deterministic `SecretFor(seed)`.
- Tab focus is trivial here (single input), but the wiring is in place.

---

## 5 — Bouncing DVD Logo (TUI) (2026-08-07)

Project: `projects/5_bouncing_dvd/`. 6/6 logic tests pass; the TUI animates
under tmux (position moved 16,4 → 2,10 between captures; corner counter
incremented; `q` quits).

### Quirk 27 — [MISSING] `Step` is a reserved word for function names too

```vb
Public Function Step(...)    ' � ✘ Expected a name for the function, found Step
```

Like `To` (Quirk 17) and `Sub` (Quirk 23), `Step` is reserved (For loops).
Renamed to `Advance`.

### Animation determinism note

For a *pure animation* there is no stable final frame to capture: the
expected-output artifact is a frame at a fixed capture delay (~8s), which
was byte-identical across three runs but is inherently timing-dependent.
The real deterministic regression artifact is the logic test suite. For the
two prior TUI projects the "stable completed state" trick worked (Progress
Bar) — an animation has no such state.

### What worked

- `Every 150 Tick` drives the loop; the event mutates State (`logo =
  Bdvd.Advance(logo, w, h)`).
- A `Public Type` in State by bare name — no qualification on the type.

---

## 76 — Tic-Tac-Toe (TUI) (2026-08-07)

Project: `projects/76_tic_tac_toe/`. 11/11 logic tests pass; the TUI was
verified interactively under tmux (moves 1,5,2,3,6 → board `X X O | - O X
| - - -` with correct alternation; `q` quits).

### Quirk 28 — [BUG] `For Each` over a nested `Vec<Vec<Long>>` mis-compiles

```vb
Dim lines As Vec<Vec<Long>> = [[0,1,2], ...]
For Each line In lines
    Dim a As Long = line[0]    ' � ✘ type `i64` cannot be dereferenced
Next
```

Iterating a `Vec<Vec<Long>>` with `For Each` and indexing the inner Vec
fails at rustc ("type `i64` cannot be dereferenced") — the transpiler emits
a bad deref for the inner indexing. Workaround: flatten to explicit checks
via a helper (`LineWins(board, a, b, c)` called eight times), or index the
outer Vec with a numeric `For` instead of `For Each`.

### Quirk 29 — [CONFIRMED] Events can't call other events

`Event Move1` calling `TryMove(0)` (a sibling event) fails: "cannot find
function `trymove` in this scope". Events lower to handler methods, not
callable functions. Workaround: put the shared logic in a `Public Sub` in
the logic module and call it qualified from each event (the life_screen
`Life.SetCell(grid, …)` pattern) — `Ttt.TryMove(board, moves, over,
message, cell)`.

### Quirk 30 — [BUG] Returning a Vec element needs `.Clone()`

```vb
Public Function Winner(...) As String
    Return board[0]    ' � ✘ cannot move out of index of `Vec<String>`
```

Indexing a `ByVal Vec<String>` (a `&Vec` borrow) and returning the element
fails — the element must be cloned: `Return board[0].Clone()`.

### What worked

- `Public Sub TryMove(ByRef board As Vec<String>, ByRef moves As Long,
  ByRef over As Boolean, ByRef message As String, ByVal cell As Long)`
  mutating four state fields in place from events — the ByRef path handles
  `Vec` and scalars alike.
- Nine near-identical key events are verbose but the pattern is dead
  simple; a `Match`-key or per-cell loop would need resolver help.

---

## 14 — Countdown (TUI) (2026-08-07)

Project: `projects/14_countdown/`. 6/6 logic tests pass; the TUI was
verified interactively under tmux (Space starts; 3s later the seven-seg
display shows 1:27).

### Notes

- Reused the 7-seg shapes from project 64 (same `Seg` type, same `Match`).
- Quirk 30 (`.Clone()` on Vec index reads) bit again — `segRow1 = rows[0]`
  fails; needs `rows[0].Clone()`. This is the second project to hit it, so
  it's a reliable rule: **any `Vec<String>` element read must be cloned.**
- My test expectations for the joined segment rows were wrong twice —
  counting `1:30`'s four glyphs × 3 chars + 3 separators = 15 chars is
  error-prone; copy from the first run.

---

## 20 — Digital Stream (TUI) (2026-08-07)

Project: `projects/20_digital_stream/`. 6/6 logic tests pass; the TUI
animates under tmux (frames differ between captures; full 14-row rain
renders after adding `Length 14` to the frame Text).

### Quirk 31 — [BUG] `vbCrLf` in a `&` chain produces a bare CR in format!

```vb
out = out & vbCrLf & line    ' � ✘ bare CR not allowed in string, use `\r` instead
```

`vbCrLf` lowers to a literal `\r\n` inside the `format!` string, and Rust
rejects the raw CR. Workaround: use `vbLf` (lowers to `\n`), which is what
a TUI wants anyway. (Quirk: the constant exists but is only safe outside
format strings.)

### Notes

- A `Text` widget defaults to one row — a multi-line frame needs a
  `Length N` (or `Fill`) size line, or only the first line shows.
- Events still can't call events (Quirk 29) — the tick inlines both the
  advance and the render loops.
- State initialisers CAN call module functions (`Dim cols As
  Vec<StreamColumn> = Stream.NewColumns(16)`), like `Life.NewGrid`.

---

## 59 — Rock Paper Scissors (TUI) (2026-08-07)

Project: `projects/59_rock_paper_scissors/`. 7/7 logic tests pass; the TUI
was verified interactively under tmux (r/s/p play rounds; the final round
P vs computer S correctly reported "computer wins").

### Quirk 32 — [CONFIRMED] `move` is a reserved word for parameter names too

`ByVal move As String` fails exactly like Hanoi's `Function Move` (Quirk
18) — `move` is a Rust keyword and the transpiler doesn't escape it.
Renamed to `choice`.

### Notes

- The `Public Sub` pattern (shared logic mutating state ByRef, called
  qualified from each key event) is now the established TUI idiom — used
  by 76 and 59. It also makes the logic directly unit-testable, which
  caught my own test-scenario error (round 2's computer move is "S", so
  player must play "P" to lose — I first asserted a computer win with
  player "R", which actually wins).
- Deterministic computer move via `seed Mod 3` keeps the demo reproducible.

---

## 48 — Monty Hall (GUI) (2026-08-07)

Project: `projects/48_monty_hall/`. First **Window** (Iced) project. 5/5
logic tests pass; the window was verified by launch + screenshot under
WSLg/X11 (title "Monty Hall", three door buttons, phase-0 state).

### Shape of a GUI project (new pattern)

- Pure logic in `<subject>.vbr` — fully testable by `vbr test`, no UI.
- `main.vbr` holds the `Window`: State/View/Events, buttons, conditional
  view via `If phase = 0 Then` blocks.
- **`expected_output.txt` documents "no stdout"** — a Window replaces the
  user's `Main()` with `iced::run`, so there is nothing to diff. The
  verification artifact is a screenshot of the running window.
- **GUI verification recipe** (works under WSLg): `WAYLAND_DISPLAY=
  WINIT_UNIX_BACKEND=x11` (the Wayland path breaks, same as softbuffer),
  launch the built binary in the background, `xwininfo -root -tree` to
  find the window ID by title, `import -window <id> shot.png`, then kill.
  `import` and `xwininfo` (ImageMagick/x11-utils) are available.

### Quirk 33 — [CONFIRMED] Window events can't call other events (same as TUI)

`Event Pick0` calling a sibling `PickDoor(0)` fails ("cannot find function
`pickdoor`"). Same rule as Quirk 29 — put shared logic in a `Public Sub`
in the logic module and call it qualified: `Monty.PickDoor(round, car,
pick, host, phase, message, 0)`. The GUI and TUI event systems share this
behaviour.

### Quirk 34 — [BUG?] `vbr build` on a GUI project only generates; `cargo build` compiles

`vbr build projects/48_monty_hall` produced the project + Cargo.toml but
did not compile the binary; the Iced deps were cached so `cargo build` in
the project's `build/` dir took 3s. (The example run earlier used
`runproject`, which builds.) Worth confirming `vbr build` is meant to be
generate-only.

### What worked

- Conditional view (`If phase = 0 Then … End If`) swaps the button rows
  per phase — clean state-machine UI.
- `Button` + `On Click` with no event parameters is fine when each button
  has its own event (three door events, each calling the shared Sub with a
  constant).

---

## 34 — Hangman (GUI) (2026-08-07)

Project: `projects/34_hangman/`. 5/5 logic tests pass; the window was
verified by launch + screenshot under WSLg/X11 (masked word `______`, 26
letter buttons in two rows).

### Notes

- 26 near-identical button events is verbose but mechanical; each calls
  the shared `Hangman.Guess` Sub qualified. A per-letter parameterised
  Button would need resolver support for event parameters on clicks.
- The view can't call `Hangman.Mask(...)` (Quirk 26), so `masked` is a
  state field refreshed inside `Guess` — the display derivation lives with
  the logic.
- Quirks 19, 26 and 30 all bit in one file (`Mid` arg to a ByVal String
  param, view call, Vec element return) — the workarounds are now routine.

---

## 42 — Magic Fortune Ball (GUI) (2026-08-07)

Project: `projects/42_magic_fortune_ball/`. 4/4 logic tests pass; the
window was verified by launch + screenshot under WSLg/X11 (title, prompt,
Shake button).

### Notes

- Simplest GUI so far: one button, one text line. The event calls module
  functions directly (`Fortune.Intro(shake) & Fortune.Answer(shake)`) and
  stores the result in state — no shared-Sub needed since there's only one
  event.
- Multiline list literal bit again (Quirk 15) — the answers list must be
  one line.
- The deterministic-by-shake answer keeps the demo and tests consistent
  (the book uses randomness).

---

## A4 — Football ELO (DataFrame) (2026-08-07)

Project: `projects/A4_football_data/`. First **DataFrame** project. 7/7
engine tests pass; the full tool read 760 real PL matches and produced a
sane ranking (Liverpool 1684 top, relegated teams at the bottom). First
stdlib-using project too (the `dataframe` feature pulled polars in).

### Quirk 35 — [BUG?] HashMap method args don't auto-borrow a String key

```vb
Dim ratings As HashMap<String, Long>
Dim home As String = homes[i].Clone()
ratings.get(home)                ' � ✘ expected `&_`, found `String`
ratings.contains_key(home)       ' � ✘ expected `&_`, found `String`
```

`HashMap.get` / `contains_key` take a reference (`&str`), and the
transpiler borrows *literals* fine (`ages.get("Alice")` in the example
works) but not a `String` *variable* — no `&home` is emitted. The `get`
result is also `&i64`, so `.Unwrap()` can't widen. Workaround: keep a
`Vec<Rating>` registry with a linear `FindTeam` scan instead (20 teams ×
760 matches is trivial) — what elo.vbr does. Worth a resolver fix:
borrow String args to HashMap methods like literals.

### Quirk 36 — [BUG] A `Const` assigned into a `Double` doesn't auto-widen

```vb
Public Const K_FACTOR As Long = 32
Dim k As Double = K_FACTOR      ' � ✘ expected `f64`, found `i64`
```

The "assign a Long into a Double and you'll see `as f64`" teaching note
applies to *variables*, not `Public Const`s — the constant arrives as i64
and isn't cast. Workaround: use the numeric literal (`32.0`). Also, `i64 *
f64` in an expression never compiles — widen before multiplying.

### Quirk 37 — [MISSING] `DataFrame.Read_Csv` aborts on `N/A` strings

The raw `premier_league_23-26.csv` has `N/A` in numeric columns
(`attendance`, `Game Week`); the stdlib `read_csv` (dataframe.rs:29) has
no null-values/ignore-errors option, so the whole read panics. Workaround:
preprocess a trimmed copy (`pl_trim.csv`, 5 clean columns, via Python).
Worth a `null_values`/`ignore_errors` read option in a later slice.

### What worked well

- `DataFrame.Read_Csv` → `Sort("timestamp")` → `Column(...)` extraction:
  all smooth; polars infers the numeric columns as i64 automatically.
- `df.Column("home_team_name")` as `Vec<String>` and goal columns as
  `Vec<Long>` — the FromColumn impl handles both.
- The `10.0 ^ (diff / 400.0)` exponentiation compiles and matches the Elo
  logistic exactly (verified: equal ratings → 0.5).
- Struct Vec sort needs `.Clone()` on both sides of a swap (Quirk 30
  again): `Dim tmp As Rating = rs[j].Clone()`; `rs[j] = rs[j-1].Clone()`.
- For-loop counters are i32 — widen before passing to a Long param
  (`Dim rank As Long = i + 1`), Quirk 2's family.

---

## 55 — Powerball (DataFrame Group_By/Agg) (2026-08-07)

Project: `projects/55_powerball/`. 6/6 logic tests pass; the full tool ran
1000 tickets through Group_By/Agg and the win simulation. First project to
use the Group_By/Agg and Write_Csv verbs.

### Quirk 38 — [BUG] `Count()` aggregates to u32 — can't extract as `Vec<Long>`

```vb
Dim byPb As DataFrame = df.Group_By("pb").Agg(Count(w1))
Dim counts As Vec<Long> = byPb.Column("w1")   ' � ✘ expected Int64, got u32
```

polars' `count()` returns a u32 column, and the stdlib `FromColumn for
i64` (dataframe.rs:267) refuses anything but Int64. Workaround: write the
grouped frame with `Write_Csv` and read it back — polars reinfers the
counts as i64, so `Column("w1")` works. (That round-trip also exercises
the Write_Csv verb, so it's not pure waste.)

### Quirk 39 — [BUG] Group_By AND Agg on the same column collides

```vb
df.Group_By("w1").Agg(Count(w1))    ' � ✘ column with name 'w1' has more than one occurrence
```

Grouping by `w1` and aggregating `w1` in one call duplicates the output
name (no aliases yet — spec §8). Workaround: aggregate a *different*
column (`Count(w2)`), which is fine for counting.

### Quirk 40 — [CONFIRMED] A `Dim`'d name inside `Agg` becomes a *value*, not a column

```vb
Dim w1 As Vec<Long> = df.Column("w1")
df.Group_By("pb").Agg(Count(w1))    ' � ✘ Vec<i64> doesn't implement Literal
```

The column-formula rule ("Dim'd names are values") bites: if you've
extracted `w1` into a variable, `Count(w1)` in Agg tries `lit(w1)` — a
Vec. Workaround: extract columns under *different* names (`white1`,
`powerball`) so the bare `w1` inside Agg stays a column reference. This is
a real footgun for the read→analyse pipeline: you can't extract and
aggregate the same column in one program without renaming.

### Win simulation result

1000 tickets at $2 = $2000 spent; the fixed draw paid $150 across 36
winning tickets (mostly $4 pb-only hits). Believable lottery behaviour and
deterministic.

---

## 8 — Calendar Maker (DateTime) (2026-08-07)

Project: `projects/8_calendar_maker/`. 6/6 tests pass; output matches.
First project to use the **DateTime** stdlib.

### What worked

- `DateTime.Parse(text, pattern).Unwrap()` + `Format("%u")` gives the ISO
  weekday deterministically — no `Now()`, so the output is reproducible
  (same discipline as the other examples).
- Leap-year logic (`Mod 400 / 100 / 4`) and the month table are plain Bust.

### Quirk 41 — [CONFIRMED] `Dim` is a reserved word as a variable name

```vb
Dim dim As Long = DaysInMonth(...)    ' � ✘ Expected a name after `Dim`, found Dim
```

Naming a variable `dim` fails at parse ("Expected a name after Dim, found
Dim") — the keyword list includes `Dim` itself. Renamed to `days`. (No
diagnostic, just a confusing parse cascade — worth a reserved-name check
like the type-name one.)

### Notes

- My test expectations for the row layout were wrong twice: July 2024's
  trailing week (29-31) is row 5 (header + 4 full weeks), and Jan 2023's
  leading blanks put "  1" at the 6th cell. Copy rows from a first run
  rather than counting cells by hand.

---

## 44 — Maze Runner 2D (Godot) (2026-08-07)

Project: `projects/44_maze_runner_2d/`. First **Godot target** project. The
GDExtension builds and launches in Godot 4.7.1; the live `On Ready`
self-check prints `MAZE LOGIC SELF-CHECK: PASS`; 9/9 standalone logic
tests pass. (`vbr rungodot` was run with the game left open — verified by
the engine's own output, then timeout-killed.)

### Quirk 42 — [DOC GAP] `vbr test` can't compile a Godot main.vbr

The test harness (`generate_project`) compiles every module with the plain
backend, so a `main.vbr` containing `Node2D` blocks fails ("cannot find
module `godot`"). Spec §10 lists ".test.vbr modules inside a Godot
project" as deferred. Workaround used here: a live self-check inside
`On Ready` (prints PASS/FAIL to Godot's output), plus the logic tests run
in a temp dir with a plain stub `main.vbr`. Worth a harness note: skip or
route Godot main.vbr in `cmd_test`.

### Quirk 43 — [BUG?] Sub handlers are only emitted when a Signal exists

A `Sub SelfCheck()` inside a Node2D with no signals isn't emitted at all —
`Sub` handlers lower to `#[func]`s only when there's a `Connect … To`
wiring (godot.rs emits the handler impl only if `!signals.is_empty() ||
!handlers.is_empty()`). Calling a signal-less Sub fails ("cannot find
value"). Workaround: inline the logic into `On Ready`, or declare a dummy
signal so the handler impl is emitted.

### Quirk 44 — [BUG] Field reads inside `Me.DrawRect(...)` args fail the borrow

```vb
Me.DrawRect(Rect2(x * TileSize, ...), ...)   ' � ✘ cannot use self.tilesize
'                                             because it was mutably borrowed
```

The spec says property *writes* are hoisted past the borrow, but reading a
member field (`TileSize`, `px`, `py`) inside a `Me.<method>` argument
still trips rustc's borrow checker (the `BaseMut` temporary lives across
the read). Workaround: copy fields to locals before the call
(`Dim ts As Single = TileSize`, `Dim lx As Long = px`).

### Design note — my maze bug

The first maze used `X` for both the border AND the exit character, so
`IsWall(0,0)` was false (the border wasn't `#`). Redesigned: `#` walls,
one `X` exit at (9,9). The self-check caught it immediately — a live
self-check on startup is a genuinely useful Godot-verification pattern.

### What worked well

- Passthrough is everything the spec promises: `Me.Position`,
  `Me.QueueRedraw()`, `Me.DrawRect(Rect2, Color)`, `Input.IsJustPressed`,
  `GetNode`, signals (`Signal`/`Emit`/`Connect … To`/`Sub OnFinished`) all
  compile and run first-try once the borrow workarounds were in.
- Cross-module calls from node bodies (`Maze.TryMove`, `Maze.IsWall`) are
  plain Bust — the resolver runs on node bodies like any other surface.
- `rungodot` project folder flow (main.vbr + sibling modules + generated
  `*_godot/`) works; the cdylib compiled without Godot present, and Godot
  loaded it at runtime.

---

## 13 — Conway's Game of Life (GUI) (2026-08-07)

Project: `projects/13_game_of_life/`. 8/8 logic tests pass; the window was
verified by launch + screenshot under WSLg/X11 (title, status "Generation
0 — 3 live cells", four buttons, blinker rendered on the canvas). First
project with a **Canvas** Draw block.

### Quirk 45 — [CONFIRMED] `Step` is a reserved word (again)

`On Click Step` / `Event Step` fails like Quirk 27's `Function Step` —
renamed to `StepOnce`. The reserved list (`To`, `Sub`, `Move`, `Dim`,
`Step`, …) keeps growing; worth a single documented list.

### Quirk 46 — [CONFIRMED] Named colours are a fixed list

`Color.LightGray` and `Color.ForestGreen` are rejected — the named set is
exactly: Black, White, Red, Green, Blue, Gray, Yellow, Orange, Purple,
Navy, Cyan, Magenta. Use `Color(r, g, b)` for anything else, and note it
takes **3** args, not 4 (no alpha).

### Quirk 47 — [BUG?] A state field initialiser can't read a sibling field

```vb
State
    Dim grid As Vec<Long> = Life.BlinkerSeed(20, 15)
    Dim rects As Vec<CellRect> = Life.LiveRects(grid, 20)   ' � ✘ cannot find value `grid`
End State
```

State fields are initialised independently — `grid` isn't in scope for
`rects`'s initialiser (the generated struct literal can't reference
another field). Workaround: compute from a fresh identical call
(`Life.LiveRects(Life.BlinkerSeed(20, 15), 20)`), or fill `rects` in an
event. Worth a resolver note: state initialisers see only their own line.

### What worked well

- The documented data-driven Canvas pattern: compute `Vec<CellRect>` in
  the events, `For Each` it in Draw with `Fill Rect` — no resolver needed
  in the canvas body.
- `Stroke Line` grid lines, `Color(r, g, b)` custom colours.
- The blinker oscillation is fully unit-tested, so the GUI's Step button
  behaviour is proven by the logic tests (synthetic clicks aren't
  available — no xdotool on this box).

---

## 53 — Periodic Table (Python target) (2026-08-08)

Project: `projects/53_periodic_table/`. First **Python target** project.
7/7 logic tests pass on the Rust side; the single-file py variant's
Python output is **byte-for-byte identical** to its Rust build (verified
by diff — the ground-truth discipline from targets_spec.md). The vbrpy
stdlib package (FileSystem) runs with zero pip installs.

### Quirk 48 — [MISSING] `vbr py` has no multi-module project mode

`cmd_py` reads and transpiles **one file**; a project folder is rejected
("Is a directory"). There's no parallel of `runproject` for the Python
target — sibling modules aren't compiled, so a `main.vbr` calling
`Periodic.ParseRow` emits Python that references an undefined
`periodic.parserow`. Workaround: keep the multi-module version for Rust
and maintain a single-file variant (`py/main.vbr`) for the Python target.
Worth a spec note: `vbr py <folder>` should behave like `runproject`.

### Quirk 49 — [MISSING] Python backend lacks the VB string builtins

A probe showed the Python target lowers `.Len()` → `len()` (works) but
passes `Mid`, `Left`, `Right`, `Val`, `UCase`, `LCase`, `Chr` straight
through as **undefined names** (`NameError: name 'mid' is not defined`).
The Rust backend has all of these; the Python `call()` table only covers
maths builtins, `CStr`, `Sleep`. Workaround: avoid them in py-target
source (hardcode tables, use `.Len()`/`&`/comparisons only), or write a
small helper in the program. This is a real coverage gap worth closing —
string code is the first thing a Python-target program reaches for.

### Quirk 50 — [CONFIRMED] `ByRef` scalar params can't be emulated in Python

```vb
Sub SetFlag(ByRef found As Boolean)   ' ⚠ assignment won't reach the caller
```

The transpiler warns: "`ByRef` parameter can't be emulated for a scalar in
Python — passed by value". Workaround: return a sentinel value instead of
a ByRef flag (the `NoneElement()` pattern — empty Element means "not
found").

### What worked well

- `Type` → `@dataclass`, `list.append`, f-strings, `len()`: the generated
  Python is genuinely idiomatic, not a transliteration.
- `FileSystem.Read_Lines` + `_unwrap` from vbrpy — the stdlib package works
  on pure Python batteries, no pip.
- The byte-identical diff is a great teaching moment: same program, three
  backends, one output.

---

## 35 — Hex Grid (C target) (2026-08-08)

Project: `projects/35_hex_grid/`. First **C target** project. 6/6 logic
tests pass on Rust; the single-file c variant compiles with plain
`cc -lm` (no warnings) and its output is **byte-for-byte identical** to
the Rust build (verified by diff).

### Quirk 51 — [CONFIRMED] `vbr c` is single-file only, like `vbr py`

`vbr c <folder>` is rejected ("Is a directory") and a multi-module
main.vbr references sibling functions that don't exist in the emitted C.
Same gap as Quirk 48 — the alternative targets have no `runproject`
equivalent. Workaround: single-file variant in a `c/` subfolder.

### Quirk 52 — [CONFIRMED] C backend lacks the VB string builtins too

`Mid`/`UCase`/`Left` in a C-target program become implicit-declaration
warnings and undefined references at link time (`undefined reference to
'mid'`). Same gap as Python (Quirk 49). The C target's usable string
surface is `.Len()` and `&` concatenation. The hex-grid project is
deliberately built from loops + concat only so it transpiles cleanly to
all three targets.

### What worked well

- Generated C is idiomatic: `Vec_str` monomorphised struct with growable
  push, `vbr_concat`, `long long` for Long, `size_t` loops — reads like
  hand-written C.
- `For` loops, `Mod`, `If/ElseIf`, string building all lower cleanly;
  `cc -lm` compiles with zero warnings.
- Byte-identical output across Rust, Python (project 53) and C — the
  ground-truth discipline holds for all three targets.

---

## A6 — Poker TUI (Texas Hold'em equity) (2026-08-08)

Project: `projects/A6_poker_tui/`. A complete poker-hand evaluator +
Monte-Carlo equity TUI. 15/15 logic tests pass; the TUI was verified under
tmux (Ah Kd vs 2 players, flop `7h Js 6c` → 63% to win; board reveals in
stages; `q` quits).

### Quirk 53 — [BUG] ByRef `pos` double-advance in CompleteHand

`CompleteHand` advances the shared deck position `pos` internally for each
filler it draws; the caller also added `(5 - board.Len())` after the call —
a double-count that exhausted the deck and panicked at the flop
("index out of bounds: len 47, index 48") at 300 trials. It passed tests
at 200 trials purely by luck (fewer trials = smaller chance of the drift
landing out of range). Fix: the caller must not advance `pos` again.
Lesson: when a helper owns a `ByRef` position, the caller must not also
recompute the advance. A higher-trial test would have caught this earlier —
the deterministic-equity test should run enough trials to exercise the
full deck slice.

### What worked well

- The whole hand-evaluator design: single comparable `Long` score =
  category × 15⁵ + base-15 rank signature — makes equity a plain `>`
  comparison. The wheel, kickers and category ladder all test clean.
- Fisher-Yates over a fixed-seed LCG gives reproducible equity; same-call
  determinism is asserted directly in the tests.
- `Sub`-per-stage pattern held up (Quirk 29 workaround): `TrySetup` and
  `ShowEquity` as Public Subs mutating state ByRef.
- TUI focus note: typing into the players field right after the card
  inputs needs Tab to move focus — my "23 players" was my own input error,
  not an app bug.

---

## A5 — Shape Areas (sum types) (2026-08-08)

Project: `projects/A5_shape_areas/`. First project to use **data-carrying
enums** — the biggest previously-untested core feature. 9/9 tests pass
first try; output matches.

### What worked (all first try)

- `Enum Shape: Circle(Double), Rectangle(Double, Double),
  Triangle(Double, Double), Polygon(Vec<Point>), Empty` — scalar, multiple,
  struct and Vec payloads all lower cleanly.
- `Match s` with `Shape.Circle(r) => Return ...` unpacks the payload; the
  binding is lowercase (raw Rust pattern, Quirk 3's rule) but works
  exactly as the sum_types.vbr example shows.
- `Sqr`, the shoelace formula, struct payloads via `Point { x: ..., y: ... }`
  — no surprises.

### Note

The spec's examples (`sum_types.vbr`, `enum_payloads.vbr`) are accurate —
no transpiler quirks surfaced here. A satisfyingly clean first test of
Bust's signature feature.

---
