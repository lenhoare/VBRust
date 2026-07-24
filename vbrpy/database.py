"""`Database` — the VBR standard library's SQLite, on Python's `sqlite3`. A
`Database` is a live connection you hold and call methods on, mirroring
`vbr_stdlib::Database` (rusqlite): `execute` returns the affected-row count,
`query` returns each row as a `Json` object keyed by column name (values in
their natural type), and params bind as text (SQLite column affinity stores them
typed), matching the Rust `Vec<String>` params."""

import sqlite3

from .prelude import Ok, Err
from .jsonval import Json


class Database:
    def __init__(self, conn):
        self._c = conn
        self._lastid = 0

    @staticmethod
    def open(path):
        try:
            return Ok(Database(sqlite3.connect(path)))
        except sqlite3.Error as e:
            return Err(str(e))

    def execute(self, sql, params):
        try:
            cur = self._c.execute(sql, params)
            self._c.commit()
            self._lastid = cur.lastrowid
            return Ok(cur.rowcount)
        except sqlite3.Error as e:
            return Err(str(e))

    def query(self, sql, params):
        try:
            cur = self._c.execute(sql, params)
            cols = [d[0] for d in cur.description]
            return Ok([Json(dict(zip(cols, row))) for row in cur.fetchall()])
        except sqlite3.Error as e:
            return Err(str(e))

    def lastinsertid(self):
        return self._lastid
