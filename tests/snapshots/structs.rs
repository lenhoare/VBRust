// Structs — Type/End Type, construction, and field access

#[derive(Debug, Clone)]
struct Person {
    pub name: String,
    pub age: i64,
}

fn vbr_main() -> Result<(), String> {
    let mut alice: Person = Person { name: "Alice".to_string(), age: 30 };
    println!("{} is {}", alice.name, alice.age);
    alice.age = alice.age + 1;
    println!("after a birthday, {} is {}", alice.name, alice.age);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
