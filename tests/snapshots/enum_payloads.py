# Enum variants can now carry any payload — structs, several values, even a
# `Vec` (which also lets an enum hold a collection of things).

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

@dataclass
class Point:
    x: float
    y: float

class Shape:
    pass

@dataclass
class Dot(Shape):
    f0: Point

@dataclass
class Segment(Shape):
    f0: Point
    f1: Point

@dataclass
class Blob(Shape):
    f0: list[Point]

@dataclass
class Empty(Shape):
    pass

def describe(s: Shape) -> str:
    _m0 = s
    match _m0:
        case Dot(p):
            return Ok(f"dot at {_vb(p.x)},{_vb(p.y)}")
        case Segment(a, b):
            return Ok(f"segment {_vb(a.x)} to {_vb(b.x)}")
        case Blob(pts):
            return Ok(f"blob of {_vb(len(pts))} points")
        case Empty():
            return Ok('nothing')

def main():
    _t0 = describe(Dot(Point(x=1.0, y=2.0)))
    if isinstance(_t0, Err):
        print(f"Error: {_t0.error}", file=sys.stderr)
        raise SystemExit(1)
    print(_vb(_t0.value))
    _t1 = describe(Segment(Point(x=1.0, y=2.0), Point(x=5.0, y=6.0)))
    if isinstance(_t1, Err):
        print(f"Error: {_t1.error}", file=sys.stderr)
        raise SystemExit(1)
    print(_vb(_t1.value))
    cloud: list[Point] = []
    cloud.append(Point(x=1.0, y=2.0))
    cloud.append(Point(x=5.0, y=6.0))
    _t2 = describe(Blob(cloud))
    if isinstance(_t2, Err):
        print(f"Error: {_t2.error}", file=sys.stderr)
        raise SystemExit(1)
    print(_vb(_t2.value))
    _t3 = describe(Empty())
    if isinstance(_t3, Err):
        print(f"Error: {_t3.error}", file=sys.stderr)
        raise SystemExit(1)
    print(_vb(_t3.value))


if __name__ == "__main__":
    main()
