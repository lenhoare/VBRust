# Result<()> — a fallible action that returns no value on success. `Ok(())` is
# the unit success; failure carries the error as usual.

from dataclasses import dataclass

@dataclass
class Ok:
    value: object

@dataclass
class Err:
    error: object

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

def save(ok: bool) -> object:
    if not (ok):
        return Err('save failed')
    return Ok(None)

def main():
    _m0 = save(True)
    match _m0:
        case Ok(_):
            print('saved')
        case Err(e):
            print(f"error: {_vb(e)}")
    _m1 = save(False)
    match _m1:
        case Ok(_):
            print('saved')
        case Err(e):
            print(f"error: {_vb(e)}")


if __name__ == "__main__":
    main()
