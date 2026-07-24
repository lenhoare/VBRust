# Guards (`If`) and the `_` wildcard. A guard is a Rust match guard — the arm
# only fires when its condition is also true. `x` binds the matched value.

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
            return 'zero'
        case x if x < 0:
            return 'negative'
        case x if x > 100:
            return 'huge'
        case _:
            return 'ordinary'

def main():
    print(f"-3 is {_vb(describe(-3))}")
    print(f"0 is {_vb(describe(0))}")
    print(f"42 is {_vb(describe(42))}")
    print(f"500 is {_vb(describe(500))}")


if __name__ == "__main__":
    main()
