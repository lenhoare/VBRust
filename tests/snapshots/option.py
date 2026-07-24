# Option<T> for maybe-absent values — Some / None

from dataclasses import dataclass

@dataclass
class Some:
    value: object

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

def main():
    _m0 = halve(10)
    match _m0:
        case Some(value):
            print(f"half of 10 = {_vb(value)}")
        case None:
            print('10 is odd, no exact half')
    _m1 = halve(7)
    match _m1:
        case Some(value):
            print(f"half of 7 = {_vb(value)}")
        case None:
            print('7 is odd, no exact half')

def halve(n: int) -> object:
    if ((n // 2) * 2) == n:
        return Some(n // 2)
    return None


if __name__ == "__main__":
    main()
