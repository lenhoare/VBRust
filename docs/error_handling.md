# Error handling in VBR — where we are, and where it creaks

*A stock-take of every failure-handling mechanism VBR has today, the
inconsistencies between them, and some recommendations to chew on before you
make architectural changes. Written from a read of the compiler, not from
memory — the two "bugs" at the end are reproduced against `rustc`.*

---

## TL;DR

VBR's headline story is clean and VB6-friendly: **there are no exceptions and no
`On Error GoTo` — a failure is an ordinary returned value**, carried in a
`Result<T>` or an `Option<T>`, and you deal with it in one of three ways
(handle / propagate / unwrap). That core is good and worth keeping.

The creaks are around the *edges* of that core:

1. **Too many verbs** for a beginner to hold at once (`Result`, `Option`,
   `Ok/Err`, `Some/None`, `?`, `.Unwrap`, `.Unwrap_Or`, `Is Some`, `.Is_Ok`,
   `.Is_Some`, `.Is_None`, `.Is_Err`, `Match`).
2. **The same situation fails in three different shapes** depending on which
   function you happened to call (a missing value is sometimes an `Option`,
   sometimes a `Result`, sometimes a silent default, sometimes a panic).
3. **Some failures are handled behind your back** — `Val` swallows bad input,
   `list[i]` / `map[k]` panic invisibly — which is exactly the "we sometimes
   handle them behind the scenes" you flagged.
4. **Two real holes** where the compiler currently emits Rust that doesn't
   compile: `?` on an `Option` inside a `Result` function, and
   `Function Main() As Result<...>`.

---

## 1. The mental model today

Fallibility is a **property of a function's type**, declared with `As`:

```vb
Function Divide(ByVal a As Long, ByVal b As Long) As Result<Long>
    If b = 0 Then Return Err("cannot divide by zero")
    Return Ok(a / b)
End Function
```

`As Option<T>` (`Some`/`None`) is the same idea for "a value, or nothing." Then
the guide teaches **three things you can do** with the box you get back:

```vb
' 1. HANDLE — examine both outcomes
Match Divide(10, 2)
    Ok(value)   => Debug.Print "got " & value
    Err(reason) => Debug.Print "failed: " & reason
End Match

' 2. PROPAGATE — "not my job", hand the Err back to my caller
Dim q As Long = Divide(a, b)?

' 3. UNWRAP — training wheels, crashes on failure
Dim v As Long = Divide(10, 2).Unwrap()
```

Plus a fourth, lighter tool for the "I only care about the good case" path — the
`Is` / *if-let* form:

```vb
If cache.Get(key) Is Some(value) Then Debug.Print value
Do While q.Pop() Is Some(item)  ' the while-let loop
```

`On Error` is explicitly rejected with a teaching diagnostic pointing you here.

---

## 2. The full inventory

Everything the language currently offers, in one place:

| Mechanism | Spelling | Lowers to | Notes |
|-----------|----------|-----------|-------|
| Declare fallibility | `As Result<T>` / `As Option<T>` | `Result<T, String>` / `Option<T>` | **Error type is always `String`** on the common path |
| Signal failure | `Return Err("...")` / `Return None` | `Err(...)` / `None` | |
| Signal success | `Return Ok(x)` / `Return Some(x)` | `Ok(x)` / `Some(x)` | payload gets numeric coercion to the declared inner type |
| Handle both | `Match … Ok(v)=> / Err(e)=>` | `match` | `Err(pattern)` can itself be a typed-error enum |
| Propagate | postfix `?` | `?` | legal only where the enclosing fn returns `Result`/`Option` |
| Unwrap (crash) | `.Unwrap()` | `.unwrap()` | warns once ("training wheels") |
| Unwrap with default | `.Unwrap_Or(d)` / `.Unwrap_Or_Else(...)` | same | |
| if-let / while-let | `If x Is Some(v) Then` / `Do While … Is` | `if let` / `while let` | |
| Query | `.Is_Some` `.Is_None` `.Is_Ok` `.Is_Err` | same | boolean tests |

---

## 3. Where failure actually comes from

The mechanisms above are the *consumer* side. On the *producer* side, VBR is much
less uniform — the same conceptual event ("this might not work") is modelled four
different ways depending on the source:

