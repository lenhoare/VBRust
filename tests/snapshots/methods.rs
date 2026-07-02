// Struct methods — impl, Me/self, and &self vs &mut self

#[derive(Debug, Clone)]
struct Person {
    pub name: String,
    pub age: i64,
}

impl Person {
    fn greet(&self) -> String {
        format!("Hi, I'm {} ({})", self.name, self.age)
    }

    fn have_birthday(&mut self) {
        self.age = self.age + 1;
    }
}

fn main() {
    let mut alice: Person = Person { name: "Alice".to_string(), age: 30 };
    println!("{}", alice.greet());
    alice.have_birthday();
    println!("{}", alice.greet());
}
