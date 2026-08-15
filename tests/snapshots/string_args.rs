// Passing strings to functions — Bust borrows an owned String automatically

fn shout(text: &str) -> Result<String, String> {
    Ok(text.to_uppercase())
}

fn vbr_main() -> Result<(), String> {
    let name: String = "alice".to_string();
    println!("{}", shout(&name)?);
    // owned String  -> shout(&name)
    println!("{}", shout("bob")?);
    // literal &str   -> shout("bob")
    println!("{}", shout(&format!("{}!", name))?);
    // concat String  -> shout(&format!(...))
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
