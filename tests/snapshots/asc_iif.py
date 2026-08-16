# Two classic VB builtins: Asc (the inverse of Chr) and IIf (immediate-if).
# Asc gives a character's code; IIf picks one of two values by a condition
# (lowered to a lazy Rust `if`/`else`, so — unlike VB — only one arm runs).

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

def main():
    print(_vb((ord(('A')[0]) if ('A') else 0)))
    # 65
    print(_vb(chr(int((ord(('A')[0]) if ('A') else 0) + 1) & 255)))
    # "B" — next letter
    size: str = ('big') if (10 > 3) else ('small')
    print(_vb(size))
    n: int = (100) if ((4 % 2) == 0) else (200)
    print(_vb(n))
    # Mismatched arms (an owned String and a &str literal) still unify.
    word: str = 'hello'
    print(_vb((word) if ((ord(('z')[0]) if ('z') else 0) > 100) else ('?')))


if __name__ == "__main__":
    main()
