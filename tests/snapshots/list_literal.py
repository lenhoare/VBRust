# Inline list literals — `[a, b, …]` builds a Vec<T>.
# 
# Prefix `[…]` is a list; postfix `x[i]` is still indexing — no clash, exactly
# like Rust. String elements are owned automatically; numbers take their type
# from the target.

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

def total(xs: list[int]) -> int:
    sum: int = 0
    for x in xs:
        sum = sum + x
    return Ok(sum)

def main():
    names: list[str] = ['alice', 'bob', 'carol']
    print(f"first = {_vb(names[0])}, of {_vb(len(names))}")
    # A list literal passed straight into a function (the common case for, e.g.,
    # query parameters).
    _t0 = total([10, 20, 30])
    if isinstance(_t0, Err):
        print(f"Error: {_t0.error}", file=sys.stderr)
        raise SystemExit(1)
    print(f"total = {_vb(_t0.value)}")
    empty: list[str] = []
    print(f"empty count = {_vb(len(empty))}")


if __name__ == "__main__":
    main()
