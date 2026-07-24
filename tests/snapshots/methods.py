# Struct methods — impl, Me/self, and &self vs &mut self

from dataclasses import dataclass

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
        return f"Hi, I'm {_vb(self.name)} ({_vb(self.age)})"

    def havebirthday(self):
        self.age = self.age + 1

def main():
    alice: Person = Person(name='Alice', age=30)
    print(_vb(alice.greet()))
    alice.havebirthday()
    print(_vb(alice.greet()))


if __name__ == "__main__":
    main()
