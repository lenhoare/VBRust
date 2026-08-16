# Simple enums — a named set of variants. They're Copy, compare with `=`, and
# pair naturally with Match. Reference a variant as `Suit.Hearts` → `Suit::Hearts`.

import sys
from dataclasses import dataclass
from enum import Enum

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

class Suit(Enum):
    Hearts = 1
    Diamonds = 2
    Clubs = 3
    Spades = 4

def color(s: Suit) -> str:
    _m0 = s
    match _m0:
        case Suit.Hearts:
            return Ok('red')
        case Suit.Diamonds:
            return Ok('red')
        case Suit.Clubs:
            return Ok('black')
        case Suit.Spades:
            return Ok('black')

def main():
    s: Suit = Suit.Spades
    _t0 = color(s)
    if isinstance(_t0, Err):
        print(f"Error: {_t0.error}", file=sys.stderr)
        raise SystemExit(1)
    print(f"Spades are {_vb(_t0.value)}")
    _t1 = color(Suit.Hearts)
    if isinstance(_t1, Err):
        print(f"Error: {_t1.error}", file=sys.stderr)
        raise SystemExit(1)
    print(f"Hearts are {_vb(_t1.value)}")
    if s == Suit.Spades:
        print('yes, spades')


if __name__ == "__main__":
    main()
