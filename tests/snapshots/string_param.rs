// A String parameter defaults to ByVal — a read-only borrow. No keyword is
// needed to read it; only changing it requires ByRef.

fn loudly(message: &str) -> Result<String, String> {
    Ok(format!("{}!", message))
}

fn vbr_main() -> Result<(), String> {
    let note: String = "hello".to_string();
    println!("{}", loudly(&note)?);
    println!("{}", note);
    // note is untouched — Loudly only borrowed it
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
