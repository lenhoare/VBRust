# Inline list literals — `[a, b, …]` builds a Vec<T>.
# 
# Prefix `[…]` is a list; postfix `x[i]` is still indexing — no clash, exactly
# like Rust. String elements are owned automatically; numbers take their type
# from the target.

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

def total(xs: list[int]) -> int:
    sum: int = 0
    for x in xs:
        sum = sum + x
    return sum

def main():
    names: list[str] = ['alice', 'bob', 'carol']
    print(f"first = {_vb(names[0])}, of {_vb(len(names))}")
    # A list literal passed straight into a function (the common case for, e.g.,
    # query parameters).
    print(f"total = {_vb(total([10, 20, 30]))}")
    empty: list[str] = []
    print(f"empty count = {_vb(len(empty))}")


if __name__ == "__main__":
    main()
