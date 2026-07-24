# Json from the standard library — parse a document and read typed fields.

from vbrpy import Some, Ok, Err, _vb, _unwrap, Json

def main():
    person: Json = _unwrap(Json.parse('{"name":"Alice","age":42}'))
    print(f"name = {_vb(_unwrap(person.getstring('name')))}")
    print(f"age  = {_vb(_unwrap(person.getint('age')))}")
    doc: Json = _unwrap(Json.parse('{"tags":["red","green","blue"]}'))
    tags: list[Json] = _unwrap(doc.getarray('tags'))
    print(f"tag count: {_vb(len(tags))}")
    for tag in tags:
        print(f"  tag: {_vb(_unwrap(tag.asstring()))}")


if __name__ == "__main__":
    main()
