# 8 — Calendar Maker

Builds printable monthly calendars as text grids (Monday-first, ISO
weeks), for any year and month. Uses the **DateTime** stdlib for weekday
calculation (deterministic via `Parse`, not `Now`), with leap-year and
days-per-month logic in pure Bust.

## Bust language features tested

**Standard library — DateTime (first project to use it):**
- `DateTime.Parse(text, pattern)` — build a fixed moment
- `d.Format("%u")` — ISO weekday (1=Mon..7=Sun); the only date piece
  needed, so no wall-clock dependence
- Deterministic: no `Now()` anywhere, so output is reproducible

**Core language (in calendar.vbr, 6 unit tests):**
- `IsLeap` — the Gregorian rule (`Mod 400 / 100 / 4`)
- `DaysInMonth` — month table + February special case
- `BuildCalendar` — header row, leading blanks, 7-cell week wrapping
- `Pad3` / `Pad2` string helpers; `Vec<String>` rows

## Standard-library features tested

- `DateTime` — Parse, Format

## Running it

```sh
vbr runproject projects/8_calendar_maker    # build + run
vbr test        projects/8_calendar_maker   # run the 6 tests
```

## Expected output

Three calendars: July 2024 (starts Monday), February 2024 (leap, 29 days,
starts Thursday), January 2023 (starts Sunday). Verified byte-for-byte.
