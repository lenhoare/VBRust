# database.vbr — SQLite via the stdlib. A Database is a live connection you
# hold and call methods on (like Json/DateTime, not a stateless namespace).
# 
# Params bind to ? placeholders (always as text — column affinity stores them
# typed, so declare columns INTEGER/REAL). NULL goes in the SQL itself:
# VALUES (?, NULL) — a list of strings has no null slot. Query rows come back
# as Json objects keyed by column name, each column with its natural type.
# A ByVal Database param borrows the connection (&Database) — open once,
# hand it around. Fallible calls propagate automatically.
# text and score are `&str` params. Dropping them straight into the params list
# fills a `Vec<String>`, so each is owned with `.to_string()` for you — no manual
# `.clone()` or `CStr(...)`. A literal element (none here) is owned by the list
# emitter as before.

import sys
from vbrpy import Ok, Err, _vb, Database, Json

def run(db: Database):
    _t0 = db.execute('CREATE TABLE IF NOT EXISTS ideas (id INTEGER PRIMARY KEY, gen INTEGER, text TEXT, score REAL, parent INTEGER)', [])
    if isinstance(_t0, Err):
        return _t0
    _t0.value
    _t1 = db.execute('DELETE FROM ideas', [])
    if isinstance(_t1, Err):
        return _t1
    _t1.value
    # A root idea has no parent — the NULL is written in the SQL.
    _t2 = db.execute('INSERT INTO ideas (gen, text, score, parent) VALUES (1, ?, ?, NULL)', ['solar tracker', '0.82'])
    if isinstance(_t2, Err):
        return _t2
    _t2.value
    root: int = db.last_insert_id()
    # A child links to its parent via the fresh rowid — lineage.
    _t3 = db.execute('INSERT INTO ideas (gen, text, score, parent) VALUES (2, ?, ?, ?)', ['improved tracker', '0.91', str(root)])
    if isinstance(_t3, Err):
        return _t3
    _t3.value
    # Insert through a helper whose text/score arrive as ByVal String params —
    # a `&str` element in the params list, owned into the Vec<String> for you.
    _t4 = addscored(db, 'wind turbine', '0.75')
    if isinstance(_t4, Err):
        return _t4
    _t4.value
    _t5 = db.query('SELECT text, score, parent FROM ideas ORDER BY score DESC', [])
    if isinstance(_t5, Err):
        return _t5
    rows: list[Json] = _t5.value
    for row in rows:
        _t6 = row.get_string('text')
        if isinstance(_t6, Err):
            return _t6
        _t7 = row.get_float('score')
        if isinstance(_t7, Err):
            return _t7
        line: str = f"{_vb(_t6.value)} scores {_vb(_t7.value)}"
        _t8 = row.get('parent')
        if isinstance(_t8, Err):
            return _t8
        if _t8.value.is_null():
            print(f"{_vb(line)} (a root idea)")
        else:
            _t9 = row.get_int('parent')
            if isinstance(_t9, Err):
                return _t9
            print(f"{_vb(line)} (child of #{_vb(_t9.value)})")
    return Ok(None)

def addscored(db: Database, text: str, score: str):
    _t10 = db.execute('INSERT INTO ideas (gen, text, score, parent) VALUES (3, ?, ?, NULL)', [text, score])
    if isinstance(_t10, Err):
        return _t10
    _t10.value
    return Ok(None)

def main():
    _t11 = Database.open('ideas.db')
    if isinstance(_t11, Err):
        print(f"Error: {_t11.error}", file=sys.stderr)
        raise SystemExit(1)
    db: Database = _t11.value
    _t12 = run(db)
    if isinstance(_t12, Err):
        print(f"Error: {_t12.error}", file=sys.stderr)
        raise SystemExit(1)
    _t12.value
    print('done')


if __name__ == "__main__":
    main()
