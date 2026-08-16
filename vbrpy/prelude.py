"""The shared Bust runtime prelude — the Python analogue of the inlined
`_vb`/`_unwrap`/`Some`/`Ok`/`Err` helpers a single-file `vbr py` program carries.

In a project (a stdlib-using program) these live here instead, so `main.py` and
every `vbrpy` module share ONE definition of `Some`/`Ok`/`Err` — otherwise their
`isinstance` checks would see different classes. Keep these byte-compatible with
the inlined constants in `src/python.rs` (a test guards the drift)."""

from dataclasses import dataclass
import math as _math


@dataclass
class Some:
    value: object


@dataclass
class Ok:
    value: object


@dataclass
class Err:
    error: object


def _vb(x):
    """Rust's `Display` for the values Python prints differently."""
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)


def _unwrap(x):
    if isinstance(x, (Some, Ok)):
        return x.value
    if isinstance(x, Err):
        raise Exception(f'unwrapped an Err: {x.error}')
    if x is None:
        raise Exception('unwrapped a None')
    return x


def _vb_round(x):
    return _math.floor(x + 0.5) if x >= 0 else _math.ceil(x - 0.5)


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


def _vb_cint(s):
    try:
        n = int(str(s).strip())
        if n < -2147483648 or n > 2147483647:
            return Err("out of range")
        return Ok(n)
    except ValueError as e:
        return Err(str(e))


def _vb_input_box(prompt):
    try:
        return Ok(input(prompt))
    except EOFError:
        return Err("end of input")
