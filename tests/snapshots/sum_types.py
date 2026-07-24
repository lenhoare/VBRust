# Data-carrying enums (sum types): each variant carries its own data. Build one
# with `Shape.Circle(r)`; pull the data back out by matching. This is the same
# shape as Option/Result — now you can define your own.

from dataclasses import dataclass

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
            return (3.14159 * r) * r
        case Rectangle(w, h):
            return w * h
        case Empty():
            return 0.0

def main():
    c: Shape = Circle(2.0)
    r: Shape = Rectangle(3.0, 4.0)
    print(f"circle area = {_vb(area(c))}")
    print(f"rect area   = {_vb(area(r))}")
    print(f"empty area  = {_vb(area(Empty()))}")


if __name__ == "__main__":
    main()
