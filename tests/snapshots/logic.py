# Logical operators: And, Or, Not, Xor. Logical (short-circuit) and looser
# than comparison, just like Rust's &&, ||, !, ^ — no backwards-compat quirks.

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

def main():
    age: int = 30
    member: bool = True
    if (age >= 18) and member:
        print('admitted')
    if (age < 13) or (age > 65):
        print('discounted')
    else:
        print('full price')
    if not (member):
        print('please join')
    else:
        print('welcome back')
    # Xor: true when exactly one side is true.
    heads: bool = True
    tails: bool = False
    print(f"valid coin: {_vb(heads ^ tails)}")
    # Precedence: And binds tighter than Or, comparisons tighter than both.
    ok: bool = ((age > 0) and (age < 120)) or member
    print(f"ok: {_vb(ok)}")


if __name__ == "__main__":
    main()
