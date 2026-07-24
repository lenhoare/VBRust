# Struct fields, collection elements, and Me carry their declared types:
# mixed-width arithmetic through them gets the same automatic widening casts
# as plain variables, and a method that mutates Me only through a mutating
# method call (Push) still takes &mut self.

from dataclasses import dataclass

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

    def totalweight(self) -> int:
        sum: int = 0
        for w in self.weights:
            sum += w
        return sum

def main():
    start: list[int] = []
    b: Basket = Basket(label='box', rate=2.5, qty=3, weights=start)
    b.addweight(10)
    b.addweight(32)
    # A Double field times an Integer field — widened automatically.
    cost: float = b.rate * b.qty
    # An Integer field meets a Long variable the same way.
    n: int = 100
    scaled: int = b.qty * n
    print(f"{_vb(b.label)} cost {_vb(cost)}, scaled {_vb(scaled)}, weight {_vb(b.totalweight())}")


if __name__ == "__main__":
    main()
