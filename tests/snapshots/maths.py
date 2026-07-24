# Built-in maths functions (work on floating-point values)

import math

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

import math as _math
def _vb_round(x):
    return _math.floor(x + 0.5) if x >= 0 else _math.ceil(x - 0.5)

def main():
    x: float = 9.0
    angle: float = 0.0
    print(f"sqrt(9)    = {_vb(math.sqrt(x))}")
    print(f"abs(-5)    = {_vb(abs(-5.0))}")
    print(f"9 ^ 2      = {_vb(x ** 2)}")
    print(f"9 ^ 0.5    = {_vb(x ** 0.5)}")
    print(f"int(3.7)   = {_vb(math.floor(3.7))}")
    print(f"round(3.5) = {_vb(_vb_round(3.5))}")
    print(f"sin(0)     = {_vb(math.sin(angle))}")
    print(f"cos(0)     = {_vb(math.cos(angle))}")
    print(f"exp(1)     = {_vb(math.exp(1.0))}")
    print(f"ln(e)      = {_vb(math.log(2.718281828))}")
    # Mod gives the remainder (→ Rust's %, same precedence as * and /)
    n: int = 17
    print(f"17 Mod 5   = {_vb(n % 5)}")


if __name__ == "__main__":
    main()
