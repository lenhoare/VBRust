"""`Regex` — the VBR standard library's regex, on Python's `re`. Mirrors
`vbr_stdlib::Regex`: a bad pattern is an `Err`, `find` returns `Ok(Option<..>)`.

Note: replacement group references differ from Rust — Python uses `\\1`, Rust
uses `$1`. Patterns without group references (the common case) are identical."""

import re

from .prelude import Ok, Err, Some


class Regex:
    @staticmethod
    def ismatch(pattern, text):
        try:
            return Ok(re.search(pattern, text) is not None)
        except re.error as e:
            return Err(str(e))

    @staticmethod
    def find(pattern, text):
        try:
            m = re.search(pattern, text)
            return Ok(Some(m.group(0)) if m else None)
        except re.error as e:
            return Err(str(e))

    @staticmethod
    def findall(pattern, text):
        try:
            return Ok(re.findall(pattern, text))
        except re.error as e:
            return Err(str(e))

    @staticmethod
    def replace(pattern, text, replacement):
        try:
            return Ok(re.sub(pattern, replacement, text, count=1))
        except re.error as e:
            return Err(str(e))

    @staticmethod
    def replaceall(pattern, text, replacement):
        try:
            return Ok(re.sub(pattern, replacement, text))
        except re.error as e:
            return Err(str(e))

    @staticmethod
    def captures(pattern, text):
        try:
            m = re.search(pattern, text)
            return Ok(list(m.groups()) if m else [])
        except re.error as e:
            return Err(str(e))
