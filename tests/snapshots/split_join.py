# Split, Join, Space — VB's string-list builtins.

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

def main():
    csv: str = 'one,two,three'
    parts: list[str] = (csv).split(',')
    print(_vb((' / ').join(parts)))
    # Default delimiter is a single space, both ways.
    print(_vb(' '.join(('a b c').split(' '))))
    print(f"[{_vb((' ' * max(int(3), 0)))}]")


if __name__ == "__main__":
    main()
