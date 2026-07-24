# Result<T> for fallible functions — propagate with ?, handle with Match

from dataclasses import dataclass

@dataclass
class Some:
    value: object

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

def _unwrap(x):
    if isinstance(x, (Some, Ok)):
        return x.value
    if isinstance(x, Err):
        raise Exception(f'unwrapped an Err: {x.error}')
    if x is None:
        raise Exception('unwrapped a None')
    return x

def main():
    # Handle the outcome explicitly
    _m0 = divide(10, 2)
    match _m0:
        case Ok(value):
            print(f"10 / 2 = {_vb(value)}")
        case Err(message):
            print(f"error: {_vb(message)}")
    _m1 = divide(7, 0)
    match _m1:
        case Ok(value):
            print(f"7 / 0 = {_vb(value)}")
        case Err(message):
            print(f"error: {_vb(message)}")
    # A function that uses ? to propagate failure
    _m2 = doublequotient(20, 4)
    match _m2:
        case Ok(value):
            print(f"double of 20 / 4 = {_vb(value)}")
        case Err(message):
            print(f"error: {_vb(message)}")
    # .Unwrap() is allowed, but training wheels
    known: int = _unwrap(divide(9, 3))
    print(f"9 / 3 = {_vb(known)}")

def divide(numerator: int, denominator: int) -> object:
    if denominator == 0:
        return Err('cannot divide by zero')
    return Ok(numerator // denominator)

def doublequotient(a: int, b: int) -> object:
    _t0 = divide(a, b)
    if isinstance(_t0, Err):
        return _t0
    q: int = _t0.value
    return Ok(q * 2)


if __name__ == "__main__":
    main()
