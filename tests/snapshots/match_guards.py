# Guards (`If`) and the `_` wildcard. A guard is a Rust match guard — the arm
# only fires when its condition is also true. `x` binds the matched value.

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

def describe(n: int) -> str:
    _m0 = n
    match _m0:
        case 0:
            return Ok('zero')
        case x if x < 0:
            return Ok('negative')
        case x if x > 100:
            return Ok('huge')
        case _:
            return Ok('ordinary')
    return Ok(None)

def main():
    _t0 = describe(-3)
    if isinstance(_t0, Err):
        print(f"Error: {_t0.error}", file=sys.stderr)
        raise SystemExit(1)
    print(f"-3 is {_vb(_t0.value)}")
    _t1 = describe(0)
    if isinstance(_t1, Err):
        print(f"Error: {_t1.error}", file=sys.stderr)
        raise SystemExit(1)
    print(f"0 is {_vb(_t1.value)}")
    _t2 = describe(42)
    if isinstance(_t2, Err):
        print(f"Error: {_t2.error}", file=sys.stderr)
        raise SystemExit(1)
    print(f"42 is {_vb(_t2.value)}")
    _t3 = describe(500)
    if isinstance(_t3, Err):
        print(f"Error: {_t3.error}", file=sys.stderr)
        raise SystemExit(1)
    print(f"500 is {_vb(_t3.value)}")


if __name__ == "__main__":
    main()
