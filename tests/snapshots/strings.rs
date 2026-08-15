// String and ownership demo

fn vbr_main() -> Result<(), String> {
    let greeting: String = "Hello".to_string();
    // a literal is a fixed-size &str
    let view = &greeting;
    // borrow — no copy is made
    let combined: String = format!("{}, World", greeting);
    // concat makes an owned String
    let copy: String = combined.clone();
    // explicit owned copy
    println!("{}", view);
    println!("{}", combined);
    println!("{}", copy);
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
