"""`DateTime` — the VBR standard library's date/time, on Python's `datetime`. A
moment in local time you hold and call methods on, mirroring `vbr_stdlib::
DateTime` (chrono). `Format`/`Parse` use strftime patterns, the same language
chrono uses (`%Y-%m-%d %H:%M:%S`). Named `datetimeval` internally so
`import datetime` reaches the real stdlib module."""

import datetime as _dt

from .prelude import Ok, Err


class DateTime:
    def __init__(self, value):
        self._v = value  # a datetime.datetime in local time

    @staticmethod
    def now():
        return DateTime(_dt.datetime.now())

    @staticmethod
    def parse(text, pattern):
        try:
            return Ok(DateTime(_dt.datetime.strptime(text, pattern)))
        except ValueError as e:
            return Err(str(e))

    def format(self, pattern):
        return self._v.strftime(pattern)

    def adddays(self, days):
        return DateTime(self._v + _dt.timedelta(days=days))

    def addhours(self, hours):
        return DateTime(self._v + _dt.timedelta(hours=hours))

    def addminutes(self, minutes):
        return DateTime(self._v + _dt.timedelta(minutes=minutes))

    def diffdays(self, other):
        # Truncate toward zero, matching chrono's `num_days`.
        return int((other._v - self._v).total_seconds() / 86400)

    def diffhours(self, other):
        return int((other._v - self._v).total_seconds() / 3600)

    def year(self):
        return self._v.year

    def month(self):
        return self._v.month

    def day(self):
        return self._v.day
