# Data-carrying enums (sum types): each variant carries its own data. Build one
# with `Shape.Circle(r)`; pull the data back out by matching. This is the same
# shape as Option/Result — now you can define your own.

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

class Shape:
    pass

@dataclass
class Circle(Shape):
    f0: float

@dataclass
class Rectangle(Shape):
    f0: float
    f1: float

@dataclass
class Empty(Shape):
    pass

def area(s: Shape) -> float:
    _m0 = s
    match _m0:
        case Circle(r):
            return Ok((3.14159 * r) * r)
        case Rectangle(w, h):
            return Ok(w * h)
        case Empty():
            return Ok(0.0)

def main():
    c: Shape = Circle(2.0)
    r: Shape = Rectangle(3.0, 4.0)
    _t0 = area(c)
    if isinstance(_t0, Err):
        print(f"Error: {_t0.error}", file=sys.stderr)
        raise SystemExit(1)
    print(f"circle area = {_vb(_t0.value)}")
    _t1 = area(r)
    if isinstance(_t1, Err):
        print(f"Error: {_t1.error}", file=sys.stderr)
        raise SystemExit(1)
    print(f"rect area   = {_vb(_t1.value)}")
    _t2 = area(Empty())
    if isinstance(_t2, Err):
        print(f"Error: {_t2.error}", file=sys.stderr)
        raise SystemExit(1)
    print(f"empty area  = {_vb(_t2.value)}")


if __name__ == "__main__":
    main()
