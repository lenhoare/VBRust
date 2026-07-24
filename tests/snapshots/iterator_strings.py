# Iterator chains over OWNED elements — a Vec<String> clones items into the
# chain (`.iter().cloned()`, a real copy), filter's closure sees each item by
# reference, and find returns an Option you Match.

from dataclasses import dataclass

@dataclass
class Some:
    value: object

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

def main():
    names: list[str] = []
    names.append('Ada')
    names.append('Grace')
    names.append('Linus')
    longnames: list[str] = [n for n in names if len(n) > 3]
    for n in longnames:
        print(f"long:  {_vb(n)}")
    shouted: list[str] = [n.upper() for n in names]
    for n in shouted:
        print(f"loud:  {_vb(n)}")
    _m0 = next((Some(n) for n in names if n.startswith('G')), None)
    match _m0:
        case Some(hit):
            print(f"found: {_vb(hit)}")
        case None:
            print('no match')


if __name__ == "__main__":
    main()
