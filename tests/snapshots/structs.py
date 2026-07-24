# Structs — Type/End Type, construction, and field access

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

def main():
    alice: Person = Person(name="Alice", age=30)
    print(f"{_vb(alice.name)} is {_vb(alice.age)}")
    alice.age = alice.age + 1
    print(f"after a birthday, {_vb(alice.name)} is {_vb(alice.age)}")


if __name__ == "__main__":
    main()
