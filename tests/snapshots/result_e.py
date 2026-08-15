# Typed errors are strings on the implicit Result<T, String> channel.
# RaiseError fails; Handle intercepts a call and binds the message.

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

def safediv(a: int, b: int) -> int:
    if b == 0:
        return Err('div by zero')
    if b < 0:
        return Err('negative divisor')
    return Ok(a // b)

def doublediv(a: int, b: int) -> int:
    _t0 = safediv(a, b)
    if isinstance(_t0, Err):
        return _t0
    q: int = _t0.value
    return Ok(q * 2)

def main():
    v: int = 0
    _t1 = doublediv(10, 2)
    if isinstance(_t1, Err):
        _ = _t1.error
        print('failed')
        return
    else:
        v = _t1.value
    print(f"ok: {_vb(v)}")
    ignored: int = 0
    _t2 = doublediv(10, 0)
    if isinstance(_t2, Err):
        _ = _t2.error
        print('failed')
        return
    else:
        ignored = _t2.value
    print(f"ok: {_vb(ignored)}")
    v3: int = 0
    _t3 = doublediv(10, -2)
    if isinstance(_t3, Err):
        err = _t3.error
        print(f"error: {_vb(err)}")
        return
    else:
        v3 = _t3.value
    print(f"ok: {_vb(v3)}")


if __name__ == "__main__":
    main()
