# A fallible action that returns no value on success. RaiseError fails; a
# bare End Function is success. Intercept at the call with Handle.

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

def save(ok: bool):
    if not (ok):
        return Err('save failed')
    return Ok(None)

def main():
    _t0 = save(True)
    if isinstance(_t0, Err):
        e = _t0.error
        print(f"error: {_vb(e)}")
        return
    print('saved')
    _t1 = save(False)
    if isinstance(_t1, Err):
        e = _t1.error
        print(f"error: {_vb(e)}")
        return
    print('saved')


if __name__ == "__main__":
    main()
