// Structs — Type/End Type, construction, and field access

struct Person {
    pub name: String,
    pub age: i32,
}

fn main() {
    let mut alice: Person = Person { name: "Alice".to_string(), age: 30 };
    println!("{}", format!("{}{}", format!("{}{}", alice.name, " is "), alice.age));
    alice.age = alice.age + 1;
    println!("{}", format!("{}{}", format!("{}{}", format!("{}{}", "after a birthday, ", alice.name), " is "), alice.age));
}
