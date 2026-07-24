"""`Http` — the VBR standard library's simple blocking HTTP, on Python's
`urllib` (stdlib, no third-party install). One-shot GET/POST returning the
response body as `Ok`/`Err`, mirroring `vbr_stdlib::Http` (which wraps `ureq`).
A 60s timeout turns a hung server into an `Err` rather than a permanent hang."""

import urllib.request

from .prelude import Ok, Err

_TIMEOUT = 60


class Http:
    @staticmethod
    def get(url):
        try:
            req = urllib.request.Request(url, method="GET")
            with urllib.request.urlopen(req, timeout=_TIMEOUT) as resp:
                return Ok(resp.read().decode("utf-8"))
        except Exception as e:
            return Err(str(e))

    @staticmethod
    def post(url, body, headers):
        try:
            req = urllib.request.Request(url, data=body.encode("utf-8"), method="POST")
            for name, value in headers.items():
                req.add_header(name, value)
            with urllib.request.urlopen(req, timeout=_TIMEOUT) as resp:
                return Ok(resp.read().decode("utf-8"))
        except Exception as e:
            return Err(str(e))
