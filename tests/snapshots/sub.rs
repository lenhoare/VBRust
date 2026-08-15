// `Sub` is sugar for a `Function` with no return value — both become a Rust `fn`.

fn greet(name: &str) -> Result<(), String> {
    println!("Hello, {}", name);
    Ok(())
}

fn vbr_main() -> Result<(), String> {
    greet("world")?;
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
