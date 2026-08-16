// Terminal I/O — InputBox reads a line (fails at end of input), MsgBox prints

fn input_box(prompt: &str) -> Result<String, String> {
    use std::io::Write;
    print!("{}", prompt);
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    match std::io::stdin().read_line(&mut line) {
        Ok(0) => Err("end of input".into()),
        Ok(_) => Ok(line.trim_end().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

fn vbr_main() -> Result<(), String> {
    let name: String = input_box("What is your name? ")?;
    println!("Hello, {}!", name);
    println!("Nice to meet you.");
    Ok(())
}

fn main() {
    if let Err(error) = vbr_main() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
