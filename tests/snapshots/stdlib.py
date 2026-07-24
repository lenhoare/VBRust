# Standard library — file I/O and regex. Calls translate `.` to `::`.

from vbrpy import Some, Ok, Err, _vb, _unwrap, FileSystem, Regex

def main():
    _unwrap(FileSystem.write('greeting.txt', 'Hello   from   VBR'))
    text: str = _unwrap(FileSystem.read('greeting.txt'))
    print(f"file says: {_vb(text)}")
    cleaned: str = _unwrap(Regex.replaceall('\\s+', text, ' '))
    print(f"cleaned:   {_vb(cleaned)}")
    _unwrap(FileSystem.delete('greeting.txt'))


if __name__ == "__main__":
    main()
