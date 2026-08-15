# Struct methods — impl, Me/self, and &self vs &mut self

import sys
from dataclasses import dataclass

@dataclass
class Ok:
    value: object

@dataclass
class Err:
    error: object

def _vb(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, float) and x.is_integer():
        return str(int(x))
    return str(x)

@dataclass
class Person:
    name: str
    age: int

    def greet(self) -> str:
        return Ok(f"Hi, I'm {_vb(self.name)} ({_vb(self.age)})")

    def havebirthday(self):
        self.age = self.age + 1
        return Ok(None)

def main():
    alice: Person = Person(name='Alice', age=30)
    _t0 = alice.greet()
    if isinstance(_t0, Err):
        print(f"Error: {_t0.error}", file=sys.stderr)
        raise SystemExit(1)
    print(_vb(_t0.value))
    _t1 = alice.havebirthday()
    if isinstance(_t1, Err):
        print(f"Error: {_t1.error}", file=sys.stderr)
        raise SystemExit(1)
    _t1.value
    _t2 = alice.greet()
    if isinstance(_t2, Err):
        print(f"Error: {_t2.error}", file=sys.stderr)
        raise SystemExit(1)
    print(_vb(_t2.value))


if __name__ == "__main__":
    main()
