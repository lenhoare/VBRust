# Format uses a Rust format string, not VB's #.### pictures.

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

def main():
    print(_vb('{:.2f}'.format(3.14159)))
    print(_vb('{:04}'.format(7)))
    print(_vb('approx {:.1f}'.format(3.5)))


if __name__ == "__main__":
    main()
