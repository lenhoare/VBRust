# A4 — Football ELO

A command-line tool that computes a classic **Elo rating** for every
Premier League team from three seasons of results (2023–24 to 2025–26),
using Bust **DataFrames** to read and process the CSV.

## What it does

1. Reads `pl_trim.csv` with `DataFrame.Read_Csv` — 760 matches.
2. Sorts by `timestamp` so matches are processed chronologically.
3. Extracts the home team, away team, and goal columns as `Vec<String>` /
   `Vec<Long>`.
4. Feeds them to the Elo engine (`elo.vbr`): every team starts at 1500,
   each match moves ratings by K·(actual − expected) with K=32 — the
   standard logistic Elo.
5. Prints the final ranking, best-rated first (ties alphabetical).

Sanity check: Liverpool top (1684), Man City third (1676), and the teams
relegated across these seasons (Luton, Burnley, Sheffield United,
Southampton, Ipswich) sit at the bottom — exactly what the underlying
results imply.

## Bust language features tested

**DataFrame (the point of this project):**
- `DataFrame.Read_Csv(path)`
- `df.Sort("timestamp")` — ascending sort by one column
- `df.Column(name)` — typed extraction into `Vec<String>` / `Vec<Long>`
- See notes.md Quirk 35–36 for two DataFrame-adjacent findings (HashMap
  key borrows; `N/A` values abort Read_Csv — the CSV was trimmed for this)

**Core language (in elo.vbr, 7 unit tests):**
- `Public Type Rating` (team + points), built complete
- `Public Function` returning `Double` / `Long` / `Vec<Rating>`
- `Public Sub SortDesc(ByRef rs As Vec<Rating>)` — insertion sort on a
  struct Vec, with `.Clone()` on moves
- `10.0 ^ (diff / 400.0)` exponentiation; `Round()` for integer ratings

## Standard-library features tested

- `DataFrame` (polars-backed) — read, sort, column extraction

## Running it

```sh
vbr runproject projects/A4_football_data    # build + run (dataframe feature)
vbr test        projects/A4_football_data   # run the 7 engine tests
```

## Data files

- `premier_league_23-26.csv` — your original file (untouched)
- `pl_trim.csv` — the trimmed copy the tool reads (5 clean columns; the
  raw file's `N/A` values abort `Read_Csv` — see notes.md)
