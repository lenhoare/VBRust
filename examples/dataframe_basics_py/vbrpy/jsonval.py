"""`Json` — the VBR standard library's JSON value, on Python's `json`. A `Json`
wraps a parsed Python object (dict/list/scalar); the typed accessors return
`Ok`/`Err`, mirroring `vbr_stdlib::Json` (`get_string` → `Result<String>` etc.).
Named `jsonval` internally so `import json` reaches the real stdlib module."""

import json as _json

from .prelude import Ok, Err

_MISSING = object()


class Json:
    def __init__(self, value):
        self._v = value

    @staticmethod
    def parse(text):
        try:
            return Ok(Json(_json.loads(text)))
        except ValueError as e:
            return Err(str(e))

    @staticmethod
    def object():
        return Json({})

    @staticmethod
    def array():
        return Json([])

    def _field(self, key):
        if isinstance(self._v, dict) and key in self._v:
            return self._v[key]
        return _MISSING

    def haskey(self, key):
        return isinstance(self._v, dict) and key in self._v

    def isnull(self):
        return self._v is None

    def getstring(self, key):
        v = self._field(key)
        if isinstance(v, str):
            return Ok(v)
        return Err(f"key '{key}' is not a string")

    def getint(self, key):
        v = self._field(key)
        if isinstance(v, int) and not isinstance(v, bool):
            return Ok(v)
        return Err(f"key '{key}' is not an integer")

    def getfloat(self, key):
        v = self._field(key)
        if isinstance(v, (int, float)) and not isinstance(v, bool):
            return Ok(float(v))
        return Err(f"key '{key}' is not a number")

    def getbool(self, key):
        v = self._field(key)
        if isinstance(v, bool):
            return Ok(v)
        return Err(f"key '{key}' is not a boolean")

    def getarray(self, key):
        v = self._field(key)
        if isinstance(v, list):
            return Ok([Json(x) for x in v])
        return Err(f"key '{key}' is not an array")

    def get(self, key):
        v = self._field(key)
        if v is _MISSING:
            return Err(f"no key '{key}'")
        return Ok(Json(v))

    def asstring(self):
        if isinstance(self._v, str):
            return Ok(self._v)
        return Err("value is not a string")

    def asint(self):
        if isinstance(self._v, int) and not isinstance(self._v, bool):
            return Ok(self._v)
        return Err("value is not an integer")

    def asfloat(self):
        if isinstance(self._v, (int, float)) and not isinstance(self._v, bool):
            return Ok(float(self._v))
        return Err("value is not a number")

    def asbool(self):
        if isinstance(self._v, bool):
            return Ok(self._v)
        return Err("value is not a boolean")

    def tostring(self):
        try:
            return Ok(_json.dumps(self._v, separators=(",", ":")))
        except (TypeError, ValueError) as e:
            return Err(str(e))

    def topretty(self):
        try:
            return Ok(_json.dumps(self._v, indent=2))
        except (TypeError, ValueError) as e:
            return Err(str(e))

    def setstring(self, key, val):
        self._v[key] = val

    def setint(self, key, val):
        self._v[key] = val

    def setbool(self, key, val):
        self._v[key] = val

    def set(self, key, val):
        self._v[key] = val._v

    def push(self, val):
        self._v.append(val._v)
