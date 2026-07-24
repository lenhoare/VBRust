# Result<T, E> with a real, typed error enum — including a message-carrying
# variant. Build errors with Err(MathError.…); read them back by matching. `?`
# works when the error types line up.

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

class MathError:
    pass

@dataclass
class DivByZero(MathError):
    pass

@dataclass
class Custom(MathError):
    f0: str

def safediv(a: int, b: int) -> object:
    if b == 0:
        return Err(DivByZero())
    if b < 0:
        return Err(Custom('negative divisor'))
    return Ok(a // b)

def doublediv(a: int, b: int) -> object:
    _t0 = safediv(a, b)
    if isinstance(_t0, Err):
        return _t0
    q: int = _t0.value
    return Ok(q * 2)

def main():
    _m0 = doublediv(10, 2)
    match _m0:
        case Ok(v):
            print(f"ok: {_vb(v)}")
        case Err(DivByZero()):
            print('div by zero')
        case Err(Custom(msg)):
            print(f"error: {_vb(msg)}")
    _m1 = doublediv(10, 0)
    match _m1:
        case Ok(v):
            print(f"ok: {_vb(v)}")
        case Err(_):
            print('failed')
    _m2 = doublediv(10, -2)
    match _m2:
        case Ok(v):
            print(f"ok: {_vb(v)}")
        case Err(DivByZero()):
            print('div by zero')
        case Err(Custom(msg)):
            print(f"error: {_vb(msg)}")


if __name__ == "__main__":
    main()
