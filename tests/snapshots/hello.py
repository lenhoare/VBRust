# VBR vertical-slice demo — everything here is in the first milestone

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

def main():
    count: int = 3
    total: int = 0
    for i in range(1, count + 1):
        total = total + i
    ratio: float = 2.5
    print(f"Sum 1..{_vb(count)} = {_vb(total)}")
    print(f"ratio is {_vb(ratio)}")
    if total > 5:
        print("big")
    elif total == 5:
        print("exactly five")
    else:
        print("small")


if __name__ == "__main__":
    main()
