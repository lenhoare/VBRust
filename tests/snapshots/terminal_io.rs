// Terminal I/O — InputBox reads a line, MsgBox prints to the terminal

fn input_box(prompt: &str) -> String {
    use std::io::Write;
    print!("{}", prompt);
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    line.trim_end().to_string()
}

fn main() {
    let name: String = input_box("What is your name? ");
    println!("{}", format!("{}{}", format!("{}{}", "Hello, ", name), "!"));
    println!("{}", "Nice to meet you.");
}
