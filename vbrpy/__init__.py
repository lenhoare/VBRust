"""vbrpy — the VBR standard library for the Python target, mirroring the
`vbr_stdlib` Rust crate. A stdlib-using program is emitted as a project folder
(`main.py` + this package beside it), the Python parallel of `vbr runproject`."""

from .prelude import Some, Ok, Err, _vb, _unwrap, _vb_round
from .filesystem import FileSystem
from .regex import Regex
from .jsonval import Json
from .database import Database
from .datetimeval import DateTime
from .http import Http
from .shell import Shell, Process

# DataFrame lowers to idiomatic polars: `col`/`when`/`read_csv` are re-exported
# straight from polars (the Rust side likewise re-exports `col`/`lit`/`when`
# from `vbr_stdlib::dataframe`). Imported LAZILY via module `__getattr__` so a
# program that doesn't use DataFrame never needs polars installed.
_POLARS_EXPORTS = {"col", "lit", "when", "read_csv"}


def __getattr__(name):
    if name in _POLARS_EXPORTS:
        import polars

        return getattr(polars, name)
    raise AttributeError(f"module 'vbrpy' has no attribute {name!r}")


__all__ = [
    "Some",
    "Ok",
    "Err",
    "_vb",
    "_unwrap",
    "_vb_round",
    "FileSystem",
    "Regex",
    "Json",
    "Database",
    "DateTime",
    "Http",
    "Shell",
    "Process",
    "col",
    "lit",
    "when",
    "read_csv",
]
