# Functions, parameters and returns

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

def main():
    a: int = add(2, 3)
    s: int = square(4)
    f: int = factorial(5)
    print(f"2 + 3 = {_vb(a)}")
    print(f"4 squared = {_vb(s)}")
    print(f"5! = {_vb(f)}")

def add(x: int, y: int) -> int:
    return x + y

def square(n: int) -> int:
    return n * n
    # VB style: assign to the function name

def factorial(n: int) -> int:
    if n <= 1:
        return 1
    return n * factorial(n - 1)


if __name__ == "__main__":
    main()
