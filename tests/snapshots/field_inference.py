# Struct fields, collection elements, and Me carry their declared types:
# mixed-width arithmetic through them gets the same automatic widening casts
# as plain variables, and a method that mutates Me only through a mutating
# method call (Push) still takes &mut self.

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
class Basket:
    label: str
    rate: float
    qty: int
    weights: list[int]

    def addweight(self, w: int):
        self.weights.append(w)
        return Ok(None)

    def totalweight(self) -> int:
        sum: int = 0
        for w in self.weights:
            sum += w
        return Ok(sum)

def main():
    start: list[int] = []
    b: Basket = Basket(label='box', rate=2.5, qty=3, weights=start)
    _t0 = b.addweight(10)
    if isinstance(_t0, Err):
        print(f"Error: {_t0.error}", file=sys.stderr)
        raise SystemExit(1)
    _t0.value
    _t1 = b.addweight(32)
    if isinstance(_t1, Err):
        print(f"Error: {_t1.error}", file=sys.stderr)
        raise SystemExit(1)
    _t1.value
    # A Double field times an Integer field — widened automatically.
    cost: float = b.rate * b.qty
    # An Integer field meets a Long variable the same way.
    n: int = 100
    scaled: int = b.qty * n
    _t2 = b.totalweight()
    if isinstance(_t2, Err):
        print(f"Error: {_t2.error}", file=sys.stderr)
        raise SystemExit(1)
    print(f"{_vb(b.label)} cost {_vb(cost)}, scaled {_vb(scaled)}, weight {_vb(_t2.value)}")


if __name__ == "__main__":
    main()