| Source | Example | Failure shape |
|--------|---------|---------------|
| **Stdlib I/O** | `FileSystem.Read`, `Http.Get`, `Json.Parse`, `Database.Query`, `Shell.Run`, `DateTime.Parse`, `Regex.Replace` | `Result<T, String>` |
| **Stdlib predicates** | `FileSystem.Exists`, `Json.HasKey`, `Json.IsNull`, `Process.IsRunning` | plain `Boolean` (never fail) |
| **Strict conversions** | `CLng`, `CInt`, `CDbl` | `Result<T, String>` |
| **Lenient conversion** | `Val(" 42x ")` | **silent** — a `Double`, `0.0` for junk, *never fails* |
| **Search** | `InStr` | `Option` |
| **Collection read** | `.Get(i)`, `.First()`, `.Last()`, `.Pop()` | `Option<T>` |
| **Collection index** | `list[i]`, `map[k]` | **panic** on miss — no `Result`, no `Option` |

So "the thing I asked for might not be there" can hand you a `Result`, an
`Option`, a silent `0`, or a runtime crash — **four contracts for one idea.**
That's the deepest inconsistency, and it's the root of most of the others.

---

## 4. The guardrails (diagnostics)

Credit where due — the compiler does nudge you. These fire today:

- **`ignored-result`** — a bare fallible call whose value is discarded is an
  *error*: "This Result is being thrown away. Handle it: `?`, `Match`, or assign
  it with `Dim`."
- **`try-needs-result`** — `?` used where the function can't fail.
- **`unwrap-training-wheels`** — warn once on `.Unwrap()`.
- **`index-bounds`** — a note that `x[i]` panics and `.Get(i)` is the checked read.

They're good, but they're scattered — four separate messages with no single "here
is how failure works in VBR" they all point back to.

---

## 5. The inconsistencies, ranked

**A. One idea, four shapes (§3).** The big one. A missing value is variously an
`Option`, a `Result`, a silent default, or a panic. A learner can't predict which
without memorising a table.

