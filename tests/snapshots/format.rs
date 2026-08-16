// Format uses a Rust format string, not VB's #.### pictures.

fn vbr_main() -> Result<(), String> {
    println!("{}", format!("{:.2}", 3.14159));
    println!("{}", format!("{:04}", 7));
    println!("{}", format!("approx {:.1}", 3.5));
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
