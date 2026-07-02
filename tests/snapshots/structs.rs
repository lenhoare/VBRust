// Structs — Type/End Type, construction, and field access

#[derive(Debug, Clone)]
struct Person {
    pub name: String,
    pub age: i64,
}

fn main() {
    let mut alice: Person = Person { name: "Alice".to_string(), age: 30 };
    println!("{} is {}", alice.name, alice.age);
    alice.age = alice.age + 1;
    println!("after a birthday, {} is {}", alice.name, alice.age);
}
