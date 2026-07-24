# Do loops, Exit and Continue

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

def main():
    i: int = 1
    while i <= 3:
        print(f"while {_vb(i)}")
        i = i + 1
    j: int = 10
    while not (j == 0):
        j = j - 2
    print(f"j ended at {_vb(j)}")
    n: int = 0
    while True:
        n = n + 1
        if not (n < 3):
            break
    print(f"n = {_vb(n)}")
    for k in range(1, 7):
        if k == 4:
            break
        if k == 2:
            continue
        print(f"k = {_vb(k)}")


if __name__ == "__main__":
    main()
