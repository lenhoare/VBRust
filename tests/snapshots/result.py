# Errors propagate automatically. Intercept a call with Handle; fail with RaiseError.

import sys
from dataclasses import dataclass

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
    value: int = 0
    _t0 = divide(10, 2)
    if isinstance(_t0, Err):
        message = _t0.error
        print(f"error: {_vb(message)}")
        return
    else:
        value = _t0.value
    print(f"10 / 2 = {_vb(value)}")
    bad: int = 0
    _t1 = divide(7, 0)
    if isinstance(_t1, Err):
        message = _t1.error
        print(f"error: {_vb(message)}")
        return
    else:
        bad = _t1.value
    print(f"7 / 0 = {_vb(bad)}")
    # Failure from Divide flows out of DoubleQuotient with no extra syntax
    doubled: int = 0
    _t2 = doublequotient(20, 4)
    if isinstance(_t2, Err):
        message = _t2.error
        print(f"error: {_vb(message)}")
        return
    else:
        doubled = _t2.value
    print(f"double of 20 / 4 = {_vb(doubled)}")
    _t3 = divide(9, 3)
    if isinstance(_t3, Err):
        print(f"Error: {_t3.error}", file=sys.stderr)
        raise SystemExit(1)
    known: int = _t3.value
    print(f"9 / 3 = {_vb(known)}")

def divide(numerator: int, denominator: int) -> int:
    if denominator == 0:
        return Err('cannot divide by zero')
    return Ok(numerator // denominator)

def doublequotient(a: int, b: int) -> int:
    _t4 = divide(a, b)
    if isinstance(_t4, Err):
        return _t4
    q: int = _t4.value
    return Ok(q * 2)


if __name__ == "__main__":
    main()
