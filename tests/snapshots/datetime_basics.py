# DateTime from the standard library — parse a fixed moment, then read, format
# and shift it. (Uses Parse, not Now, so the output is deterministic.)

import sys
from vbrpy import Ok, Err, _vb, DateTime

def main():
    _t0 = DateTime.parse('2026-07-24 09:30:00', '%Y-%m-%d %H:%M:%S')
    if isinstance(_t0, Err):
        print(f"Error: {_t0.error}", file=sys.stderr)
        raise SystemExit(1)
    d: DateTime = _t0.value
    print(f"year:  {_vb(d.year())}")
    print(f"month: {_vb(d.month())}")
    print(f"day:   {_vb(d.day())}")
    print(f"iso:   {_vb(d.format('%Y-%m-%d'))}")
    later: DateTime = d.add_days(10)
    print(f"in 10 days: {_vb(later.format('%Y-%m-%d'))}")
    soon: DateTime = d.add_hours(5)
    print(f"in 5 hours: {_vb(soon.format('%H:%M'))}")


if __name__ == "__main__":
    main()
