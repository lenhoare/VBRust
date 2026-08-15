// Struct methods — impl, Me/self, and &self vs &mut self

#[derive(Debug, Clone)]
struct Person {
    pub name: String,
    pub age: i64,
}

impl Person {
    fn greet(&self) -> Result<String, String> {
        Ok(format!("Hi, I'm {} ({})", self.name, self.age))
    }

    fn havebirthday(&mut self) -> Result<(), String> {
        self.age = self.age + 1;
        Ok(())
    }
}

fn vbr_main() -> Result<(), String> {
    let mut alice: Person = Person { name: "Alice".to_string(), age: 30 };
    println!("{}", alice.greet()?);
    alice.havebirthday()?;
    println!("{}", alice.greet()?);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
