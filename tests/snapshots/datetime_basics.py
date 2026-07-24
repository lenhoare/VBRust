# DateTime from the standard library — parse a fixed moment, then read, format
# and shift it. (Uses Parse, not Now, so the output is deterministic.)

from vbrpy import Some, Ok, Err, _vb, _unwrap, DateTime

def main():
    d: DateTime = _unwrap(DateTime.parse('2026-07-24 09:30:00', '%Y-%m-%d %H:%M:%S'))
    print(f"year:  {_vb(d.year())}")
    print(f"month: {_vb(d.month())}")
    print(f"day:   {_vb(d.day())}")
    print(f"iso:   {_vb(d.format('%Y-%m-%d'))}")
    later: DateTime = d.adddays(10)
    print(f"in 10 days: {_vb(later.format('%Y-%m-%d'))}")
    soon: DateTime = d.addhours(5)
    print(f"in 5 hours: {_vb(soon.format('%H:%M'))}")


if __name__ == "__main__":
    main()
