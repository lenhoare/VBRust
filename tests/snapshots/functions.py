# Functions, parameters and returns

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
    _t0 = add(2, 3)
    if isinstance(_t0, Err):
        print(f"Error: {_t0.error}", file=sys.stderr)
        raise SystemExit(1)
    a: int = _t0.value
    _t1 = square(4)
    if isinstance(_t1, Err):
        print(f"Error: {_t1.error}", file=sys.stderr)
        raise SystemExit(1)
    s: int = _t1.value
    _t2 = factorial(5)
    if isinstance(_t2, Err):
        print(f"Error: {_t2.error}", file=sys.stderr)
        raise SystemExit(1)
    f: int = _t2.value
    print(f"2 + 3 = {_vb(a)}")
    print(f"4 squared = {_vb(s)}")
    print(f"5! = {_vb(f)}")

def add(x: int, y: int) -> int:
    return Ok(x + y)

def square(n: int) -> int:
    return Ok(n * n)
    # VB style: assign to the function name

def factorial(n: int) -> int:
    if n <= 1:
        return Ok(1)
    _t3 = factorial(n - 1)
    if isinstance(_t3, Err):
        return _t3
    return Ok(n * _t3.value)


if __name__ == "__main__":
    main()
