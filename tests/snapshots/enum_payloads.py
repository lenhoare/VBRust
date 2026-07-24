# Enum variants can now carry any payload — structs, several values, even a
# `Vec` (which also lets an enum hold a collection of things).

from dataclasses import dataclass

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
            return f"dot at {_vb(p.x)},{_vb(p.y)}"
        case Segment(a, b):
            return f"segment {_vb(a.x)} to {_vb(b.x)}"
        case Blob(pts):
            return f"blob of {_vb(len(pts))} points"
        case Empty():
            return 'nothing'

def main():
    print(_vb(describe(Dot(Point(x=1.0, y=2.0)))))
    print(_vb(describe(Segment(Point(x=1.0, y=2.0), Point(x=5.0, y=6.0)))))
    cloud: list[Point] = []
    cloud.append(Point(x=1.0, y=2.0))
    cloud.append(Point(x=5.0, y=6.0))
    print(_vb(describe(Blob(cloud))))
    print(_vb(describe(Empty())))


if __name__ == "__main__":
    main()
