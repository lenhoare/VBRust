# Simple enums — a named set of variants. They're Copy, compare with `=`, and
# pair naturally with Match. Reference a variant as `Suit.Hearts` → `Suit::Hearts`.

from enum import Enum

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
            return "red"
        case Suit.Diamonds:
            return "red"
        case Suit.Clubs:
            return "black"
        case Suit.Spades:
            return "black"

def main():
    s: Suit = Suit.Spades
    print(f"Spades are {_vb(color(s))}")
    print(f"Hearts are {_vb(color(Suit.Hearts))}")
    if s == Suit.Spades:
        print("yes, spades")


if __name__ == "__main__":
    main()
