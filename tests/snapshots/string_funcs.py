# Built-in string functions

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

def main():
    s: str = 'Hello, World'
    print(f"length:    {_vb(len(s))}")
    print(f"upper:     {_vb((s).upper())}")
    print(f"lower:     {_vb((s).lower())}")
    print(f"left 5:    {_vb((s)[:max(int(5), 0)])}")
    print(f"right 5:   {_vb(((s)[-int(5):] if int(5) else ''))}")
    print(f"mid 2,3:   {_vb((s)[max(int(2) - 1, 0):max(int(2) - 1, 0) + int(3)])}")
    print(f"trimmed:   {_vb(('   padded   ').strip())}")
    print(f"replaced:  {_vb((s).replace('World', 'Rust'))}")
    print(f"str of 42: {_vb(str(42))}")


if __name__ == "__main__":
    main()
