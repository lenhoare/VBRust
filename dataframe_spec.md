# VBR DataFrame Specification

A `DataFrame` is a native, first-class table in VBR — columns of typed data you
read, transform with **column formulas**, and write back out. It is backed by the
Rust **polars** crate: pure Rust, no Python, no GIL, self-contained. (This is a
different track from inline `Python` blocks — see `inline_python_spec` notes — and
the better home for dataframes because it needs no interpreter.)

> Status: **slices 1–3 BUILT** (read/inspect/`Select`/`WithColumn`/`Filter`/
> `Sort`/column-out/write + the full column-formula lowering; **GroupBy/Agg**
> and whole-column scalar aggregations; **joins** with `IsNull` handling).
> Later slices (lazy, more formats) are marked §8 and not yet built.

---

## 1. Design goals

- **Think in whole columns, like an Excel array formula.** You write formulas
  over columns (`price * qty`, `IIf(age >= 18, "adult", "minor")`) and they apply
  down the whole column. This is exactly what polars' expression engine does.
- **Native and self-contained.** Backed by the `polars` crate — pure Rust. A
  dataframe program needs no Python and links no interpreter.
- **Readable generated Rust.** The *verbs* (`ReadCsv`, `WithColumn`) are a thin
  `vbr_stdlib` wrapper for clean IO and error handling; the *formulas* lower to
  genuine polars expressions (`col`, `lit`, `when/then/otherwise`) — so the
  generated code teaches the real polars engine underneath.
- **Familiar to a VB/SQL mind.** `IIf` is the VB6 ternary you already know;
  backtick-quoted names and `Col(...)` echo SQL's bracketed identifiers.

---

## 2. Where it lives

A `dataframe` **feature on `vbr_stdlib`**, gated exactly like `Json`/`Http`/
`DateTime`. Nothing that doesn't use a `DataFrame` pays for it; the project build
detects usage and enables the feature. Because polars is a **heavy** dependency
(long first compile, large binary — the Iced tier, not the regex tier), a build
notice warns on the first build. Dataframe programs are `runproject` programs
(like anything using the stdlib).

---

## 3. The `DataFrame` type

`DataFrame` is a `DeclType::Named("DataFrame")`. You get one by reading a file and
pass it through transforms (each returns a new `DataFrame`):

```vb
Dim df = DataFrame.ReadCsv("people.csv")
```

Reading (slice 1: CSV): `DataFrame.ReadCsv(path)`. Later: `ReadParquet`, `ReadJson`.

Inspecting:

| Call | Result |
|------|--------|
| `df.Head(n)` | a `DataFrame` of the first `n` rows |
| `df.Shape()` | `(rows, cols)` as `(Long, Long)` |
| `df.Columns()` | `Vec<String>` of column names |
| `df.Print` | pretty-print the frame (debugging) |

---

## 4. Column formulas — the heart

The arguments to `Filter`, `WithColumn`, and `Select` are a **column-formula
context**. Your ordinary VBR expression is read as a formula over columns: it
applies down the whole column and broadcasts elementwise. The same VBR operators
and grammar you already use — only the meaning of the operands changes:

