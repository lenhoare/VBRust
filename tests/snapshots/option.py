# Option<T> for maybe-absent values — Some / None

import sys
from dataclasses import dataclass

@dataclass
class Some:
    value: object

@dataclass
class Ok:
    value: object

@dataclass
class Err:
    error: object

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

def main():
    _t0 = halve(10)
    if isinstance(_t0, Err):
        print(f"Error: {_t0.error}", file=sys.stderr)
        raise SystemExit(1)
    _m0 = _t0.value
    match _m0:
        case Some(value):
            print(f"half of 10 = {_vb(value)}")
        case None:
            print('10 is odd, no exact half')
    _t1 = halve(7)
    if isinstance(_t1, Err):
        print(f"Error: {_t1.error}", file=sys.stderr)
        raise SystemExit(1)
    _m1 = _t1.value
    match _m1:
        case Some(value):
            print(f"half of 7 = {_vb(value)}")
        case None:
            print('7 is odd, no exact half')

def halve(n: int) -> object:
    if (n % 2) == 0:
        return Ok(Some(n // 2))
        # `/` floats; the Option<Long> payload narrows back to Long
    return Ok(None)


if __name__ == "__main__":
    main()
