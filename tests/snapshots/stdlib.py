# Standard library — file I/O and regex. Calls translate `.` to `::`.

import sys
from vbrpy import Ok, Err, _vb, FileSystem, Regex

def main():
    _t0 = FileSystem.write('greeting.txt', 'Hello   from   Bust')
    if isinstance(_t0, Err):
        print(f"Error: {_t0.error}", file=sys.stderr)
        raise SystemExit(1)
    _t0.value
    _t1 = FileSystem.read('greeting.txt')
    if isinstance(_t1, Err):
        print(f"Error: {_t1.error}", file=sys.stderr)
        raise SystemExit(1)
    text: str = _t1.value
    print(f"file says: {_vb(text)}")
    _t2 = Regex.replace_all('\\s+', text, ' ')
    if isinstance(_t2, Err):
        print(f"Error: {_t2.error}", file=sys.stderr)
        raise SystemExit(1)
    cleaned: str = _t2.value
    print(f"cleaned:   {_vb(cleaned)}")
    _t3 = FileSystem.delete('greeting.txt')
    if isinstance(_t3, Err):
        print(f"Error: {_t3.error}", file=sys.stderr)
        raise SystemExit(1)
    _t3.value


if __name__ == "__main__":
    main()
