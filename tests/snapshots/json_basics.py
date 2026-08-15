# Json from the standard library — parse a document and read typed fields.

import sys
from vbrpy import Ok, Err, _vb, Json

def main():
    _t0 = Json.parse('{"name":"Alice","age":42}')
    if isinstance(_t0, Err):
        print(f"Error: {_t0.error}", file=sys.stderr)
        raise SystemExit(1)
    person: Json = _t0.value
    _t1 = person.get_string('name')
    if isinstance(_t1, Err):
        print(f"Error: {_t1.error}", file=sys.stderr)
        raise SystemExit(1)
    print(f"name = {_vb(_t1.value)}")
    _t2 = person.get_int('age')
    if isinstance(_t2, Err):
        print(f"Error: {_t2.error}", file=sys.stderr)
        raise SystemExit(1)
    print(f"age  = {_vb(_t2.value)}")
    _t3 = Json.parse('{"tags":["red","green","blue"]}')
    if isinstance(_t3, Err):
        print(f"Error: {_t3.error}", file=sys.stderr)
        raise SystemExit(1)
    doc: Json = _t3.value
    _t4 = doc.get_array('tags')
    if isinstance(_t4, Err):
        print(f"Error: {_t4.error}", file=sys.stderr)
        raise SystemExit(1)
    tags: list[Json] = _t4.value
    print(f"tag count: {_vb(len(tags))}")
    for tag in tags:
        _t5 = tag.as_string()
        if isinstance(_t5, Err):
            print(f"Error: {_t5.error}", file=sys.stderr)
            raise SystemExit(1)
        print(f"  tag: {_vb(_t5.value)}")


if __name__ == "__main__":
    main()
