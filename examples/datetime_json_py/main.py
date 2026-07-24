# Standard library — DateTime and Json are value types you hold and use

from vbrpy import Some, Ok, Err, _vb, _unwrap, DateTime, Json

def main():
    now: DateTime = DateTime.now()
    print(f"this year: {_vb(now.year())}")
    later: DateTime = now.adddays(30)
    print(f"in 30 days: {_vb(later.format('%Y-%m-%d'))}")
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