| You write | Means | Lowers to |
|-----------|-------|-----------|
| `age` | column (simple name) | `col("age")` |
| `` `Unit Price` `` | column (awkward name — spaces, symbols) | `col("Unit Price")` |
| `Col(selected)` | column named by an expression/variable (**dynamic**) | `col(selected)` |
| `"adult"` | a string **value** | `lit("adult")` |
| `30`, `3.14`, `True` | a literal value | `lit(30)` … |
| `cutoff` (a `Dim`'d variable) | that variable's **value** | `lit(cutoff)` |

The rule in one line: **bare/backtick/`Col(…)` are columns; literals and `Dim`'d
names are values.** This keeps string literals always meaning string *values* (so
`category = "adult"` is unambiguous), while `Col(var)` is the one form that can
name a column chosen at runtime — and the explicit override when a `Dim`'d name
should be read as a column instead of a value.

Operators broadcast down the column: `+ - * / ^ Mod`, comparisons `> < >= <= = <>`,
and logical `And Or Not` (elementwise boolean masks). The VB6 **`IIf(cond, then,
else)`** is the array-formula `IF`, lowering to polars `when/then/otherwise`.

Examples and their lowering:

```vb
df = df.WithColumn("total", price * qty)
'   → df.with_columns([(col("price") * col("qty")).alias("total")])

df = df.Filter(age > 30 And active)
'   → df.filter(col("age").gt(lit(30)).and(col("active")))

df = df.WithColumn("band", IIf(age >= 18, "adult", "minor"))
'   → ... when(col("age").gt_eq(lit(18))).then(lit("adult")).otherwise(lit("minor")).alias("band")

df = df.Filter(Col(selected) = target)          ' dynamic column, injected value
'   → df.filter(col(selected).eq(lit(target)))

df = df.Filter(`Order Date` >= start)           ' awkward name, Dim'd value
'   → df.filter(col("Order Date").gt_eq(lit(start)))
```

`Select` takes column names (or formulas): `df.Select("name", "band", "total")`.

### 4b. GroupBy & aggregation

`df.GroupBy(key, …)` groups rows by one or more key columns (string names, like
`Select`); the chained **`.Agg(…)`** says what to compute per group — one
aggregation per output column. Inside `Agg`, the aggregation functions
**`Sum` / `Mean` / `Min` / `Max` / `Count`** wrap an ordinary column formula:

```vb
Dim byband As DataFrame = df.GroupBy("band").Agg(Count(name), Mean(age), Sum(qty))
'   → df.group_by(&["band"]).agg(&[col("name").count(), col("age").mean(), col("qty").sum()])

Dim spend As DataFrame = df.GroupBy("band").Agg(Sum(price * qty))
'   → … .agg(&[(col("price") * col("qty")).sum()])
```

- Groups keep **first-seen order** (polars' stable group-by), so output is
  deterministic.
- An aggregated column keeps its source column's name (aliases are a later
  slice — for now, avoid two aggregations of the same column in one `Agg`).
- `GroupBy` is used inline with `.Agg(…)` — the grouped intermediate isn't a
  `DataFrame`, so it can't be stored in a variable.

### 4c. Joins

Combine two frames on shared key column(s), SQL-style. Three verbs, chosen by
which rows survive:

```vb
Dim buyers As DataFrame = people.Join(orders, "name")       ' inner: matches only
Dim everyone As DataFrame = people.LeftJoin(orders, "name") ' all left rows
Dim union As DataFrame = people.OuterJoin(orders, "name")   ' all rows, both sides
```

- Keys are string column names (several allowed: `Join(other, "a", "b")`); the
  key must exist under the same name in both frames (differing names —
  `left_on`/`right_on` — are a later slice). Key columns come out **coalesced**:
  one `name` column, not `name` and `name_right`.
- **Nulls.** Where a key found no match, the new cells are **null** — a state
  VBR has no type for. `IsNull(col)` in a `Filter` formula finds those rows
  (`df.Filter(Not IsNull(item))` removes them), and `df.Column(...)` **refuses**
  a column containing nulls rather than silently returning a shortened `Vec`.

---

## 5. Getting data out

Cross the boundary into plain VBR by naming a type — one bulk extraction:

```vb
Dim ages As Vec<Long> = df.Column("age")
Dim names As Vec<String> = df.Column("name")
```

`df.Column(name)` → the column as a typed `Vec<T>`.

**Scalar aggregations** give a single number for a whole column, always as a
`Double` (one simple type for every aggregation):

```vb
Debug.Print "mean age: " & df.Mean("age")     ' also Sum / Min / Max
```

---

## 6. Writing

`df.WriteCsv(path)` — write the frame to CSV. Later: `WriteParquet`, `WriteJson`.

---

## 7. Eager vs lazy

Slice 1 is **eager**: each transform runs immediately and returns a materialised
`DataFrame` — simplest to reason about. Polars' lazy engine (query optimisation
over a whole pipeline) comes later via `.Lazy()` / `.Collect()`, once the eager
surface is proven.

---

## 8. Deferred (later slices)

- **Aggregation aliases** — naming an `Agg` output (`Sum(qty) As "total"`), so
  two aggregations of one column can coexist.
- **Join options** — differing key names per side (`left_on`/`right_on`), and
  the rarer Right/Semi/Anti join types.
- **Lazy pipeline** — `.Lazy()` / `.Collect()` and lazy-only optimisations.
- **More formats** — Parquet, JSON, and read options (`CleanHeaders` to snake_case
  headers on read, schema/dtype overrides).
- **More expression functions** — string ops, dates, `Cast`, window functions,
  `Sort` by multiple keys / descending.

---

## 9. Examples

`examples/dataframe_basics.vbr` (read → inspect → `WithColumn`/`Filter`/`Select`
formulas → `Column` out → write) lands with slice 1;
`examples/dataframe_groupby.vbr` (scalar aggregations → `GroupBy`/`Agg`,
including a formula inside `Sum`) with slice 2;
`examples/dataframe_join.vbr` (inner/left joins over `people.csv` +
`orders.csv`, `IsNull` filtering, column extraction) with slice 3.
