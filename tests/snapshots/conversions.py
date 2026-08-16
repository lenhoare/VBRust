# VB's two ways to turn text into a number, and how they differ.
# 
# Val   — the *lenient* one. Always a Double, ignores surrounding spaces,
# and returns 0 for text that isn't a number. It never fails, so
# there is nothing to handle.
# CDbl / CLng / CInt — the *strict* conversions. On text that isn't a
# number they fail. The error propagates automatically, or you
# intercept it with Handle. Use these when bad input is an error
# you want to catch, not silently turn into 0.

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

def _vb_val(s):
    try:
        return float(str(s).strip())
    except ValueError:
        return 0.0

def _vb_cdbl(s):
    try:
        return Ok(float(str(s).strip()))
    except ValueError as e:
        return Err(str(e))

def _vb_clng(s):
    try:
        return Ok(int(str(s).strip()))
    except ValueError as e:
        return Err(str(e))

def priceof(txt: str) -> float:
    # A bad CDbl fails this function — the caller Handle's it, or Main exits.
    _t0 = _vb_cdbl(txt)
    if isinstance(_t0, Err):
        return _t0
    return Ok(_t0.value)

def main():
    # Lenient: 0 on nonsense, spaces ignored, always a Double.
    print(_vb(_vb_val('3.14')))
    print(_vb(_vb_val('  42  ')))
    print(_vb(_vb_val('nonsense')))
    # A Double flows into a Long with Bust's automatic numeric cast.
    count: int = _vb_val('100')
    print(_vb(count))
    # Strict: intercept failure with Handle.
    v: int = 0
    _t1 = _vb_clng('77')
    if isinstance(_t1, Err):
        e = _t1.error
        print(f"not a number: {_vb(e)}")
        return
    else:
        v = _t1.value
    print(f"parsed {_vb(v)}")
    p: float = 0.0
    _t2 = priceof('9.99')
    if isinstance(_t2, Err):
        e = _t2.error
        print(_vb(e))
        return
    else:
        p = _t2.value
    print(f"price is {_vb(p)}")


if __name__ == "__main__":
    main()
