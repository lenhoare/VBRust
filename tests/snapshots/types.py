# Every fixed-size type — these all copy freely

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

def main():
    small: int = 42
    count: int = 100000
    huge: int = 9000000000
    pi: float = 3.14
    ratio: float = 2.5
    flag: bool = True
    letter: int = 65
    print(f"small  = {_vb(small)}")
    print(f"count  = {_vb(count)}")
    print(f"huge   = {_vb(huge)}")
    print(f"pi     = {_vb(pi)}")
    print(f"ratio  = {_vb(ratio)}")
    print(f"flag   = {_vb(flag)}")
    print(f"letter = {_vb(letter)}")


if __name__ == "__main__":
    main()