**B. Behind-the-scenes handling — the thing you flagged.**
- `Val` eats bad input and returns `0.0`. Classic VB footgun, now silent.
- `list[i]` / `map[k]` panic with no visible sign at the call site.
- Numeric coercions get inserted for you (defensible as teaching, but it *is*
  the compiler doing something you didn't write).

**C. `Option` and `Result` are near-duplicate vocabularies.** `?`, `.Unwrap`,
`.Unwrap_Or`, `Is Some`/`Is Ok`, `.Is_*` all come in two parallel flavours, and a
single expression often forces you to switch mid-chain — `db.Query(...)?` gives a
`Result`, then `.First()` on the rows gives an `Option`. You change vocabulary
without changing intent.

**D. Stringly-typed errors, but the machinery half-supports typed ones.** Every
stdlib error is `Result<T, String>`, yet `Match` already lets `Err(SomeEnum.Case)`
destructure a typed error. The engine is more ambitious than the idiom — an
unfinished decision, not a finished design.

**E. Two holes where we used to emit code that won't compile — now FIXED**
*(2026-08-14; both were reproduced against `rustc`, fixed, and regression-tested
in `tests/compiler_fixes.rs`)*:

- **`?` on an `Option` inside a `Result` function** (and vice versa). Previously
  `Dim v As Long = xs.First()?` in an `As Result<Long>` function generated
  `xs.first().copied()?`, which rustc rejected (E0277). The `?` check now reads
  the *operand's* shape (`try_operand_shape` in `resolver.rs`) and, on a
  mismatch, raises a VB-level error instead of leaking a rustc one — e.g. "this
  value is an `Option` … give the empty case a reason first with `.Ok_Or("…")`,
  or declare this function `As Option<T>`." Matching shapes still pass through.

- **`?` in `Main`.** `Main` is now the one function that may propagate without an
  explicit fallible return: a plain `Function Main()` whose body uses `?` is
  emitted as `fn main() -> Result<(), String>` with a closing `Ok(())`, so error
  propagation works in the function every program has. Declaring
  `Function Main() As Result<…>` (which can't map to a valid `fn main`) is now a
  clear error steering you to the plain form, rather than broken Rust.

  *(Note the small asymmetry this introduces — see R-note below.)*

**F. Naming regimes collide on the error verbs.** `.Unwrap_Or` (literal Rust,
underscored) sits next to stdlib `.GetString` (PascalCase). This is the
deliberate "a method is its Rust name" decision, but the failure-handling verbs
are exactly where a beginner meets both styles in one breath.

---

## 6. Recommendations

Ordered by bang-for-buck. None of these are urgent bug-fixes except where noted;
they're a menu for the redesign.

**R1 — Fix the two holes regardless of any bigger redesign. ✅ DONE (2026-08-14).**
- `?` is now shape-aware: an `Option`'s `?` in a `Result` function (or vice
  versa) raises a VB-level error suggesting `.Ok_Or("…")` / `.Ok()`, instead of
  leaking a rustc E0277.
- `Main` is special-cased: a plain `Function Main()` that uses `?` becomes
  `fn main() -> Result<(), String>` + `Ok(())`; an explicit fallible return on
  `Main` is rejected with guidance.

  **R-note (a wrinkle worth a later think):** the `Main` special-case is itself a
  little "behind the scenes" — the exact kind of magic §B is wary of, but here it
  buys real ergonomics. It also means `?` behaves differently in `Main` (auto
  Result) than in any other `Sub`/`Function` (must declare). That's defensible
  (Rust's own `main` is special too), but it's an asymmetry to keep in mind when
  deciding R2/R3.

**R2 — Pick one shape per situation and enforce it.** Write down the doctrine and
make the language obey it:
- *"a value might be absent"* → **`Option`** (map miss, list read, `InStr`).
- *"an operation can fail with a reason"* → **`Result`** (I/O, parse, network).
- Then reconcile the outliers: give `list[i]`/`map[k]` a blessed checked form (or
  promote `.Get` everywhere and brand `[]` loudly as "panics — you're asserting
  it's there"), and decide `Val`'s fate (see R4).

**R3 — Shrink the vocabulary to three blessed tools.** Teach **`Is` (if-let)**,
**`?` (propagate)**, and **`Match` (handle)** as *the* way, and demote `.Unwrap*`
to a warned escape hatch. Consider unifying the beginner's view of `Option` and
`Result` so the same three tools read identically on both (e.g. treat `Option`'s
`None` as an `Err("")` for teaching purposes) — fewer parallel words, one story.

**R4 — Make silent handling opt-in, not the default.** The `Val`-returns-0 and
`[]`-panics behaviours are the "behind the scenes" you dislike. Options, cheapest
first: (a) document every silent-fallback builtin in one table and emit a
one-time teaching note like the unwrap one; (b) require an explicit spelling for
lenience (`Val(x, orElse:=0)` or a distinct `TryVal`); (c) make the strict form
the default and lenience the thing you ask for.

**R5 — Settle the typed-error question before `Result<T, String>` ossifies.**
Either commit to "errors are Strings, full stop" (simplest, very teachable, and
honestly fine for a teaching language) *or* introduce a first-class error enum
story and thread it through the stdlib. The current half-way state — string
errors everywhere but `Match Err(enum)` quietly working — is the inconsistency.
For a teaching tool I'd lean toward **Strings, stated plainly**, and revisit only
if a real app needs to branch on error kind.

**R6 — One page, one family of diagnostics.** Consolidate `ignored-result`,
`try-needs-result`, `unwrap-training-wheels`, and `index-bounds` under a single
"how failure works in VBR" model, and have each message link back to it. This is
mostly a docs/diagnostics-copy job and makes the whole system *feel* coherent
even before the deeper changes land.

---

## 7. If you were starting the error story from scratch

A sketch, not a proposal — to react against:

- **One fallible type in the surface language.** Keep `Result<T>` as *the* box;
  present `Option` as `Result<T>` with an empty reason (`None` ≈ `Err("")`), so
  `Is`, `?`, `Match`, and `.Unwrap_Or` have exactly one meaning each. (Rust still
  gets real `Option`/`Result` underneath; this is about what the *learner* holds.)
- **Errors are `String`.** Human-readable, VB-ish, no type gymnastics. A power
  user reaches for an inline `Rust` block if they need typed errors.
- **Nothing fails silently.** `Val`-style lenience only when you spell it. `[]`
  stays as the "I'm asserting this is valid" sharp tool, with `.Get` the default
  the docs reach for.
- **`?` everywhere it reads naturally, including `Main`.** The compiler makes the
  signatures line up so the beginner never meets a `Termination` error or a
  `FromResidual` mismatch.

The through-line: **one box, one set of verbs, no silent saves, and the compiler
speaks VB when the shapes don't match — never Rust.**
